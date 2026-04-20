// SPDX-License-Identifier: MPL-2.0

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RectU32 {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

impl RectU32 {
    pub fn new(x: u32, y: u32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }

    pub fn x(&self) -> u32 {
        self.x
    }

    pub fn y(&self) -> u32 {
        self.y
    }

    pub fn width(&self) -> u32 {
        self.w
    }

    pub fn height(&self) -> u32 {
        self.h
    }

    pub fn set_x(&mut self, x: u32) {
        self.x = x;
    }

    pub fn set_y(&mut self, y: u32) {
        self.y = y;
    }

    pub fn set_width(&mut self, w: u32) {
        self.w = w;
    }

    pub fn set_height(&mut self, h: u32) {
        self.h = h;
    }
}
