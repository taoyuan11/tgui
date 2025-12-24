#[derive(Debug, Clone, Copy)]
pub struct Padding {
    pub left: i32,
    pub right: i32,
    pub top: i32,
    pub bottom: i32,
}

impl Padding {
    pub fn build() -> Self {
        Self { left: 0, right: 0, top: 0, bottom: 0, }
    }

    pub fn with_left(mut self, val: i32) -> Self {
        self.left = val;
        self
    }

    pub fn with_right(mut self, val: i32) -> Self {
        self.right = val;
        self
    }

    pub fn with_top(mut self, val: i32) -> Self {
        self.top = val;
        self
    }

    pub fn with_bottom(mut self, val: i32) -> Self {
        self.bottom = val;
        self
    }

    pub fn with_vertical(mut self, vertical: i32) -> Self {
        self.top = vertical;
        self.bottom = vertical;
        self
    }

    pub fn with_horizontal(mut self, horizontal: i32) -> Self {
        self.left = horizontal;
        self.right = horizontal;
        self
    }

    pub fn all(val: i32) -> Self {
        Padding { left: val, right: val, top: val, bottom: val, }
    }
}