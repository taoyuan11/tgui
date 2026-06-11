use std::sync::Arc;

/// 滑块组件的方向。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SliderOrientation {
    #[default]
    Horizontal,
    Vertical,
}

impl SliderOrientation {
    pub(crate) fn is_horizontal(self) -> bool {
        matches!(self, Self::Horizontal)
    }
}

#[derive(Clone)]
pub(crate) struct SliderValueFormatter {
    formatter: Arc<dyn Fn(f32) -> String + Send + Sync>,
}

impl SliderValueFormatter {
    pub(crate) fn new(formatter: impl Fn(f32) -> String + Send + Sync + 'static) -> Self {
        Self {
            formatter: Arc::new(formatter),
        }
    }

    pub(crate) fn format(&self, value: f32) -> String {
        (self.formatter)(value)
    }
}
