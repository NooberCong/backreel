//! Image work through Windows Imaging Component: no bundled codecs.

use crate::Result;
use std::path::Path;
use windows::Win32::Graphics::Imaging::*;
use windows::Win32::System::Com::StructuredStorage::{IPropertyBag2, PROPBAG2};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, IStream};
use windows::Win32::System::Variant::{VARIANT, VT_R4};
use windows::core::{GUID, HSTRING, Interface, PWSTR, w};

const JPEG_QUALITY: f32 = 0.93;
/// Anything larger is a corrupt header or a decompression bomb.
const MAX_DIMENSION: u32 = 20_000;
const MAX_PIXELS: u64 = 120_000_000;

/// How to scale a source so it covers a target, then which part to keep.
#[derive(Debug, PartialEq, Eq)]
pub struct Cover {
    pub scaled_width: u32,
    pub scaled_height: u32,
    pub crop_x: u32,
    pub crop_y: u32,
}

/// Computes a centred aspect-fill of `source` onto `target`.
pub fn cover((src_w, src_h): (u32, u32), (dst_w, dst_h): (u32, u32)) -> Cover {
    // Cross-multiplication keeps the comparison exact.
    let wider = u64::from(src_w) * u64::from(dst_h) >= u64::from(dst_w) * u64::from(src_h);
    let (scaled_width, scaled_height) = if wider {
        (scale(src_w, dst_h, src_h).max(dst_w), dst_h)
    } else {
        (dst_w, scale(src_h, dst_w, src_w).max(dst_h))
    };
    Cover { scaled_width, scaled_height, crop_x: (scaled_width - dst_w) / 2, crop_y: (scaled_height - dst_h) / 2 }
}

fn scale(value: u32, numerator: u32, denominator: u32) -> u32 {
    let scaled = (u64::from(value) * u64::from(numerator)).div_ceil(u64::from(denominator));
    u32::try_from(scaled).unwrap_or(u32::MAX)
}

/// Fraction of the source lost when it is cropped to `target`'s aspect ratio.
pub fn crop_loss(source: (u32, u32), target: (u32, u32)) -> f64 {
    let source_aspect = f64::from(source.0) / f64::from(source.1);
    let target_aspect = f64::from(target.0) / f64::from(target.1);
    1.0 - (source_aspect / target_aspect).min(target_aspect / source_aspect)
}

pub struct Imaging {
    factory: IWICImagingFactory,
}

impl Imaging {
    pub fn new() -> Result<Self> {
        // SAFETY: COM is initialised on this thread by the caller.
        let factory = unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)? };
        Ok(Self { factory })
    }

    /// Decodes `bytes`, covers `target`, and writes a JPEG to `path`.
    pub fn render_jpeg(&self, bytes: &[u8], target: (u32, u32), path: &Path) -> Result<()> {
        // SAFETY: `bytes` outlives the stream and decoder, which are local to this call.
        unsafe {
            let stream = self.factory.CreateStream()?;
            stream.InitializeFromMemory(bytes)?;
            let decoder =
                self.factory.CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnDemand)?;
            self.write_jpeg(&self.cover_source(&decoder.GetFrame(0)?, target)?, target, path)
        }
    }

    /// Decodes the image at `path` covering `target` into a bitmap source.
    pub fn load_cover(&self, path: &Path, target: (u32, u32)) -> Result<IWICBitmapSource> {
        // SAFETY: the path string outlives the call.
        unsafe {
            let decoder = self.factory.CreateDecoderFromFilename(
                &HSTRING::from(path),
                None,
                windows::Win32::Foundation::GENERIC_READ,
                WICDecodeMetadataCacheOnDemand,
            )?;
            self.cover_source(&decoder.GetFrame(0)?, target)
        }
    }

    /// Converts `source` into the premultiplied BGRA format Direct2D expects.
    pub fn to_pbgra(&self, source: &IWICBitmapSource) -> Result<IWICBitmapSource> {
        self.convert(source, &GUID_WICPixelFormat32bppPBGRA)
    }

    fn cover_source(&self, frame: &IWICBitmapFrameDecode, target: (u32, u32)) -> Result<IWICBitmapSource> {
        let (mut width, mut height) = (0, 0);
        // SAFETY: the out-pointers are valid, and each WIC object keeps its input alive.
        unsafe {
            frame.GetSize(&mut width, &mut height)?;
            check_dimensions(width, height)?;
            let plan = cover((width, height), target);

            // The scaler sits directly on the frame so JPEG can downscale while decoding.
            let scaler = self.factory.CreateBitmapScaler()?;
            scaler.Initialize(
                frame,
                plan.scaled_width,
                plan.scaled_height,
                WICBitmapInterpolationModeHighQualityCubic,
            )?;

            let clipper = self.factory.CreateBitmapClipper()?;
            let rect = WICRect {
                X: plan.crop_x as i32,
                Y: plan.crop_y as i32,
                Width: target.0 as i32,
                Height: target.1 as i32,
            };
            clipper.Initialize(&scaler, &rect)?;
            Ok(clipper.cast()?)
        }
    }

    fn convert(&self, source: &IWICBitmapSource, format: &GUID) -> Result<IWICBitmapSource> {
        // SAFETY: `source` and `format` are valid for the calls; the converter holds its own reference.
        unsafe {
            let converter = self.factory.CreateFormatConverter()?;
            converter.Initialize(source, format, WICBitmapDitherTypeNone, None, 0.0, WICBitmapPaletteTypeCustom)?;
            Ok(converter.cast()?)
        }
    }

    fn write_jpeg(&self, source: &IWICBitmapSource, (width, height): (u32, u32), path: &Path) -> Result<()> {
        let source = self.convert(source, &GUID_WICPixelFormat24bppBGR)?;
        // SAFETY: all pointers are valid for each call; objects are held until `Commit` returns.
        unsafe {
            let stream = self.factory.CreateStream()?;
            stream.InitializeFromFilename(&HSTRING::from(path), windows::Win32::Foundation::GENERIC_WRITE.0)?;
            let encoder = self.factory.CreateEncoder(&GUID_ContainerFormatJpeg, std::ptr::null())?;
            encoder.Initialize(&stream.cast::<IStream>()?, WICBitmapEncoderNoCache)?;

            let mut frame = None;
            let mut options: Option<IPropertyBag2> = None;
            encoder.CreateNewFrame(&mut frame, &mut options)?;
            let frame = frame.ok_or("JPEG encoder returned no frame")?;
            let options = options.ok_or("JPEG encoder returned no options")?;
            let quality = PROPBAG2 { pstrName: PWSTR(w!("ImageQuality").as_ptr().cast_mut()), ..Default::default() };
            let mut value = VARIANT::default();
            (*value.Anonymous.Anonymous).vt = VT_R4;
            (*value.Anonymous.Anonymous).Anonymous.fltVal = JPEG_QUALITY;
            options.Write(1, &quality, &value)?;

            frame.Initialize(&options)?;
            frame.SetSize(width, height)?;
            let mut format: GUID = GUID_WICPixelFormat24bppBGR;
            frame.SetPixelFormat(&mut format)?;
            frame.WriteSource(&source, std::ptr::null())?;
            frame.Commit()?;
            encoder.Commit()?;
        }
        Ok(())
    }
}

fn check_dimensions(width: u32, height: u32) -> Result<()> {
    let pixels = u64::from(width) * u64::from(height);
    if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION || pixels > MAX_PIXELS {
        return Err(format!("refusing a {width}x{height} image").into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_aspect_needs_no_crop() {
        let plan = cover((3840, 2160), (2560, 1440));
        assert_eq!(plan, Cover { scaled_width: 2560, scaled_height: 1440, crop_x: 0, crop_y: 0 });
    }

    #[test]
    fn wider_source_crops_the_sides() {
        let plan = cover((4000, 1000), (1920, 1080));
        assert_eq!((plan.scaled_width, plan.scaled_height), (4320, 1080));
        assert_eq!((plan.crop_x, plan.crop_y), (1200, 0));
    }

    #[test]
    fn taller_source_crops_top_and_bottom() {
        let plan = cover((1000, 2000), (1920, 1080));
        assert_eq!((plan.scaled_width, plan.scaled_height), (1920, 3840));
        assert_eq!((plan.crop_x, plan.crop_y), (0, 1380));
    }

    #[test]
    fn cover_always_contains_the_target() {
        for src in [(1, 1), (7, 3), (3, 7), (1920, 1081), (2561, 1440), (20000, 3)] {
            for dst in [(2560, 1440), (1440, 2560), (3440, 1440), (5120, 1440), (1366, 768)] {
                let plan = cover(src, dst);
                assert!(plan.crop_x + dst.0 <= plan.scaled_width, "{src:?} {dst:?}");
                assert!(plan.crop_y + dst.1 <= plan.scaled_height, "{src:?} {dst:?}");
            }
        }
    }

    #[test]
    fn crop_loss_matches_aspect_difference() {
        assert_eq!(crop_loss((1920, 1080), (3840, 2160)), 0.0);
        let ultrawide = crop_loss((1920, 1080), (3440, 1440));
        assert!((ultrawide - 0.2558).abs() < 0.001, "{ultrawide}");
        assert_eq!(crop_loss((1000, 1000), (2000, 1000)), crop_loss((2000, 1000), (1000, 1000)));
    }

    /// A minimal 24-bit BMP, so that decoding can be tested without a fixture file.
    fn bmp(width: u32, height: u32) -> Vec<u8> {
        let row = (width * 3).div_ceil(4) * 4;
        let pixels = row * height;
        let mut file = Vec::new();
        file.extend_from_slice(b"BM");
        file.extend_from_slice(&(54 + pixels).to_le_bytes());
        file.extend_from_slice(&[0; 4]);
        file.extend_from_slice(&54u32.to_le_bytes());
        file.extend_from_slice(&40u32.to_le_bytes());
        file.extend_from_slice(&width.to_le_bytes());
        file.extend_from_slice(&height.to_le_bytes());
        file.extend_from_slice(&1u16.to_le_bytes());
        file.extend_from_slice(&24u16.to_le_bytes());
        file.extend_from_slice(&[0; 24]);
        file.resize(file.len() + pixels as usize, 0x80);
        file
    }

    #[test]
    fn renders_a_real_image_to_the_exact_target_size() {
        let _com = crate::Com::init().unwrap();
        let imaging = Imaging::new().unwrap();
        let path = std::env::temp_dir().join("backreel-test-render.jpg");

        imaging.render_jpeg(&bmp(300, 100), (64, 48), &path).unwrap();

        let (mut width, mut height) = (0, 0);
        let rendered = imaging.load_cover(&path, (64, 48)).unwrap();
        unsafe { rendered.GetSize(&mut width, &mut height).unwrap() };
        assert_eq!((width, height), (64, 48));
        drop(rendered); // Releases the decoder's hold on the file.
        assert_eq!(std::fs::read(&path).unwrap()[..2], [0xFF, 0xD8], "not a JPEG");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn garbage_bytes_are_an_error_not_a_crash() {
        let _com = crate::Com::init().unwrap();
        let imaging = Imaging::new().unwrap();
        let path = std::env::temp_dir().join("backreel-test-garbage.jpg");
        assert!(imaging.render_jpeg(b"<html>not an image</html>", (64, 48), &path).is_err());
        assert!(imaging.render_jpeg(&[], (64, 48), &path).is_err());
    }

    #[test]
    fn rejects_absurd_dimensions() {
        assert!(check_dimensions(0, 100).is_err());
        assert!(check_dimensions(30_000, 100).is_err());
        assert!(check_dimensions(15_000, 15_000).is_err());
        assert!(check_dimensions(8000, 6000).is_ok());
    }
}
