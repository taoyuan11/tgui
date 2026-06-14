use super::resolved_freeze::lifecycle_snapshot;
use super::*;
use crate::ui::widget::r#virtual::{apply_virtual_runtime_state_to_element, VirtualViewportHint};
use crate::ui::widget::FocusScopeState;

mod chrome;
mod controls;
mod drawer;
mod layout_media;
mod menu;
mod modal;
mod popover;
pub(crate) mod portal;
mod toast;
mod tooltip;

struct CollectResolvedStyles {
    button_style: Option<ResolvedButtonStyle>,
    select_style: Option<ResolvedSelectStyle>,
    slider_style: Option<ResolvedSliderStyle>,
    progress_bar_style: Option<crate::ui::widget::style::ProgressBarStyle>,
    spinner_style: Option<crate::ui::widget::style::SpinnerStyle>,
    divider_style: Option<crate::ui::widget::style::DividerStyle>,
    switch_style: Option<crate::ui::widget::style::SwitchStyle>,
    input_style: Option<ResolvedInputStyle>,
    checkbox_style: Option<ResolvedCheckboxStyle>,
    radio_style: Option<ResolvedRadioStyle>,
}

struct CollectVisualState {
    frame: Rect,
    background_frame: Rect,
    background_radius: Dp,
    runtime_visual: VisualStyle,
    primitive_clip: Option<Rect>,
    overflow_clip: Option<Rect>,
    primitive_clip_mask: Option<ClipMask>,
    disabled: bool,
    widget_state: WidgetState,
    opacity: f32,
    border_width: Dp,
    border_radius: Dp,
    border_color: Color,
    background: Color,
    styles: CollectResolvedStyles,
}

struct CollectCaches<'a, VM> {
    lifecycle_states: &'a mut HashMap<WidgetId, LifecycleEventState<VM>>,
    chunks: &'a mut HashMap<WidgetId, ComputedScene<VM>>,
    chunk_parts: &'a mut HashMap<WidgetId, SceneChunkParts<VM>>,
    visual_contexts: &'a mut HashMap<WidgetId, VisualContextSnapshot>,
}

pub(super) fn prepare_nested_scene_root<VM>(
    root: &mut Element<VM>,
    context: &CollectContext<'_, '_>,
    fallback_viewport: Rect,
) {
    apply_virtual_runtime_state_to_element(
        root,
        context.scroll_offsets,
        context.virtual_states,
        VirtualViewportHint {
            width: fallback_viewport.width,
            height: fallback_viewport.height,
        },
    );
}

impl<VM: 'static> ResolvedElement<VM> {
    pub(super) fn can_skip_when_fully_clipped(&self) -> bool {
        if !self.clip_cull_safe_self() {
            return false;
        }

        match &self.kind {
            ResolvedWidgetKind::Text { text, .. } => !text.user_select,
            ResolvedWidgetKind::Container {
                layout, children, ..
            } => {
                layout.overflow_x != Overflow::Scroll
                    && layout.overflow_y != Overflow::Scroll
                    && children
                        .iter()
                        .all(ResolvedElement::can_skip_when_fully_clipped)
            }
            _ => false,
        }
    }

    fn clip_cull_safe_self(&self) -> bool {
        let has_runtime_overlay = self.tooltip.is_some()
            || self.popover.is_some()
            || self.menu.is_some()
            || self.context_menu.is_some()
            || self.modal.is_some()
            || self.drawer.is_some();
        let has_runtime_role = self.tab_trigger.is_some()
            || self.list_item.is_some()
            || self.tree_root.is_some()
            || self.tree_node.is_some()
            || self.data_grid_root.is_some()
            || self.data_grid_cell.is_some()
            || self.data_grid_header.is_some()
            || self.data_grid_resize_handle.is_some()
            || self.splitter_handle.is_some()
            || self.carousel_auto_play.is_some();

        !self.interactions.has_any()
            && !self.lifecycle_events.has_any()
            && !self.media_events.has_any()
            && self.focus.focusable.is_none()
            && self.focus.tab_index.is_none()
            && self.focus.scope.is_none()
            && !has_runtime_overlay
            && !has_runtime_role
            && self.visual.shadow.is_none()
            && matches!(&self.visual.background_blur, Value::Static(value) if *value == Dp::ZERO)
            && matches!(&self.visual.offset, Value::Static(value) if *value == Point::ZERO)
            && matches!(&self.visual.scale, Value::Static(value) if *value == 1.0)
    }

    /// 收集 `self` 子树的场景 chunk,把结果**移动**进 `chunks[self.id]`,
    /// 并返回该节点的 `WidgetId`。
    ///
    /// 旧实现这里返回整棵合并后的子树(owned `ComputedScene`),再被父节点 `extend`
    /// 后丢弃 —— 也就是说每个节点都把自己的合并子树深拷贝了两次(一次进 `chunks`,
    /// 一次作为返回值)。现在结果只在 `chunks` 里存一份:
    /// - 父节点通过 `chunks.get(&child.id)` 只读引用来 `extend`,不再深拷贝;
    /// - 需要 owned 场景的根/overlay 调用方在收集结束后 `chunks.get(&id).cloned()`,
    ///   仅根节点保留这一次必要的克隆。
    pub(in super::super) fn collect_subtree_cache(
        &self,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
        lifecycle_states: &mut HashMap<WidgetId, LifecycleEventState<VM>>,
        chunks: &mut HashMap<WidgetId, ComputedScene<VM>>,
        chunk_parts: &mut HashMap<WidgetId, SceneChunkParts<VM>>,
        visual_contexts: &mut HashMap<WidgetId, VisualContextSnapshot>,
    ) -> WidgetId {
        super::super::tree::with_widget_stack_frame(|| {
            let owner = self.id.dependency_owner(DependencyPhase::Scene);
            track_dependency_scope(owner, || {
                self.collect_subtree_cache_tracked(
                    layout_node,
                    visual_context,
                    context,
                    lifecycle_states,
                    chunks,
                    chunk_parts,
                    visual_contexts,
                )
            })
        })
    }

    fn collect_subtree_cache_tracked(
        &self,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
        lifecycle_states: &mut HashMap<WidgetId, LifecycleEventState<VM>>,
        chunks: &mut HashMap<WidgetId, ComputedScene<VM>>,
        chunk_parts: &mut HashMap<WidgetId, SceneChunkParts<VM>>,
        visual_contexts: &mut HashMap<WidgetId, VisualContextSnapshot>,
    ) -> WidgetId {
        let previous_focus = context.focus.clone();
        if let Some(scope) = self.focus.scope.as_ref() {
            let active = scope.is_active();
            context.focus.scope_path.push(self.id);
            if !active {
                context.focus.disabled_depth += 1;
            }
        }
        let mut caches = CollectCaches {
            lifecycle_states,
            chunks,
            chunk_parts,
            visual_contexts,
        };
        self.collect_runtime_lifecycle_state(caches.lifecycle_states);

        let mut computed = ComputedScene::default();
        computed
            .scene
            .set_active_gpu_scroll_container(context.gpu_scroll_container);
        if let Some(scope) = self.focus.scope.as_ref() {
            computed.register_focus_scope(FocusScopeState {
                scope_id: self.id,
                path: context.focus.scope_path.clone(),
                options: scope.clone(),
                active: scope.is_active(),
            });
        }
        use super::super::collect_profile::{record_node, record_node_visible, timed, Phase};
        record_node();
        let visual = timed(Phase::VisualState, || {
            self.resolve_collect_visual_state(layout_node, visual_context, context)
        });
        // 节点 frame 与视口相交即记一次「可见」，配合 `record_node` 给出 recollect/visible 比值。
        if visual.frame.intersect(context.viewport).is_some() {
            record_node_visible();
        }
        timed(Phase::Surface, || {
            self.push_surface_primitives_and_base_hit_regions(&mut computed, context, &visual)
        });
        if let Some(auto_play) = self.carousel_auto_play.as_ref() {
            let mut auto_play = auto_play.clone();
            auto_play.frame = visual.frame;
            computed.carousel_auto_play.push(auto_play);
        }

        let kind_handled = timed(Phase::KindBody, || {
            self.collect_layout_media_kind(
                layout_node,
                visual_context,
                context,
                &mut caches,
                &mut computed,
                &visual,
            )
        });
        if !kind_handled {
            let handled = self.collect_control_kind(context, &mut computed, &visual);
            debug_assert!(
                handled,
                "unhandled widget kind in collect_subtree_cache_tracked"
            );
        }
        self.clear_closed_modal_interactions(&mut computed);
        // `before_overlays` 仅在 `Container`/`Virtual` 节点上被消费(用于把 overlay 增量
        // 并入 `chunk_parts.after_children`)。叶子节点占节点总数绝大多数,为它们克隆整份
        // `computed` 是纯浪费 —— 因此把这次快照限定到带子节点的 kind。
        let is_container_like = matches!(
            self.kind,
            ResolvedWidgetKind::Container { .. } | ResolvedWidgetKind::Virtual { .. }
        );
        let before_overlays = is_container_like.then(|| computed.clone());

        self.emit_tooltip_if_visible(context, &mut computed, &visual);
        self.emit_popover_overlay_if_visible(context, &mut computed, &visual);
        self.emit_menu_overlay_if_open(context, &mut computed, &visual);
        self.emit_modal_close_overlay_if_open(context, &mut computed, &visual);
        self.emit_drawer_close_overlay_if_open(context, &mut computed, &visual);
        self.emit_toast_overlay_if_visible(context, &mut computed, &visual);
        self.emit_portal_if_open(context, &mut computed, &visual);

        if let Some(before_overlays) = before_overlays {
            let overlay_delta = computed.delta_since(&before_overlays);
            if let Some(parts) = caches.chunk_parts.get_mut(&self.id) {
                parts.after_children.extend(&overlay_delta);
            }
        }

        timed(Phase::Bookkeeping, || {
            // `chunk_parts` 只被 `recompose_scene_chunk` 读取(scene_layout.rs),而 recompose
            // 仅在「祖先」节点上运行(scene_patch.rs / bench_support.rs 都由 `parent_of` 向上推导
            // 祖先集合),且仅对 `Container` / `Virtual` 这两种带子节点的 kind 迭代子 chunk。
            // 其余 kind 在 resolved 树里都是叶子,永远不会成为祖先 → 它们的 `chunk_parts` 是
            // 只写不读的死数据。而 `Container` / `Virtual` 收集臂(layout_media.rs)又各自显式
            // `insert` 了自己的 `chunk_parts`,所以这里的兜底 `or_insert_with` 对它们也已是空操作。
            //
            // 因此把兜底克隆限定到带子节点的 kind:对叶子(占节点总数绝大多数,且每个都要
            // `before_children: computed.clone()`)直接跳过,消除逐帧重收集里最大的单项独占开销
            // (n=1000 长列表约 22%)。即便未来出现意外读取,`recompose` 的 `chunk_parts.get(&id)?`
            // 会得到 `None` 并安全回退到整帧重收集,绝不会产生错误渲染。
            if is_container_like {
                caches
                    .chunk_parts
                    .entry(self.id)
                    .or_insert_with(|| SceneChunkParts {
                        before_children: computed.clone(),
                        after_children: ComputedScene::default(),
                    });
            }
            if let Some(gpu_scroll_container) = context.gpu_scroll_container {
                computed.fill_gpu_scroll_container(gpu_scroll_container);
                if let Some(parts) = caches.chunk_parts.get_mut(&self.id) {
                    parts
                        .before_children
                        .fill_gpu_scroll_container(gpu_scroll_container);
                    parts
                        .after_children
                        .fill_gpu_scroll_container(gpu_scroll_container);
                }
            }
            caches
                .visual_contexts
                .insert(self.id, visual_context.into());
            context.focus = previous_focus;
            // 把合并后的子树移动进 chunks(此前是 `clone` + 返回 owned 的双份拷贝)。
            // 父节点改为 `chunks.get(&child.id)` 只读引用来 extend。
            caches.chunks.insert(self.id, computed);
        });
        self.id
    }

    fn collect_runtime_lifecycle_state(
        &self,
        lifecycle_states: &mut HashMap<WidgetId, LifecycleEventState<VM>>,
    ) {
        if self.lifecycle_events.has_any() || self.requires_runtime_lifecycle() {
            lifecycle_states.insert(
                self.id,
                LifecycleEventState {
                    widget_id: self.id,
                    snapshot: lifecycle_snapshot(self),
                    handlers: self.lifecycle_events.clone(),
                },
            );
        }
    }
}
