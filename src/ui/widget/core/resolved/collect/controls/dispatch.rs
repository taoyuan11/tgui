use super::*;

mod basic;
mod value;

impl<VM> ResolvedElement<VM> {
    pub(in super::super) fn collect_control_kind(
        &self,
        context: &mut CollectContext<'_, '_>,
        computed: &mut ComputedScene<VM>,
        visual: &CollectVisualState,
    ) -> bool {
        match &self.kind {
            ResolvedWidgetKind::Button { .. } => {
                self.collect_button_control(context, computed, visual)
            }
            ResolvedWidgetKind::Checkbox { .. } => {
                self.collect_checkbox_control(context, computed, visual)
            }
            ResolvedWidgetKind::Radio { .. } => {
                self.collect_radio_control(context, computed, visual)
            }
            ResolvedWidgetKind::Switch { .. } => {
                self.collect_switch_control(context, computed, visual)
            }
            ResolvedWidgetKind::Slider { .. } => {
                self.collect_slider_control(context, computed, visual)
            }
            ResolvedWidgetKind::Select { .. } => {
                self.collect_select_control(context, computed, visual)
            }
            ResolvedWidgetKind::TextEditor { .. } => {
                self.collect_text_editor_control(context, computed, visual)
            }
            _ => false,
        }
    }
}
