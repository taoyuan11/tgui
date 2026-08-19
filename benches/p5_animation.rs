use std::rc::Rc;
use std::time::{Duration, Instant};

use tgui::animation::{Animated, AnimationImpact, AnimationKey, AnimationSpec, Timeline};
use tgui::core::{ElementId, PropertyId};
use tgui::test_support::FakeClock;

fn main() {
    let clock = Rc::new(FakeClock::new());
    let mut timeline = Timeline::new(clock.clone());
    let value = Animated::new(0.0_f32);
    let started = Instant::now();
    for index in 0..10_000_u64 {
        timeline.animate(
            AnimationKey::new(ElementId::from_parts(index as u32, 1), PropertyId::new(1)),
            &value,
            1.0,
            AnimationSpec::new(Duration::from_millis(100), AnimationImpact::Paint),
        );
    }
    let setup = started.elapsed();
    let tick_started = Instant::now();
    for _ in 0..10 {
        clock.advance(Duration::from_millis(10)).unwrap();
        let _ = timeline.tick();
    }
    let ticks = tick_started.elapsed();
    assert!(timeline.is_idle() || timeline.running_animation_count() <= 10_000);
    println!("animations=10000 setup={setup:?} ten_ticks={ticks:?}");
}
