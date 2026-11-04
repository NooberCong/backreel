//! A small drawing surface over Direct2D and DirectWrite.

use super::layout::Rect;
use crate::Result;
use windows::Win32::Foundation::{D2DERR_RECREATE_TARGET, HWND, RECT};
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Imaging::IWICBitmapSource;
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;
use windows::core::{HSTRING, w};
use windows_numerics::Matrix3x2;

pub const CORNER: f32 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color(pub u32, pub f32);

impl Color {
    pub const fn rgb(hex: u32) -> Self {
        Self(hex, 1.0)
    }

    pub const fn with_alpha(self, alpha: f32) -> Self {
        Self(self.0, alpha)
    }

    fn to_d2d(self) -> D2D1_COLOR_F {
        let channel = |shift: u32| ((self.0 >> shift) & 0xFF) as f32 / 255.0;
        D2D1_COLOR_F { r: channel(16), g: channel(8), b: channel(0), a: self.1 }
    }
}

#[derive(Clone, Copy)]
pub enum Font {
    Title,
    Body,
    Small,
    Label,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Align {
    Leading,
    Center,
}

struct Fonts {
    title: IDWriteTextFormat,
    body: IDWriteTextFormat,
    small: IDWriteTextFormat,
    label: IDWriteTextFormat,
}

pub struct Canvas {
    hwnd: HWND,
    factory: ID2D1Factory,
    fonts: Fonts,
    target: Option<ID2D1HwndRenderTarget>,
    brush: Option<ID2D1SolidColorBrush>,
}

impl Canvas {
    pub fn new(hwnd: HWND) -> Result<Self> {
        // SAFETY: both factories are created with valid arguments and no options.
        unsafe {
            let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let write: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let fonts = Fonts {
                title: font(&write, 15.0, DWRITE_FONT_WEIGHT_SEMI_BOLD)?,
                body: font(&write, 14.0, DWRITE_FONT_WEIGHT_NORMAL)?,
                small: font(&write, 12.0, DWRITE_FONT_WEIGHT_NORMAL)?,
                label: font(&write, 12.0, DWRITE_FONT_WEIGHT_SEMI_BOLD)?,
            };
            Ok(Self { hwnd, factory, fonts, target: None, brush: None })
        }
    }

    /// Drops device resources; the next `begin` recreates them.
    pub fn reset(&mut self) {
        self.target = None;
        self.brush = None;
    }

    /// Prepares to draw. Returns true when the target was just created, which
    /// means bitmaps made for the previous target are gone.
    pub fn begin(&mut self, dpi: f32, background: Color) -> Result<bool> {
        let created = self.target.is_none();
        if created {
            self.create_target(dpi)?;
        }
        let target = self.target()?;
        // SAFETY: the target is valid; `EndDraw` is called in `end`.
        unsafe {
            target.SetDpi(dpi, dpi);
            target.BeginDraw();
            target.Clear(Some(&background.to_d2d()));
        }
        Ok(created)
    }

    /// Finishes drawing. On device loss, resources are dropped so the next paint rebuilds them.
    pub fn end(&mut self) -> Result<bool> {
        // SAFETY: pairs with `BeginDraw` in `begin`.
        let result = unsafe { self.target()?.EndDraw(None, None) };
        match result {
            Ok(()) => Ok(true),
            Err(error) if error.code() == D2DERR_RECREATE_TARGET => {
                self.reset();
                Ok(false)
            }
            Err(error) => Err(error.into()),
        }
    }

    fn create_target(&mut self, dpi: f32) -> Result<()> {
        let mut client = RECT::default();
        // SAFETY: `client` is valid for writes and the property structs outlive the calls.
        unsafe {
            GetClientRect(self.hwnd, &mut client)?;
            // Software rendering: this window is static, and loading the GPU
            // driver would cost far more memory than it saves in drawing time.
            let properties = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_IGNORE,
                },
                dpiX: dpi,
                dpiY: dpi,
                ..Default::default()
            };
            let window = D2D1_HWND_RENDER_TARGET_PROPERTIES {
                hwnd: self.hwnd,
                pixelSize: D2D_SIZE_U { width: client.right as u32, height: client.bottom as u32 },
                presentOptions: D2D1_PRESENT_OPTIONS_NONE,
            };
            let target = self.factory.CreateHwndRenderTarget(&properties, &window)?;
            target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            self.brush = Some(target.CreateSolidColorBrush(&Color::rgb(0).to_d2d(), None)?);
            self.target = Some(target);
        }
        Ok(())
    }

    pub fn resize(&self, width: u32, height: u32) -> Result<()> {
        if let Some(target) = &self.target {
            // SAFETY: the size struct outlives the call.
            unsafe { target.Resize(&D2D_SIZE_U { width, height })? };
        }
        Ok(())
    }

    fn target(&self) -> Result<&ID2D1HwndRenderTarget> {
        Ok(self.target.as_ref().ok_or("no render target")?)
    }

    fn brush(&self, color: Color) -> Result<&ID2D1SolidColorBrush> {
        let brush = self.brush.as_ref().ok_or("no brush")?;
        // SAFETY: the colour struct outlives the call.
        unsafe { brush.SetColor(&color.to_d2d()) };
        Ok(brush)
    }

    pub fn fill(&self, rect: Rect, radius: f32, color: Color) -> Result<()> {
        // SAFETY: valid only between begin and end, which is where painting happens.
        unsafe { self.target()?.FillRoundedRectangle(&rounded(rect, radius), self.brush(color)?) };
        Ok(())
    }

    pub fn stroke(&self, rect: Rect, radius: f32, color: Color, width: f32) -> Result<()> {
        // Inset by half the stroke so the line stays inside `rect`.
        let inset = width / 2.0;
        let rect = Rect::new(rect.x + inset, rect.y + inset, rect.w - width, rect.h - width);
        // SAFETY: as in `fill`.
        unsafe { self.target()?.DrawRoundedRectangle(&rounded(rect, radius), self.brush(color)?, width, None) };
        Ok(())
    }

    pub fn circle(&self, x: f32, y: f32, radius: f32, color: Color) -> Result<()> {
        let ellipse = D2D1_ELLIPSE { point: point(x, y), radiusX: radius, radiusY: radius };
        // SAFETY: as in `fill`.
        unsafe { self.target()?.FillEllipse(&ellipse, self.brush(color)?) };
        Ok(())
    }

    pub fn line(&self, from: (f32, f32), to: (f32, f32), color: Color) -> Result<()> {
        // SAFETY: as in `fill`.
        unsafe { self.target()?.DrawLine(point(from.0, from.1), point(to.0, to.1), self.brush(color)?, 1.0, None) };
        Ok(())
    }

    pub fn text(&self, text: &str, rect: Rect, font: Font, align: Align, color: Color) -> Result<()> {
        let format = match font {
            Font::Title => &self.fonts.title,
            Font::Body => &self.fonts.body,
            Font::Small => &self.fonts.small,
            Font::Label => &self.fonts.label,
        };
        let wide = HSTRING::from(text);
        // SAFETY: `wide`, the format and the rectangle outlive the call.
        unsafe {
            format.SetTextAlignment(match align {
                Align::Leading => DWRITE_TEXT_ALIGNMENT_LEADING,
                Align::Center => DWRITE_TEXT_ALIGNMENT_CENTER,
            })?;
            self.target()?.DrawText(
                &wide,
                format,
                &to_rect(rect),
                self.brush(color)?,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
        Ok(())
    }

    /// Uploads a pixel source sized in physical pixels to match `dpi`.
    pub fn create_bitmap(&self, source: &IWICBitmapSource, dpi: f32) -> Result<ID2D1Bitmap> {
        let properties = D2D1_BITMAP_PROPERTIES {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: dpi,
            dpiY: dpi,
        };
        // SAFETY: `source` and the properties outlive the call.
        Ok(unsafe { self.target()?.CreateBitmapFromWicBitmap(source, Some(&properties))? })
    }

    /// Fills `rect` with `bitmap`, keeping the rounded corners.
    pub fn image(&self, bitmap: &ID2D1Bitmap, rect: Rect, radius: f32) -> Result<()> {
        // SAFETY: as in `fill`; the brush lives for the duration of the draw.
        unsafe {
            let target = self.target()?;
            let brush = target.CreateBitmapBrush(bitmap, None, None)?;
            brush.SetTransform(&Matrix3x2::translation(rect.x, rect.y));
            target.FillRoundedRectangle(&rounded(rect, radius), &brush);
        }
        Ok(())
    }
}

fn font(write: &IDWriteFactory, size: f32, weight: DWRITE_FONT_WEIGHT) -> Result<IDWriteTextFormat> {
    // SAFETY: the strings are static and the trimming structs outlive the calls.
    unsafe {
        let format = write.CreateTextFormat(
            w!("Segoe UI Variable Text"),
            None,
            weight,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            size,
            w!("en-us"),
        )?;
        format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
        let ellipsis = write.CreateEllipsisTrimmingSign(&format)?;
        let trimming = DWRITE_TRIMMING { granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER, ..Default::default() };
        format.SetTrimming(&trimming, &ellipsis)?;
        Ok(format)
    }
}

fn point(x: f32, y: f32) -> windows_numerics::Vector2 {
    windows_numerics::Vector2 { X: x, Y: y }
}

fn to_rect(rect: Rect) -> D2D_RECT_F {
    D2D_RECT_F { left: rect.x, top: rect.y, right: rect.x + rect.w, bottom: rect.y + rect.h }
}

fn rounded(rect: Rect, radius: f32) -> D2D1_ROUNDED_RECT {
    D2D1_ROUNDED_RECT { rect: to_rect(rect), radiusX: radius, radiusY: radius }
}
