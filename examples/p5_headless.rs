use std::rc::Rc;
use std::time::Duration;

use tgui::animation::{Animated, AnimationImpact, AnimationKey, AnimationSpec};
use tgui::core::{DpiScale, ItemKey, PropertyId, Size};
use tgui::test_support::FakeClock;
use tgui::virtualization::{VirtualList, VirtualListDataSource};
use tgui::widget::{BuildContext, OPACITY, Widget, WidgetNode};
use tgui::widgets::Button;

struct Rows(usize);
struct Row;

impl VirtualListDataSource for Rows {
    fn len(&self) -> usize {
        self.0
    }

    fn item_key(&self, index: usize) -> ItemKey {
        ItemKey::numeric(index as u64)
    }

    fn build_item(
        &self,
        _index: usize,
        _key: &ItemKey,
        _context: &mut BuildContext,
    ) -> tgui::Result<WidgetNode> {
        Ok(WidgetNode::new::<Row>())
    }
}

fn main() -> tgui::Result<()> {
    let clock = Rc::new(FakeClock::new());
    let mut application = tgui::Application::with_frame_clock(clock.clone());
    let window = application.create_window(tgui::WindowSpec::new("p5-headless"))?;
    let mut context = BuildContext::new();
    let button = Button::new("Animated").build(&mut context)?;
    let report = application.mount_widget(window, button)?;
    let element = report
        .invalidations()
        .next()
        .map(|invalidation| invalidation.element())
        .expect("mounted button has an element");
    let presentation = Animated::new(1.0_f32);
    application.animate(
        window,
        AnimationKey::new(element, OPACITY),
        &presentation,
        0.25,
        AnimationSpec::new(Duration::from_millis(100), AnimationImpact::Paint),
    )?;
    clock.advance(Duration::from_millis(50))?;
    let animation = application.tick_animations(window)?;
    application.layout_window(window)?;
    let frame = application.render_window(window)?;

    let mut list = VirtualList::new(Rows(100_000), 20.0)?;
    list.set_viewport(240.0, 0.0)?;
    list.set_scroll_offset(40_000.0)?;
    let list_metrics = list.metrics();
    application.record_virtualization_metrics(window, list_metrics.into())?;
    println!(
        "animation_active={} sampled={} opacity={:.2} materialized={}/{} peak={} scene_fingerprint={}",
        animation.metrics.active,
        animation.metrics.sampled,
        presentation.value(),
        list_metrics.materialized_items,
        list_metrics.total_items,
        list_metrics.peak_materialized_items,
        frame.scene.fingerprint(),
    );
    let _ = (DpiScale::ONE, Size::new(1.0, 1.0), PropertyId::new(0));
    Ok(())
}
