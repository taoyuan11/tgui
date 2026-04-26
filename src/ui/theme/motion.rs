#[derive(Clone, Debug, PartialEq)]
pub struct MotionScale {
    pub fast_ms: u64,
    pub normal_ms: u64,
    pub slow_ms: u64,
}

impl Default for MotionScale {
    fn default() -> Self {
        Self {
            fast_ms: 120,
            normal_ms: 180,
            slow_ms: 280,
        }
    }
}
