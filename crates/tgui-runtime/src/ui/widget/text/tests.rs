use crate::animation::AnimationCoordinator;
use crate::foundation::binding::{InvalidationSignal, ViewModelContext};

use super::Text;

fn test_context() -> ViewModelContext {
    ViewModelContext::new(InvalidationSignal::new(), AnimationCoordinator::default())
}

#[test]
fn text_new_accepts_display_values() {
    let text = Text::new(42);

    assert_eq!(text.content.resolve(), "42");
}

#[test]
fn text_new_accepts_display_signals() {
    let ctx = test_context();
    let count = ctx.state(7);
    let text = Text::new(count.signal());

    assert_eq!(text.content.resolve(), "7");
    count.set(12);
    assert_eq!(text.content.resolve(), "12");
}
