use super::scene::ActiveTooltipState;
use super::*;
use crate::ui::widget::r#virtual::{
    apply_virtual_runtime_state_to_element, VirtualCacheState, VirtualViewportHint,
};
use std::time::Instant;

pub struct WidgetTree<VM> {
    pub(super) root: std::sync::Arc<Element<VM>>,
    /// 树内是否存在 Virtual 节点。无虚拟节点时全量重建可直接共享源树（Arc clone），
    /// 跳过整棵树的深拷贝。
    pub(super) has_virtual: bool,
}

/// 在递归走 widget 树的入口（root collect / root layout / overlay 子场景）使用，
/// 一次性预留 8MB 备用栈，避免 debug 构建中每层 ~数十 KB 的局部把默认 1MB 栈打爆。
pub(crate) fn with_widget_stack<R>(f: impl FnOnce() -> R) -> R {
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    {
        const WIDGET_STACK_SIZE: usize = 8 * 1024 * 1024;
        const WIDGET_STACK_RED_ZONE: usize = WIDGET_STACK_SIZE;
        return stacker::maybe_grow(WIDGET_STACK_RED_ZONE, WIDGET_STACK_SIZE, f);
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        f()
    }
}

/// 每层递归调用都会过一次的轻量栈守护：仅当剩余栈低于红区阈值时才分配额外段。
/// 用于 `Element::resolve_with_previous` / `ResolvedElement::build_layout_tree` /
/// `ResolvedElement::collect_subtree_cache` 等递归热点，配合 taffy measure 回调
/// 等会重入到 widget 树构建链路的场景，保证栈在意外深度下也能动态扩展。
pub(crate) fn with_widget_stack_frame<R>(f: impl FnOnce() -> R) -> R {
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    {
        const FRAME_RED_ZONE: usize = 1024 * 1024;
        const FRAME_GROW_SIZE: usize = 4 * 1024 * 1024;
        return stacker::maybe_grow(FRAME_RED_ZONE, FRAME_GROW_SIZE, f);
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        f()
    }
}

/// 检测树内是否存在「会被运行时状态注入触及」的 Virtual 节点。遍历方式与
/// [`apply_virtual_runtime_state_to_element`] 完全一致：仅下钻 Container 的静态
/// 子节点。动态子节点在该注入阶段尚为未展开的闭包，apply 从不会进入，因此其内部
/// 的虚拟节点在新旧实现下都拿不到滚动/视口状态——故此处同样不将其计入，保持行为一致
/// 的同时让含动态子节点的常规树仍能走 Arc 共享的快路径。
fn element_contains_virtual<VM>(element: &Element<VM>) -> bool {
    match &element.kind {
        WidgetKind::Virtual { .. } => true,
        WidgetKind::Container { children, .. } => {
            children.iter().any(|child_source| match child_source {
                crate::ui::widget::common::ChildSource::Static(children) => {
                    children.iter().any(element_contains_virtual)
                }
                crate::ui::widget::common::ChildSource::Dynamic(_) => false,
            })
        }
        _ => false,
    }
}

impl<VM: 'static> WidgetTree<VM> {
    pub fn new(root: impl Into<Element<VM>>) -> Self {
        let root = root.into();
        let has_virtual = element_contains_virtual(&root);
        Self {
            root: std::sync::Arc::new(root),
            has_virtual,
        }
    }

    pub(crate) fn compute_scene_with_units_and_widget_state_at(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        units: UnitContext,
        animations: &mut AnimationEngine,
        reduced_motion: bool,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        widget_states: &WidgetStateMap,
        select_open_states: &HashMap<WidgetId, bool>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        focused_input: Option<WidgetId>,
        focused_text_state: Option<&TextEditState>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        caret_visible: bool,
        now: Instant,
    ) -> ComputedScene<VM> {
        let layout = self.build_scene_layout_at(
            font_manager,
            theme,
            media,
            animations,
            units,
            scroll_offsets,
            &HashMap::new(),
            viewport,
            now,
        );
        self.collect_scene_from_layout_at(
            font_manager,
            &layout,
            theme,
            media,
            animations,
            reduced_motion,
            hovered_scrollbar,
            active_scrollbar,
            widget_states,
            select_open_states,
            scroll_offsets,
            viewport,
            focused_input,
            focused_text_state,
            selected_text,
            selected_text_state,
            caret_visible,
            now,
        )
    }

    pub(crate) fn build_scene_layout(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        units: UnitContext,
        scroll_offsets: &HashMap<WidgetId, Point>,
        virtual_states: &HashMap<WidgetId, VirtualCacheState>,
        viewport: Rect,
    ) -> ResolvedSceneLayout<VM> {
        self.build_scene_layout_at(
            font_manager,
            theme,
            media,
            animations,
            units,
            scroll_offsets,
            virtual_states,
            viewport,
            Instant::now(),
        )
    }

    pub(crate) fn build_scene_layout_at(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        units: UnitContext,
        scroll_offsets: &HashMap<WidgetId, Point>,
        virtual_states: &HashMap<WidgetId, VirtualCacheState>,
        viewport: Rect,
        now: Instant,
    ) -> ResolvedSceneLayout<VM> {
        let (mut layout, dependencies) = with_widget_stack(|| {
            with_dependency_collection(|| {
                let mut taffy = TaffyTree::new();
                // Without Virtual nodes there is no per-frame runtime state to
                // apply, so the source tree is shared via Arc instead of deep
                // cloned. Virtual trees clone-on-write to inject scroll/viewport
                // state before resolving.
                let root: std::sync::Arc<Element<VM>> = if self.has_virtual {
                    let mut root = (*self.root).clone();
                    apply_virtual_runtime_state_to_element(
                        &mut root,
                        scroll_offsets,
                        virtual_states,
                        VirtualViewportHint {
                            width: viewport.width,
                            height: viewport.height,
                        },
                    );
                    std::sync::Arc::new(root)
                } else {
                    std::sync::Arc::clone(&self.root)
                };
                let resolved_root = root.resolve(theme);
                let root_layout = resolved_root
                    .build_layout_tree(
                        &mut taffy, animations, theme, units, None, viewport, true, now,
                    )
                    .expect("widget tree layout should build");
                taffy
                    .compute_layout_with_measure(
                        root_layout.node,
                        TaffySize {
                            width: AvailableSpace::Definite(viewport.width.get()),
                            height: AvailableSpace::Definite(viewport.height.get()),
                        },
                        |known_dimensions, _, _, node_context, _| {
                            measure_node(
                                node_context,
                                known_dimensions,
                                font_manager,
                                theme,
                                media,
                                units,
                            )
                        },
                    )
                    .expect("widget tree layout should compute");

                ResolvedSceneLayout {
                    source_root: root,
                    root_id: resolved_root.id,
                    resolved_root,
                    layout_root: root_layout,
                    taffy,
                    units,
                    dependencies: DependencyGraph::default(),
                    paths: HashMap::new(),
                    parents: HashMap::new(),
                    depths: HashMap::new(),
                }
            })
        });
        layout.dependencies = dependencies;
        layout.rebuild_indexes();
        layout
    }

    #[cfg(test)]
    pub(crate) fn collect_scene_from_layout(
        &self,
        font_manager: &FontManager,
        layout: &ResolvedSceneLayout<VM>,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        reduced_motion: bool,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        widget_states: &WidgetStateMap,
        select_open_states: &HashMap<WidgetId, bool>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        focused_input: Option<WidgetId>,
        focused_text_state: Option<&TextEditState>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        caret_visible: bool,
    ) -> ComputedScene<VM> {
        self.collect_scene_from_layout_at(
            font_manager,
            layout,
            theme,
            media,
            animations,
            reduced_motion,
            hovered_scrollbar,
            active_scrollbar,
            widget_states,
            select_open_states,
            scroll_offsets,
            viewport,
            focused_input,
            focused_text_state,
            selected_text,
            selected_text_state,
            caret_visible,
            Instant::now(),
        )
    }

    pub(crate) fn collect_scene_from_layout_at(
        &self,
        font_manager: &FontManager,
        layout: &ResolvedSceneLayout<VM>,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        reduced_motion: bool,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        widget_states: &WidgetStateMap,
        select_open_states: &HashMap<WidgetId, bool>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        focused_input: Option<WidgetId>,
        focused_text_state: Option<&TextEditState>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        caret_visible: bool,
        now: Instant,
    ) -> ComputedScene<VM> {
        self.collect_scene_cache_from_layout_with_focus_value_at(
            font_manager,
            layout,
            theme,
            media,
            animations,
            reduced_motion,
            hovered_scrollbar,
            active_scrollbar,
            widget_states,
            select_open_states,
            scroll_offsets,
            viewport,
            focused_input,
            focused_text_state,
            None,
            None,
            None,
            None,
            selected_text,
            selected_text_state,
            caret_visible,
            now,
            &HashMap::new(),
            None,
            None,
        )
        .computed
    }

    pub(crate) fn collect_scene_cache_from_layout_with_focus_value(
        &self,
        font_manager: &FontManager,
        layout: &ResolvedSceneLayout<VM>,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        reduced_motion: bool,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        widget_states: &WidgetStateMap,
        select_open_states: &HashMap<WidgetId, bool>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        focused_input: Option<WidgetId>,
        focused_text_state: Option<&TextEditState>,
        focused_text_value: Option<&str>,
        focused_text_layout: Option<&TextLayoutInfo>,
        text_layout_overrides: Option<&HashMap<WidgetId, TextInputLayoutOverride<'_>>>,
        active_slider_value: Option<(WidgetId, f32)>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        caret_visible: bool,
        tooltip_hover_started_at: &HashMap<WidgetId, Instant>,
        active_tooltip: Option<ActiveTooltipState>,
        active_hover_popover: Option<WidgetId>,
    ) -> CollectedSceneCache<VM> {
        self.collect_scene_cache_from_layout_with_focus_value_at(
            font_manager,
            layout,
            theme,
            media,
            animations,
            reduced_motion,
            hovered_scrollbar,
            active_scrollbar,
            widget_states,
            select_open_states,
            scroll_offsets,
            viewport,
            focused_input,
            focused_text_state,
            focused_text_value,
            focused_text_layout,
            text_layout_overrides,
            active_slider_value,
            selected_text,
            selected_text_state,
            caret_visible,
            Instant::now(),
            tooltip_hover_started_at,
            active_tooltip,
            active_hover_popover,
        )
    }

    pub(crate) fn collect_scene_cache_from_layout_with_focus_value_and_virtual_state(
        &self,
        font_manager: &FontManager,
        layout: &ResolvedSceneLayout<VM>,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        reduced_motion: bool,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        widget_states: &WidgetStateMap,
        select_open_states: &HashMap<WidgetId, bool>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        virtual_states: &HashMap<WidgetId, VirtualCacheState>,
        viewport: Rect,
        focused_input: Option<WidgetId>,
        focused_text_state: Option<&TextEditState>,
        focused_text_value: Option<&str>,
        focused_text_layout: Option<&TextLayoutInfo>,
        text_layout_overrides: Option<&HashMap<WidgetId, TextInputLayoutOverride<'_>>>,
        active_slider_value: Option<(WidgetId, f32)>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        caret_visible: bool,
        tooltip_hover_started_at: &HashMap<WidgetId, Instant>,
        active_tooltip: Option<ActiveTooltipState>,
        active_hover_popover: Option<WidgetId>,
    ) -> CollectedSceneCache<VM> {
        self.collect_scene_cache_from_layout_with_focus_value_at_with_virtual_state(
            font_manager,
            layout,
            theme,
            media,
            animations,
            reduced_motion,
            hovered_scrollbar,
            active_scrollbar,
            widget_states,
            select_open_states,
            scroll_offsets,
            viewport,
            focused_input,
            focused_text_state,
            focused_text_value,
            focused_text_layout,
            text_layout_overrides,
            active_slider_value,
            selected_text,
            selected_text_state,
            caret_visible,
            Instant::now(),
            tooltip_hover_started_at,
            virtual_states,
            active_tooltip,
            active_hover_popover,
        )
    }

    pub(crate) fn collect_scene_cache_from_layout_with_focus_value_virtual_and_menu_state(
        &self,
        font_manager: &FontManager,
        layout: &ResolvedSceneLayout<VM>,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        reduced_motion: bool,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        widget_states: &WidgetStateMap,
        select_open_states: &HashMap<WidgetId, bool>,
        menu_open_states: &HashMap<WidgetId, bool>,
        menubar_active_states: &HashMap<u64, Option<usize>>,
        context_menu_anchor_states: &HashMap<WidgetId, Point>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        virtual_states: &HashMap<WidgetId, VirtualCacheState>,
        viewport: Rect,
        focused_input: Option<WidgetId>,
        focused_text_state: Option<&TextEditState>,
        focused_text_value: Option<&str>,
        focused_text_layout: Option<&TextLayoutInfo>,
        text_layout_overrides: Option<&HashMap<WidgetId, TextInputLayoutOverride<'_>>>,
        active_slider_value: Option<(WidgetId, f32)>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        caret_visible: bool,
        tooltip_hover_started_at: &HashMap<WidgetId, Instant>,
        active_tooltip: Option<ActiveTooltipState>,
        active_hover_popover: Option<WidgetId>,
    ) -> CollectedSceneCache<VM> {
        self.collect_scene_cache_from_layout_with_focus_value_and_reduced_motion_at(
            font_manager,
            layout,
            theme,
            media,
            animations,
            reduced_motion,
            hovered_scrollbar,
            active_scrollbar,
            widget_states,
            select_open_states,
            menu_open_states,
            menubar_active_states,
            context_menu_anchor_states,
            scroll_offsets,
            viewport,
            focused_input,
            focused_text_state,
            focused_text_value,
            focused_text_layout,
            text_layout_overrides,
            active_slider_value,
            selected_text,
            selected_text_state,
            caret_visible,
            Instant::now(),
            tooltip_hover_started_at,
            virtual_states,
            active_tooltip,
            active_hover_popover,
        )
    }

    pub(crate) fn collect_scene_cache_from_layout_with_focus_value_at(
        &self,
        font_manager: &FontManager,
        layout: &ResolvedSceneLayout<VM>,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        reduced_motion: bool,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        widget_states: &WidgetStateMap,
        select_open_states: &HashMap<WidgetId, bool>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        focused_input: Option<WidgetId>,
        focused_text_state: Option<&TextEditState>,
        focused_text_value: Option<&str>,
        focused_text_layout: Option<&TextLayoutInfo>,
        text_layout_overrides: Option<&HashMap<WidgetId, TextInputLayoutOverride<'_>>>,
        active_slider_value: Option<(WidgetId, f32)>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        caret_visible: bool,
        now: Instant,
        tooltip_hover_started_at: &HashMap<WidgetId, Instant>,
        active_tooltip: Option<ActiveTooltipState>,
        active_hover_popover: Option<WidgetId>,
    ) -> CollectedSceneCache<VM> {
        self.collect_scene_cache_from_layout_with_focus_value_at_with_virtual_state(
            font_manager,
            layout,
            theme,
            media,
            animations,
            reduced_motion,
            hovered_scrollbar,
            active_scrollbar,
            widget_states,
            select_open_states,
            scroll_offsets,
            viewport,
            focused_input,
            focused_text_state,
            focused_text_value,
            focused_text_layout,
            text_layout_overrides,
            active_slider_value,
            selected_text,
            selected_text_state,
            caret_visible,
            now,
            tooltip_hover_started_at,
            &HashMap::new(),
            active_tooltip,
            active_hover_popover,
        )
    }

    pub(crate) fn collect_scene_cache_from_layout_with_focus_value_at_with_virtual_state(
        &self,
        font_manager: &FontManager,
        layout: &ResolvedSceneLayout<VM>,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        reduced_motion: bool,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        widget_states: &WidgetStateMap,
        select_open_states: &HashMap<WidgetId, bool>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        focused_input: Option<WidgetId>,
        focused_text_state: Option<&TextEditState>,
        focused_text_value: Option<&str>,
        focused_text_layout: Option<&TextLayoutInfo>,
        text_layout_overrides: Option<&HashMap<WidgetId, TextInputLayoutOverride<'_>>>,
        active_slider_value: Option<(WidgetId, f32)>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        caret_visible: bool,
        now: Instant,
        tooltip_hover_started_at: &HashMap<WidgetId, Instant>,
        virtual_states: &HashMap<WidgetId, VirtualCacheState>,
        active_tooltip: Option<ActiveTooltipState>,
        active_hover_popover: Option<WidgetId>,
    ) -> CollectedSceneCache<VM> {
        self.collect_scene_cache_from_layout_with_focus_value_and_reduced_motion_at(
            font_manager,
            layout,
            theme,
            media,
            animations,
            reduced_motion,
            hovered_scrollbar,
            active_scrollbar,
            widget_states,
            select_open_states,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            scroll_offsets,
            viewport,
            focused_input,
            focused_text_state,
            focused_text_value,
            focused_text_layout,
            text_layout_overrides,
            active_slider_value,
            selected_text,
            selected_text_state,
            caret_visible,
            now,
            tooltip_hover_started_at,
            virtual_states,
            active_tooltip,
            active_hover_popover,
        )
    }

    pub(crate) fn collect_scene_cache_from_layout_with_focus_value_and_reduced_motion_at(
        &self,
        font_manager: &FontManager,
        layout: &ResolvedSceneLayout<VM>,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        reduced_motion: bool,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        widget_states: &WidgetStateMap,
        select_open_states: &HashMap<WidgetId, bool>,
        menu_open_states: &HashMap<WidgetId, bool>,
        menubar_active_states: &HashMap<u64, Option<usize>>,
        context_menu_anchor_states: &HashMap<WidgetId, Point>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        focused_input: Option<WidgetId>,
        focused_text_state: Option<&TextEditState>,
        focused_text_value: Option<&str>,
        focused_text_layout: Option<&TextLayoutInfo>,
        text_layout_overrides: Option<&HashMap<WidgetId, TextInputLayoutOverride<'_>>>,
        active_slider_value: Option<(WidgetId, f32)>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        caret_visible: bool,
        now: Instant,
        tooltip_hover_started_at: &HashMap<WidgetId, Instant>,
        virtual_states: &HashMap<WidgetId, VirtualCacheState>,
        active_tooltip: Option<ActiveTooltipState>,
        active_hover_popover: Option<WidgetId>,
    ) -> CollectedSceneCache<VM> {
        let next_tooltip_wakeup: std::cell::Cell<Option<Instant>> = std::cell::Cell::new(None);
        let next_toast_wakeup: std::cell::Cell<Option<Instant>> = std::cell::Cell::new(None);
        let ((mut computed, lifecycle_states, chunks, chunk_parts, visual_contexts), dependencies) =
            with_widget_stack(|| {
                with_dependency_collection(|| {
                    let cap = layout.resolved_root.estimated_node_count();
                    let mut lifecycle_states = HashMap::with_capacity(cap / 4);
                    let mut chunks = HashMap::with_capacity(cap);
                    let mut chunk_parts = HashMap::with_capacity(cap / 2);
                    let mut visual_contexts = HashMap::with_capacity(cap);
                    let mut context = CollectContext {
                        taffy: &layout.taffy,
                        font_manager,
                        theme,
                        media,
                        focused_input,
                        focused_text_state,
                        focused_text_value,
                        focused_text_layout,
                        text_layout_overrides,
                        active_slider_value,
                        caret_visible,
                        selected_text,
                        selected_text_state,
                        hovered_scrollbar,
                        active_scrollbar,
                        widget_states,
                        select_open_states,
                        menu_open_states,
                        menubar_active_states,
                        context_menu_anchor_states,
                        scroll_offsets,
                        virtual_states,
                        viewport,
                        units: layout.units,
                        animations,
                        reduced_motion,
                        now,
                        focus: super::scene::FocusCollectState::default(),
                        tooltip_hover_started_at,
                        next_tooltip_wakeup: &next_tooltip_wakeup,
                        next_toast_wakeup: &next_toast_wakeup,
                        active_tooltip,
                        active_hover_popover,
                    };
                    let root_id = layout.resolved_root.collect_subtree_cache(
                        &layout.layout_root,
                        VisualContext {
                            origin: Point {
                                x: viewport.x,
                                y: viewport.y,
                            },
                            opacity: 1.0,
                            clip_rect: viewport,
                            clip_mask: None,
                        },
                        &mut context,
                        &mut lifecycle_states,
                        &mut chunks,
                        &mut chunk_parts,
                        &mut visual_contexts,
                    );
                    let computed = chunks.get(&root_id).cloned().unwrap_or_default();
                    (
                        computed,
                        lifecycle_states,
                        chunks,
                        chunk_parts,
                        visual_contexts,
                    )
                })
            });
        computed.finalize_portals(viewport);
        computed.dependencies = dependencies.clone();
        CollectedSceneCache {
            computed,
            lifecycle_states,
            chunks,
            chunk_parts,
            visual_contexts,
            dependencies,
            next_tooltip_wakeup: next_tooltip_wakeup.get(),
            next_toast_wakeup: next_toast_wakeup.get(),
        }
    }

    #[cfg(test)]
    pub(crate) fn collect_scene_from_layout_with_focus_value(
        &self,
        font_manager: &FontManager,
        layout: &ResolvedSceneLayout<VM>,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        reduced_motion: bool,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        widget_states: &WidgetStateMap,
        select_open_states: &HashMap<WidgetId, bool>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        focused_input: Option<WidgetId>,
        focused_text_state: Option<&TextEditState>,
        focused_text_value: Option<&str>,
        focused_text_layout: Option<&TextLayoutInfo>,
        text_layout_overrides: Option<&HashMap<WidgetId, TextInputLayoutOverride<'_>>>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        caret_visible: bool,
    ) -> ComputedScene<VM> {
        self.collect_scene_cache_from_layout_with_focus_value(
            font_manager,
            layout,
            theme,
            media,
            animations,
            reduced_motion,
            hovered_scrollbar,
            active_scrollbar,
            widget_states,
            select_open_states,
            scroll_offsets,
            viewport,
            focused_input,
            focused_text_state,
            focused_text_value,
            focused_text_layout,
            text_layout_overrides,
            None,
            selected_text,
            selected_text_state,
            caret_visible,
            &HashMap::new(),
            None,
            None,
        )
        .computed
    }
}

pub enum WidgetCommand<VM> {
    Command(Command<VM>),
    Value(ValueCommand<VM, String>, String),
}

pub struct WidgetEventResult<VM> {
    pub command: Option<WidgetCommand<VM>>,
    pub focus: Option<WidgetId>,
    pub request_redraw: bool,
}

pub fn rect(x: Dp, y: Dp, width: Dp, height: Dp) -> Rect {
    Rect::new(x, y, width, height)
}
