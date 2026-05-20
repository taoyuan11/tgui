use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(in crate::runtime) fn focused_text_input_id(&mut self) -> Option<WidgetId> {
        let focused = self.focused_widget_id()?;
        let computed = self.computed_scene();
        computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .find_map(|region| match &region.interaction {
                crate::ui::widget::HitInteraction::TextInput { id, .. } if *id == focused => {
                    Some(*id)
                }
                _ => None,
            })
    }

    pub(in crate::runtime) fn ime_request_data_for_text_input(
        &mut self,
    ) -> Option<crate::platform::window::ImeRequestData> {
        let id = self.focused_text_input_id()?;
        let region = {
            let computed = self.computed_scene();
            let ime_cursor_area = computed.ime_cursor_area;
            computed
                .hit_regions
                .iter()
                .chain(computed.overlay_hit_regions.iter())
                .find_map(|region| match &region.interaction {
                    crate::ui::widget::HitInteraction::TextInput {
                        id: hit_id,
                        controller,
                        ..
                    } if *hit_id == id => Some((ime_cursor_area, controller.clone())),
                    _ => None,
                })?
        };
        let text = self
            .text_input_buffers
            .get(&id)
            .map(|session| session.current_text.clone())
            .unwrap_or_else(|| region.1.text());
        let state = self
            .text_edit_state(id)
            .cloned()
            .unwrap_or_else(|| self.default_text_edit_state(id, &text));
        let surrounding = ImeSurroundingText::new(text, state.cursor, state.anchor).ok();
        let mut data = crate::platform::window::ImeRequestData::default()
            .with_hint_and_purpose(ImeHint::NONE, ImePurpose::Normal);
        if let Some(rect) = region.0 {
            let cursor = Self::ime_cursor_request_data(rect, self.unit_context());
            if let Some((position, size)) = cursor.cursor_area {
                data = data.with_cursor_area(position, size);
            }
        }
        if let Some(surrounding) = surrounding {
            data = data.with_surrounding_text(surrounding);
        }
        Some(data)
    }

    pub(in crate::runtime) fn sync_ime_state(&mut self) {
        #[cfg(all(target_os = "android", feature = "android"))]
        {
            self.sync_android_text_input_state();
        }
        #[cfg(not(all(target_os = "android", feature = "android")))]
        {
            if let Some(request_data) = self.ime_request_data_for_text_input() {
                let capabilities = ImeCapabilities::new()
                    .with_hint_and_purpose()
                    .with_cursor_area();
                let capabilities = if request_data.surrounding_text.is_some() {
                    capabilities.with_surrounding_text()
                } else {
                    capabilities
                };
                if let Some(enable) = ImeEnableRequest::new(capabilities, request_data.clone()) {
                    if let Some(window) = self.window.as_ref() {
                        let _ = window.request_ime_update(ImeRequest::Enable(enable));
                    }
                }
                if let Some(window) = self.window.as_ref() {
                    let _ = window.request_ime_update(ImeRequest::Update(request_data));
                }
            } else if let Some(window) = self.window.as_ref() {
                let _ = window.request_ime_update(ImeRequest::Disable);
            }
        }
    }
}
