use super::*;
use crate::runtime::state::CaretDecorationBinding;
use crate::ui::widget::{OverlayTextDecorationPrimitiveSlot, RenderCommand};

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn rebuild_caret_decoration_binding(&mut self) {
        let Some(cached) = self.cached_scene.as_mut() else {
            return;
        };
        cached.caret_decoration = find_caret_decoration_binding(&cached.computed);
    }

    pub(super) fn try_update_caret_visibility_slot(&mut self, caret_visible: bool) -> bool {
        let Some(cached) = self.cached_scene.as_mut() else {
            return false;
        };
        if cached.caret_visible == caret_visible && cached.caret_decoration.is_none() {
            return true;
        }
        let Some(binding) = cached.caret_decoration else {
            return false;
        };
        let color = if caret_visible {
            binding.visible_color
        } else {
            binding.visible_color.with_alpha_factor(0.0)
        };
        let Some(mut decoration) = cached
            .computed
            .scene
            .overlay_text_decorations
            .get(binding.overlay_text_decoration_index)
            .cloned()
        else {
            return false;
        };
        if decoration.color == color {
            cached.caret_visible = caret_visible;
            return true;
        }
        decoration.color = color;
        if !cached.computed.scene.write_overlay_text_decoration_slot(
            &SceneCounts::default(),
            OverlayTextDecorationPrimitiveSlot {
                text_decoration_index: binding.overlay_text_decoration_index,
                command_index: binding.overlay_command_index,
            },
            decoration,
        ) {
            return false;
        }
        cached.caret_visible = caret_visible;
        super::action_stats::record("caret_visibility_slot_write");
        true
    }
}

fn find_caret_decoration_binding<VM>(
    computed: &ComputedScene<VM>,
) -> Option<CaretDecorationBinding> {
    let caret = computed.ime_cursor_area?;
    let mut decoration_index = 0usize;
    for (command_index, command) in computed.scene.overlay_commands.iter().enumerate() {
        let RenderCommand::TextDecoration(decoration) = command else {
            continue;
        };
        let is_caret = decoration.segments.len() == 1
            && decoration.segments.first().copied() == Some(caret)
            && decoration.stroke_width == 0.0;
        if is_caret {
            return Some(CaretDecorationBinding {
                overlay_text_decoration_index: decoration_index,
                overlay_command_index: command_index,
                visible_color: decoration.color,
            });
        }
        decoration_index += 1;
    }
    None
}
