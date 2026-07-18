use super::scene::ActiveTooltipState;
use super::*;
use crate::ui::widget::r#virtual::{VirtualCacheState, VirtualViewportHint};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrictReactiveViolation {
    DynamicChildren,
}

impl std::fmt::Display for StrictReactiveViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DynamicChildren => f.write_str(
                "strict reactive widget trees do not allow signal-driven dynamic children",
            ),
        }
    }
}

impl std::error::Error for StrictReactiveViolation {}

pub struct WidgetTree<VM> {
    pub(super) root: std::sync::Arc<Element<VM>>,
    /// 树内是否存在 Virtual 节点。无虚拟节点时全量重建可直接共享源树（Arc clone），
    /// 跳过整棵树的深拷贝。
    pub(super) has_virtual: bool,
    /// Whether this tree can contain media event handlers.
    ///
    /// Static trees are classified exactly once at construction. Dynamic and
    /// virtual child sources are conservatively classified as capable because
    /// their resolved children can add or remove handlers at any revision.
    /// This lets the runtime skip the otherwise O(n) resolve/event walk for the
    /// overwhelmingly common static tree with no media callbacks, while never
    /// hiding a callback introduced by a legacy structural update.
    pub(super) may_have_media_event_handlers: bool,
    strict_reactive: bool,
}

/// 在递归走 widget 树的入口（root collect / root layout / overlay 子场景）使用，
/// 一次性预留 16MB 备用栈，避免 debug 构建中每层 ~数十 KB 的局部把默认 1MB 栈打爆。
pub(crate) fn with_widget_stack<R>(f: impl FnOnce() -> R) -> R {
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    {
        const WIDGET_STACK_SIZE: usize = 16 * 1024 * 1024;
        return stacker::grow(WIDGET_STACK_SIZE, f);
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

/// 检测树内是否存在 Virtual 节点。动态子节点需要保守视为可能包含 Virtual，
/// 因为它们会在 resolve 阶段展开，滚动时也需要让 layout cache 失效以重算可见窗口。
fn element_contains_virtual<VM>(element: &Element<VM>) -> bool {
    match &element.kind {
        WidgetKind::Virtual { .. } => true,
        WidgetKind::Container { children, .. } => {
            children.iter().any(|child_source| match child_source {
                crate::ui::widget::common::ChildSource::Static(children) => {
                    children.iter().any(element_contains_virtual)
                }
                crate::ui::widget::common::ChildSource::Dynamic(_) => true,
                crate::ui::widget::common::ChildSource::KeyedFor(_) => true,
                crate::ui::widget::common::ChildSource::Switch {
                    cases, fallback, ..
                } => {
                    cases.iter().any(element_contains_virtual)
                        || fallback.as_deref().is_some_and(element_contains_virtual)
                }
                crate::ui::widget::common::ChildSource::Show { child, .. } => {
                    element_contains_virtual(child)
                }
            })
        }
        _ => false,
    }
}

fn element_contains_dynamic_children<VM>(element: &Element<VM>) -> bool {
    match &element.kind {
        WidgetKind::Container { children, .. } => {
            children.iter().any(|child_source| match child_source {
                crate::ui::widget::common::ChildSource::Static(children) => {
                    children.iter().any(element_contains_dynamic_children)
                }
                crate::ui::widget::common::ChildSource::Dynamic(_) => true,
                crate::ui::widget::common::ChildSource::KeyedFor(_) => false,
                crate::ui::widget::common::ChildSource::Switch {
                    cases, fallback, ..
                } => {
                    cases.iter().any(element_contains_dynamic_children)
                        || fallback
                            .as_deref()
                            .is_some_and(element_contains_dynamic_children)
                }
                crate::ui::widget::common::ChildSource::Show { child, .. } => {
                    element_contains_dynamic_children(child)
                }
            })
        }
        _ => false,
    }
}

fn element_may_have_media_event_handlers<VM>(element: &Element<VM>) -> bool {
    if element.media_events.has_any() {
        return true;
    }

    match &element.kind {
        WidgetKind::Container { children, .. } => children.iter().any(|source| match source {
            crate::ui::widget::common::ChildSource::Static(children) => {
                children.iter().any(element_may_have_media_event_handlers)
            }
            // The resolved contents of these sources are revision-dependent.
            // Treat them as capable even when their current value is empty so
            // a later add/remove cannot leave a stale negative capability.
            crate::ui::widget::common::ChildSource::Dynamic(_)
            | crate::ui::widget::common::ChildSource::KeyedFor(_) => true,
            crate::ui::widget::common::ChildSource::Switch {
                cases, fallback, ..
            } => {
                cases.iter().any(element_may_have_media_event_handlers)
                    || fallback
                        .as_deref()
                        .is_some_and(element_may_have_media_event_handlers)
            }
            crate::ui::widget::common::ChildSource::Show { child, .. } => {
                element_may_have_media_event_handlers(child)
            }
        }),
        // Virtual item sources resolve runtime-owned children, so their
        // handler capability cannot be proven empty from the root element.
        WidgetKind::Virtual { .. } => true,
        _ => false,
    }
}

impl<VM: 'static> WidgetTree<VM> {
    fn from_root(root: Element<VM>, strict_reactive: bool) -> Self {
        with_widget_stack(|| {
            let has_virtual = element_contains_virtual(&root);
            let may_have_media_event_handlers = element_may_have_media_event_handlers(&root);
            Self {
                root: std::sync::Arc::new(root),
                has_virtual,
                may_have_media_event_handlers,
                strict_reactive,
            }
        })
    }

    /// Construct a strict retained-reactive widget tree.
    ///
    /// The default constructor rejects signal-driven child insertion/removal.
    /// Structural changes must go through an explicit rebuild path instead of
    /// being driven implicitly by `Signal<Element>` / `Signal<Vec<Element>>`.
    pub fn new(root: impl Into<Element<VM>>) -> Self {
        with_widget_stack(|| {
            let root = root.into();
            if let Err(error) = Self::validate_strict_root(&root) {
                panic!("{error}");
            }
            Self::from_root(root, true)
        })
    }

    /// Construct a legacy tree that allows signal-driven dynamic children.
    ///
    /// This is an explicit compatibility path for tests and for code that
    /// still performs structural updates through dependency invalidation.
    /// Strict O(1) reactive updates are not guaranteed for this tree.
    pub fn new_legacy(root: impl Into<Element<VM>>) -> Self {
        with_widget_stack(|| Self::from_root(root.into(), false))
    }

    fn validate_strict_root(root: &Element<VM>) -> Result<(), StrictReactiveViolation> {
        if element_contains_dynamic_children(root) {
            return Err(StrictReactiveViolation::DynamicChildren);
        }
        Ok(())
    }

    /// Construct a tree that follows the strict retained-reactive rules.
    ///
    /// In strict mode, signals may update retained values and slots, but they
    /// may not implicitly add or remove widgets. Use `WidgetTree::new_legacy`
    /// only for the explicit legacy compatibility path.
    pub fn try_new_strict(root: impl Into<Element<VM>>) -> Result<Self, StrictReactiveViolation> {
        with_widget_stack(|| {
            let root = root.into();
            Self::validate_strict_root(&root)?;
            Ok(Self::from_root(root, true))
        })
    }

    pub(crate) fn has_virtual(&self) -> bool {
        self.has_virtual
    }

    pub(crate) fn may_have_media_event_handlers(&self) -> bool {
        self.may_have_media_event_handlers
    }

    pub(crate) fn is_strict_reactive(&self) -> bool {
        self.strict_reactive
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
        with_widget_stack(|| {
            self.compute_scene_with_units_and_widget_state_at_inner(
                font_manager,
                theme,
                media,
                units,
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
        })
    }

    fn compute_scene_with_units_and_widget_state_at_inner(
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
        let default_style_sheet = crate::ui::widget::StyleSheet::default();
        let layout = self.build_scene_layout_at_with_previous_style_sheet_and_reduced_motion(
            font_manager,
            theme,
            media,
            animations,
            units,
            scroll_offsets,
            &HashMap::new(),
            viewport,
            now,
            None,
            reduced_motion,
            &default_style_sheet,
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
        self.build_scene_layout_at_with_previous(
            font_manager,
            theme,
            media,
            animations,
            units,
            scroll_offsets,
            virtual_states,
            viewport,
            now,
            None,
        )
    }

    pub(crate) fn build_scene_layout_at_with_previous(
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
        previous: Option<&ResolvedSceneLayout<VM>>,
    ) -> ResolvedSceneLayout<VM> {
        let default_style_sheet = crate::ui::widget::StyleSheet::default();
        self.build_scene_layout_at_with_previous_and_style_sheet(
            font_manager,
            theme,
            media,
            animations,
            units,
            scroll_offsets,
            virtual_states,
            viewport,
            now,
            previous,
            &default_style_sheet,
        )
    }

    pub(crate) fn build_scene_layout_at_with_previous_and_style_sheet(
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
        previous: Option<&ResolvedSceneLayout<VM>>,
        style_sheet: &crate::ui::widget::StyleSheet,
    ) -> ResolvedSceneLayout<VM> {
        self.build_scene_layout_at_with_previous_and_style_context(
            font_manager,
            theme,
            media,
            animations,
            units,
            scroll_offsets,
            virtual_states,
            viewport,
            now,
            previous,
            crate::ui::theme::StyleContext::from_theme(theme).with_text_scale(units.font_scale()),
            style_sheet,
        )
    }

    pub(crate) fn build_scene_layout_at_with_previous_style_sheet_and_reduced_motion(
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
        previous: Option<&ResolvedSceneLayout<VM>>,
        reduced_motion: bool,
        style_sheet: &crate::ui::widget::StyleSheet,
    ) -> ResolvedSceneLayout<VM> {
        self.build_scene_layout_at_with_previous_and_style_context(
            font_manager,
            theme,
            media,
            animations,
            units,
            scroll_offsets,
            virtual_states,
            viewport,
            now,
            previous,
            crate::ui::theme::StyleContext::from_theme(theme)
                .with_reduced_motion(reduced_motion)
                .with_text_scale(units.font_scale()),
            style_sheet,
        )
    }

    fn build_scene_layout_at_with_previous_and_style_context(
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
        previous: Option<&ResolvedSceneLayout<VM>>,
        style_context: crate::ui::theme::StyleContext<'_>,
        style_sheet: &crate::ui::widget::StyleSheet,
    ) -> ResolvedSceneLayout<VM> {
        let (mut layout, dependencies) = with_widget_stack(|| {
            with_dependency_collection(|| {
                let mut taffy = TaffyTree::new();
                let root = std::sync::Arc::clone(&self.root);
                let resolved_root = root.resolve_with_runtime_state_and_style_sheet(
                    theme,
                    previous.map(|layout| &layout.resolved_root),
                    scroll_offsets,
                    virtual_states,
                    VirtualViewportHint {
                        width: viewport.width,
                        height: viewport.height,
                    },
                    &style_context,
                    style_sheet,
                );
                let root_layout = resolved_root
                    .build_layout_tree(
                        &mut taffy, animations, theme, units, None, viewport, true, now,
                    )
                    .expect("widget tree layout should build");
                compute_taffy_layout_with_measure(
                    &mut taffy,
                    root_layout.node,
                    viewport,
                    font_manager,
                    theme,
                    media,
                    units,
                )
                .expect("widget tree layout should compute");

                ResolvedSceneLayout {
                    source_root: root,
                    root_id: resolved_root.id,
                    resolved_root,
                    layout_root: root_layout,
                    taffy,
                    units,
                    frame_clock: crate::animation::FrameClockSnapshot::fallback(now),
                    dependencies: DependencyGraph::default(),
                    paths: HashMap::new(),
                    parents: HashMap::new(),
                    depths: HashMap::new(),
                    virtual_widgets: HashSet::new(),
                    subtree_sizes: HashMap::new(),
                    scroll_view_controllers: HashMap::new(),
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
        let default_style_sheet = crate::ui::widget::StyleSheet::default();
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
            &default_style_sheet,
        )
        .computed
    }

    #[cfg(test)]
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
        let default_style_sheet = crate::ui::widget::StyleSheet::default();
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
            &default_style_sheet,
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
        gpu_scroll_enabled: bool,
        style_sheet: &crate::ui::widget::StyleSheet,
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
            gpu_scroll_enabled,
            style_sheet,
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
        style_sheet: &crate::ui::widget::StyleSheet,
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
            style_sheet,
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
        style_sheet: &crate::ui::widget::StyleSheet,
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
            false,
            style_sheet,
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
        gpu_scroll_enabled: bool,
        style_sheet: &crate::ui::widget::StyleSheet,
    ) -> CollectedSceneCache<VM> {
        let next_tooltip_wakeup: std::cell::Cell<Option<Instant>> = std::cell::Cell::new(None);
        let next_toast_wakeup: std::cell::Cell<Option<Instant>> = std::cell::Cell::new(None);
        let style_context = crate::ui::theme::StyleContext::from_theme(theme)
            .with_reduced_motion(reduced_motion)
            .with_text_scale(layout.units.font_scale());
        let context = CollectContext {
            taffy: &layout.taffy,
            font_manager,
            theme,
            style_context,
            style_sheet,
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
            frame_clock: layout.frame_clock,
            focus: super::scene::FocusCollectState::default(),
            tooltip_hover_started_at,
            next_tooltip_wakeup: &next_tooltip_wakeup,
            next_toast_wakeup: &next_toast_wakeup,
            active_tooltip,
            active_hover_popover,
            gpu_scroll_enabled,
            gpu_scroll_container: None,
            transform_stack: smallvec::SmallVec::new(),
            portal_accessibility_geometry: None,
            portal_accessibility_path: smallvec::SmallVec::new(),
        };
        self.collect_scene_cache_with_context(
            layout,
            context,
            &next_tooltip_wakeup,
            &next_toast_wakeup,
        )
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

    /// 简化的场景收集接口：接受已构建的 CollectContext。
    ///
    /// 此函数是场景收集重构的核心：将 30+ 参数的函数签名简化为接受预构建的上下文。
    /// 调用点负责构建 CollectContext，这样可以：
    /// - 减少函数签名膨胀
    /// - 让参数组织更清晰（按职责分组）
    /// - 便于未来扩展（新增上下文字段不影响所有调用点）
    pub(crate) fn collect_scene_cache_with_context(
        &self,
        layout: &ResolvedSceneLayout<VM>,
        mut context: CollectContext<'_, '_>,
        next_tooltip_wakeup: &std::cell::Cell<Option<Instant>>,
        next_toast_wakeup: &std::cell::Cell<Option<Instant>>,
    ) -> CollectedSceneCache<VM> {
        let viewport = context.viewport;
        let ((mut computed, lifecycle_states, chunks, chunk_parts, visual_contexts), dependencies) =
            with_widget_stack(|| {
                with_dependency_collection(|| {
                    let cap = layout.subtree_size(layout.root_id());
                    let mut lifecycle_states = HashMap::with_capacity(cap / 4);
                    let mut chunks = HashMap::with_capacity(cap);
                    let mut chunk_parts = HashMap::with_capacity(cap / 2);
                    let mut visual_contexts = HashMap::with_capacity(cap);
                    let root_id = layout.resolved_root.collect_subtree_cache(
                        &layout.layout_root,
                        VisualContext {
                            origin: Point {
                                x: viewport.x,
                                y: viewport.y,
                            },
                            opacity: 1.0,
                            clip_rect: viewport,
                            overflow_clip_rect: None,
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
