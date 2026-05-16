mod controller;
mod types;

pub(crate) mod backend;

pub use crate::ui::widget::Audio;
pub use controller::AudioController;
pub(crate) use types::AudioSnapshot;
pub use types::{AudioMetrics, AudioSource, PlaybackState};
