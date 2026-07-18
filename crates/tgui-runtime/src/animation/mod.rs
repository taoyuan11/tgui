mod controller;
mod engine;
mod frame_clock;
mod spec;
#[cfg(test)]
mod tests;
mod value;

pub use controller::{AnimationControllerBuilder, AnimationControllerHandle, AnimationStatus};
pub use engine::Animatable;
pub use spec::{
    AnimationCurve, AnimationSpec, Easing, FillMode, Keyframe, Keyframes, Playback,
    PlaybackDirection, Repeat, Transition,
};
pub use value::AnimatedValue;

#[cfg(feature = "bench-support")]
pub(crate) use controller::sample_timeline;
pub(crate) use controller::AnimationCoordinator;
pub(crate) use engine::{
    default_theme_transition, AnimationEngine, AnimationKey, AnimationRefresh, WidgetProperty,
    WindowProperty,
};
#[cfg(feature = "bench-support")]
pub(crate) use engine::{
    refresh_widget_dedup_stats, reset_refresh_widget_dedup_stats, with_legacy_refresh_widget_dedup,
};
pub(crate) use frame_clock::{AdaptiveFrameClock, FrameClockSnapshot};
