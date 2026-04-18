mod controller;
mod types;

pub(crate) mod backend;

pub use crate::ui::widget::VideoSurface;
pub use controller::VideoController;
pub(crate) use types::VideoSurfaceSnapshot;
pub use types::{PlaybackState, VideoMetrics, VideoSize, VideoSource};
