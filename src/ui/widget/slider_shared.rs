use std::sync::Arc;

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
