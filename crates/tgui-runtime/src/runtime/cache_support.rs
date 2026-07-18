use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(in crate::runtime) fn scroll_mismatch_requires_layout_rebuild(
        &self,
        cached: &CachedScene<VM>,
    ) -> bool {
        if cached.scroll_epoch == self.scroll_epoch {
            return false;
        }

        let Some(layout) = cached.layout.as_ref() else {
            return self
                .widget_tree
                .as_ref()
                .map(|tree| tree.has_virtual())
                .unwrap_or(false);
        };

        if self.scroll_dirty_widgets.is_empty() {
            return layout.contains_virtual();
        }

        self.scroll_dirty_widgets.iter().any(|widget_id| {
            matches!(
                layout
                    .resolved_widget(*widget_id)
                    .map(|resolved| &resolved.kind),
                Some(crate::ui::widget::ResolvedWidgetKind::Virtual { .. }) | None
            )
        })
    }

    pub(in crate::runtime) fn scene_cache_matches(
        &self,
        cached: &CachedScene<VM>,
        viewport: Rect,
        units: UnitContext,
        caret_visible: bool,
        active_scrollbar: Option<ScrollbarHandle>,
    ) -> bool {
        if !cached.computed_valid {
            return false;
        }
        cached.scroll_epoch == self.scroll_epoch
            && self.scene_cache_fields_match_ignoring_scroll(
                cached,
                viewport,
                units,
                caret_visible,
                active_scrollbar,
            )
    }

    /// 除 `scroll_epoch`（与 `computed_valid`）外的所有场景缓存匹配字段。
    /// `scene_cache_matches` 与「纯滚动帧」检测共用，避免字段列表漂移。
    pub(in crate::runtime) fn scene_cache_fields_match_ignoring_scroll(
        &self,
        cached: &CachedScene<VM>,
        viewport: Rect,
        units: UnitContext,
        caret_visible: bool,
        active_scrollbar: Option<ScrollbarHandle>,
    ) -> bool {
        cached.hover_epoch == self.hover_epoch
            && self.scene_cache_fields_match_ignoring_scroll_and_hover(
                cached,
                viewport,
                units,
                caret_visible,
                active_scrollbar,
            )
    }

    pub(in crate::runtime) fn scene_cache_fields_match_ignoring_scroll_and_hover(
        &self,
        cached: &CachedScene<VM>,
        viewport: Rect,
        units: UnitContext,
        caret_visible: bool,
        active_scrollbar: Option<ScrollbarHandle>,
    ) -> bool {
        cached.pressed_widget == self.pressed_widget
            && self.scene_cache_fields_match_ignoring_scroll_hover_and_pressed(
                cached,
                viewport,
                units,
                caret_visible,
                active_scrollbar,
            )
    }

    pub(in crate::runtime) fn scene_cache_fields_match_ignoring_scroll_hover_and_pressed(
        &self,
        cached: &CachedScene<VM>,
        viewport: Rect,
        units: UnitContext,
        caret_visible: bool,
        active_scrollbar: Option<ScrollbarHandle>,
    ) -> bool {
        cached.viewport == viewport
            && cached.units == units
            && cached.focused_widget == self.focused_widget_id()
            && cached.focus_visible == self.focus_visible
            && cached.selected_text == self.selected_text
            && cached.caret_visible == caret_visible
            && cached.theme_epoch == self.theme_store.version()
            && cached.style_sheet_version == self.config.style_sheet.version()
            && cached.density == self.theme.density
            && cached.reduced_motion == self.reduced_motion
            && cached.text_scale_bits == units.font_scale().to_bits()
            && cached.animation_epoch == self.animation_epoch
            && cached.layout_animation_epoch == self.layout_animation_epoch
            && cached.text_input_epoch == self.text_input_epoch
            && cached.external_portal_revision == self.external_portal_revision
            && cached.hovered_scrollbar == self.hovered_scrollbar
            && cached.active_scrollbar == active_scrollbar
    }

    pub(in crate::runtime) fn scene_layout_cache_matches(
        &self,
        cached: &CachedScene<VM>,
        viewport: Rect,
        units: UnitContext,
    ) -> bool {
        let virtual_scroll_matches = !self.scroll_mismatch_requires_layout_rebuild(cached);
        cached.layout_valid
            && cached.viewport == viewport
            && cached.units == units
            && cached.theme_epoch == self.theme_store.version()
            && cached.style_sheet_version == self.config.style_sheet.version()
            && cached.density == self.theme.density
            && cached.reduced_motion == self.reduced_motion
            && cached.text_scale_bits == units.font_scale().to_bits()
            && cached.layout_animation_epoch == self.layout_animation_epoch
            && virtual_scroll_matches
    }

    pub(in crate::runtime) fn scene_cache_mismatch_summary(
        &self,
        cached: &CachedScene<VM>,
        viewport: Rect,
        units: UnitContext,
        caret_visible: bool,
        active_scrollbar: Option<ScrollbarHandle>,
    ) -> String {
        let mut reasons = Vec::new();
        if !cached.computed_valid {
            reasons.push("computed_valid");
        }
        if !cached.layout_valid {
            reasons.push("layout_valid");
        }
        if cached.viewport != viewport {
            reasons.push("viewport");
        }
        if cached.units != units {
            reasons.push("units");
        }
        if cached.focused_widget != self.focused_widget_id() {
            reasons.push("focused_widget");
        }
        if cached.focus_visible != self.focus_visible {
            reasons.push("focus_visible");
        }
        if cached.pressed_widget != self.pressed_widget {
            reasons.push("pressed_widget");
        }
        if cached.selected_text != self.selected_text {
            reasons.push("selected_text");
        }
        if cached.caret_visible != caret_visible {
            reasons.push("caret_visible");
        }
        if cached.theme_epoch != self.theme_store.version() {
            reasons.push("theme_epoch");
        }
        if cached.style_sheet_version != self.config.style_sheet.version() {
            reasons.push("style_sheet_version");
        }
        if cached.density != self.theme.density {
            reasons.push("density");
        }
        if cached.reduced_motion != self.reduced_motion {
            reasons.push("reduced_motion");
        }
        if cached.text_scale_bits != units.font_scale().to_bits() {
            reasons.push("text_scale");
        }
        if cached.animation_epoch != self.animation_epoch {
            reasons.push("animation_epoch");
        }
        if cached.layout_animation_epoch != self.layout_animation_epoch {
            reasons.push("layout_animation_epoch");
        }
        if cached.scroll_epoch != self.scroll_epoch {
            reasons.push("scroll_epoch");
        }
        if cached.hover_epoch != self.hover_epoch {
            reasons.push("hover_epoch");
        }
        if cached.text_input_epoch != self.text_input_epoch {
            reasons.push("text_input_epoch");
        }
        if cached.external_portal_revision != self.external_portal_revision {
            reasons.push("external_portal_revision");
        }
        if cached.hovered_scrollbar != self.hovered_scrollbar {
            reasons.push("hovered_scrollbar");
        }
        if cached.active_scrollbar != active_scrollbar {
            reasons.push("active_scrollbar");
        }
        if self.scroll_mismatch_requires_layout_rebuild(cached) {
            reasons.push("virtual_scroll_epoch");
        }
        if reasons.is_empty() {
            "none".to_string()
        } else {
            reasons.join("|")
        }
    }

    pub(in crate::runtime) fn can_patch_text_input_scene(
        &self,
        cached: &CachedScene<VM>,
        viewport: Rect,
        units: UnitContext,
        caret_visible: bool,
        active_scrollbar: Option<ScrollbarHandle>,
    ) -> bool {
        let focused_input = self.focused_text_input_id_cached(&cached.computed);
        let stable_shell = focused_input.is_some()
            && cached.computed_valid
            && cached.layout_valid
            && cached.layout.is_some()
            && cached.viewport == viewport
            && cached.units == units
            && cached.focused_widget == self.focused_widget_id()
            && cached.focus_visible == self.focus_visible
            && cached.pressed_widget == self.pressed_widget
            && cached.selected_text == self.selected_text
            && cached.theme_epoch == self.theme_store.version()
            && cached.style_sheet_version == self.config.style_sheet.version()
            && cached.density == self.theme.density
            && cached.reduced_motion == self.reduced_motion
            && cached.text_scale_bits == units.font_scale().to_bits()
            && cached.animation_epoch == self.animation_epoch
            && cached.layout_animation_epoch == self.layout_animation_epoch
            && cached.hover_epoch == self.hover_epoch
            && cached.external_portal_revision == self.external_portal_revision
            && cached.hovered_scrollbar == self.hovered_scrollbar
            && cached.active_scrollbar == active_scrollbar;
        stable_shell
            && (cached.text_input_epoch != self.text_input_epoch
                || cached.caret_visible != caret_visible)
    }

    pub(in crate::runtime) fn visible_text_input_roots_from_computed(
        computed: &ComputedScene<VM>,
    ) -> Vec<WidgetId> {
        let mut ids = HashSet::new();
        for region in computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
        {
            if let crate::ui::widget::HitInteraction::TextInput { id, .. } = &region.interaction {
                ids.insert(*id);
            }
        }
        ids.into_iter().collect()
    }
}
