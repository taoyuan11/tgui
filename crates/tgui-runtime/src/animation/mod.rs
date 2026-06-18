mod controller;
mod engine;
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

pub(crate) use controller::AnimationCoordinator;
pub(crate) use engine::{
    default_theme_transition, AnimationEngine, AnimationKey, WidgetProperty, WindowProperty,
};
