//! The settings window's geometry, in device-independent pixels. Painting and
//! hit-testing both read from here, so they cannot disagree.

use crate::sources::{Category, Source};

pub const WIDTH: f32 = 400.0;
pub const HEIGHT: f32 = 594.0;

const PAD: f32 = 20.0;
const CONTENT: f32 = WIDTH - 2.0 * PAD;
const GAP: f32 = 8.0;
const CHIP_HEIGHT: f32 = 34.0;
const CHIP_WIDTH: f32 = (CONTENT - 2.0 * GAP) / 3.0;
const BUTTON_WIDTH: f32 = 84.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

pub const PREVIEW: Rect = Rect::new(PAD, PAD, CONTENT, CONTENT * 9.0 / 16.0);
pub const TITLE: Rect = Rect::new(PAD, 234.0, CONTENT - BUTTON_WIDTH - 12.0, 24.0);
pub const CREDIT: Rect = Rect::new(PAD, 257.0, CONTENT - BUTTON_WIDTH - 12.0, 18.0);
pub const NEXT: Rect = Rect::new(WIDTH - PAD - BUTTON_WIDTH, 238.0, BUTTON_WIDTH, 34.0);
pub const CATEGORIES_LABEL: Rect = Rect::new(PAD, 294.0, CONTENT, 18.0);
pub const SOURCES_LABEL: Rect = Rect::new(PAD, 410.0, CONTENT, 18.0);
pub const DIVIDER_Y: f32 = 488.0;
pub const DAILY_LABEL: Rect = Rect::new(PAD, 500.0, CONTENT - 56.0, 22.0);
pub const DAILY_STATUS: Rect = Rect::new(PAD, 522.0, CONTENT - 56.0, 18.0);
pub const DAILY_SWITCH: Rect = Rect::new(WIDTH - PAD - 44.0, 509.0, 44.0, 22.0);
pub const TRAY_LABEL: Rect = Rect::new(PAD, 552.0, CONTENT - 56.0, 22.0);
pub const TRAY_SWITCH: Rect = Rect::new(WIDTH - PAD - 44.0, 552.0, 44.0, 22.0);

const _: () = assert!(PREVIEW.y + PREVIEW.h < TITLE.y, "the preview must clear the title row");

/// Everything that responds to the mouse or keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    OpenPage,
    Next,
    Category(Category),
    Source(Source),
    Daily,
    Tray,
}

/// Controls in tab order, each with its rectangle.
pub fn controls() -> Vec<(Control, Rect)> {
    let mut items = vec![
        (Control::OpenPage, Rect::new(TITLE.x, TITLE.y, TITLE.w, CREDIT.y + CREDIT.h - TITLE.y)),
        (Control::Next, NEXT),
    ];
    for (index, category) in Category::ALL.into_iter().enumerate() {
        let row = (index / 3) as f32;
        items.push((Control::Category(category), chip(index % 3, 318.0 + row * (CHIP_HEIGHT + GAP))));
    }
    for (index, source) in Source::ALL.into_iter().enumerate() {
        items.push((Control::Source(source), chip(index, 434.0)));
    }
    items.push((Control::Daily, DAILY_SWITCH));
    items.push((Control::Tray, TRAY_SWITCH));
    items
}

fn chip(column: usize, y: f32) -> Rect {
    Rect::new(PAD + column as f32 * (CHIP_WIDTH + GAP), y, CHIP_WIDTH, CHIP_HEIGHT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlap(a: &Rect, b: &Rect) -> bool {
        a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
    }

    #[test]
    fn controls_never_overlap() {
        let items = controls();
        for (i, (_, a)) in items.iter().enumerate() {
            for (_, b) in &items[i + 1..] {
                assert!(!overlap(a, b), "{a:?} overlaps {b:?}");
            }
        }
    }

    #[test]
    fn everything_fits_inside_the_padding() {
        for (control, rect) in controls() {
            assert!(rect.x >= PAD - 0.01, "{control:?}");
            assert!(rect.x + rect.w <= WIDTH - PAD + 0.01, "{control:?}");
            assert!(rect.y + rect.h <= HEIGHT - 12.0, "{control:?}");
        }
    }

    #[test]
    fn hit_testing_uses_half_open_edges() {
        let rect = Rect::new(10.0, 10.0, 5.0, 5.0);
        assert!(rect.contains(10.0, 14.9));
        assert!(!rect.contains(15.0, 10.0));
    }
}
