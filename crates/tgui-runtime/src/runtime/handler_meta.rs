use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(in crate::runtime) fn with_view_model<R>(&self, f: impl FnOnce(&mut VM) -> R) -> R {
        let mut view_model = self.view_model.lock().expect("view model lock poisoned");
        f(&mut view_model)
    }

    pub(in crate::runtime) fn set_definition(
        &mut self,
        role: WindowRole,
        config: ApplicationConfig,
        window_bindings: WindowBindings,
        root_view: Option<RootViewFactory<VM>>,
        commands: Vec<WindowCommand<VM>>,
        close_policy: WindowClosePolicy,
    ) {
        self.role = role;
        let font_manager = Arc::new(FontManager::new(&config.fonts));
        if let Some(window) = self.window.as_ref() {
            if window.is_decorated() != config.decorations {
                window.set_decorations(config.decorations);
            }
        }
        self.config = config;
        self.font_manager = font_manager;
        self.window_bindings = window_bindings;
        self.root_view = root_view;
        self.commands = commands;
        self.close_policy = close_policy;
    }

    pub(in crate::runtime) fn rebuild_widget_tree_from_root_view(&mut self) -> bool {
        let Some(root_view) = self.root_view.clone() else {
            return false;
        };
        let tree = {
            let view_model = self.view_model.lock().expect("view model lock poisoned");
            WidgetTree::new(build_root_element(&root_view, &view_model))
        };
        self.widget_tree = Some(tree);
        self.portal_publication_generation = self.portal_publication_generation.wrapping_add(1);
        self.cached_scene = None;
        self.clear_rebuilt_tree_runtime_state();
        true
    }

    pub(in crate::runtime) fn observe_root_rebuild_request(&mut self) -> bool {
        let revision = self.invalidation.root_rebuild_revision();
        if revision == self.last_root_rebuild_revision {
            return false;
        }
        self.last_root_rebuild_revision = revision;
        self.rebuild_widget_tree_from_root_view()
    }

    fn clear_rebuilt_tree_runtime_state(&mut self) {
        self.hovered_widgets.clear();
        self.button_hover_patch_pending = None;
        self.hover_patch_pending = None;
        self.button_pressed_patch_pending = None;
        self.row_hover_patch_pending = None;
        self.tooltip_hover_started_at.clear();
        self.next_tooltip_wakeup_deadline = None;
        self.next_toast_wakeup_deadline = None;
        self.next_carousel_wakeup_deadline = None;
        self.tooltip_state.active = None;
        self.tooltip_state.hover_suppressed = None;
        self.tooltip_state.focus_suppressed = None;
        self.tooltip_state.long_press_suppressed = None;
        self.tooltip_state.long_press_candidate = None;
        self.tooltip_state.long_press_release_deadline = None;
        self.hover_popover_anchor = None;
        self.menu_open_states.clear();
        self.menubar_active_states.clear();
        self.context_menu_anchor_states.clear();
        self.menu_keyboard_cursor.clear();
        self.list_anchor_states.clear();
        self.list_focus_state = None;
        self.tree_anchor_states.clear();
        self.tree_focus_state = None;
        self.data_grid_anchor_states.clear();
        self.data_grid_focus_state = None;
        self.hovered_scrollbar = None;
        self.active_scrollbar_drag = None;
        self.active_touch_scroll = None;
        self.active_gesture = None;
        self.active_pinch = None;
        self.active_slider_drag = None;
        self.active_canvas_drag = None;
        self.active_tab_reorder = None;
        self.active_tree_drag = None;
        self.active_data_grid_column_resize = None;
        self.active_splitter_resize = None;
        self.active_data_grid_column_reorder = None;
        self.carousel_auto_play_last.clear();
        self.active_key_repeat = None;
        self.pending_click = None;
        self.deferred_mouse_click = None;
        self.pressed_widget = None;
        self.focused_widget = None;
        self.focus_visible = false;
        self.active_auto_focus_scope = None;
        self.selected_text = None;
        self.text_edit_states.clear();
        self.text_input_buffers.clear();
        self.text_input_regions.clear();
        self.text_input_flush_data.clear();
        self.active_text_selection = None;
        self.caret_blink_origin = Instant::now();
        self.cursor_icon = None;
        self.scroll_states.clear();
        self.smooth_scroll_states.clear();
        self.touch_scroll_inertia_states.clear();
        self.virtual_states.clear();
        self.select_open_states.clear();
        self.scroll_dirty_widgets.clear();
    }

    pub(in crate::runtime) fn close_policy(&self) -> WindowClosePolicy {
        self.close_policy
    }

    pub(in crate::runtime) fn is_main_window(&self) -> bool {
        matches!(self.role, WindowRole::Main)
    }

    pub(in crate::runtime) fn blocks_main_window(&self) -> bool {
        matches!(
            self.role,
            WindowRole::Child {
                blocks_main_window: true
            }
        )
    }

    pub(in crate::runtime) fn fail(&mut self, event_loop: &dyn ActiveEventLoop, error: TguiError) {
        Log::with_tag("tgui-runtime").error(format_args!("bound runtime failed: {error}"));
        self.error = Some(error);
        event_loop.exit();
    }
}
