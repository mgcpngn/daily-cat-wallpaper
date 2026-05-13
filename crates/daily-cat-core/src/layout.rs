use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Canvas {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct SafeArea {
    pub left: u32,
    pub right: u32,
    pub top: u32,
    pub bottom: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Default)]
pub struct LayoutEngine;

impl LayoutEngine {
    pub fn slots(&self, canvas: Canvas, safe_area: SafeArea, cat_count: u8) -> Vec<Rect> {
        if cat_count == 0 || cat_count > 5 {
            return Vec::new();
        }

        let usable_x = safe_area.left.min(canvas.width);
        let usable_y = safe_area.top.min(canvas.height);
        let usable_width = canvas
            .width
            .saturating_sub(safe_area.left)
            .saturating_sub(safe_area.right);
        let usable_height = canvas
            .height
            .saturating_sub(safe_area.top)
            .saturating_sub(safe_area.bottom);

        if usable_width == 0 || usable_height == 0 {
            return Vec::new();
        }

        let count = u32::from(cat_count);
        let columns = match cat_count {
            1 => 1,
            2 => 2,
            3 | 4 => 2,
            _ => 3,
        };
        let rows = count.div_ceil(columns);
        let gutter = (usable_width.min(usable_height) / 40).clamp(16, 64);
        let cell_width = usable_width / columns;
        let cell_height = usable_height / rows;

        (0..count)
            .map(|index| {
                let col = index % columns;
                let row = index / columns;
                let x = usable_x + col * cell_width + gutter / 2;
                let y = usable_y + row * cell_height + gutter / 2;
                let width = cell_width.saturating_sub(gutter).max(1);
                let height = cell_height.saturating_sub(gutter).max(1);

                Rect {
                    x,
                    y,
                    width,
                    height,
                }
            })
            .collect()
    }
}
