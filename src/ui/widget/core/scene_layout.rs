mod patch;

pub(super) use self::patch::media_event_phase;
use self::patch::{
    collect_indexes, collect_resolved_widget_ids, layout_at_path, patch_layout_at_path,
    patch_resolved_at_path, resolved_at_path,
};
use super::scene::ActiveTooltipState;
use super::*;
use crate::ui::widget::VirtualCacheState;

#[derive(Clone)]
pub(crate) struct ResolvedSceneLayout<VM> {
    pub(super) source_root: std::sync::Arc<Element<VM>>,
    pub(super) resolved_root: ResolvedElement<VM>,
    pub(super) layout_root: LayoutNode,
    pub(super) taffy: TaffyTree<MeasureContext>,
    pub(super) units: UnitContext,
    pub(super) dependencies: DependencyGraph,
    pub(super) root_id: WidgetId,
    pub(super) paths: HashMap<WidgetId, Vec<usize>>,
    pub(super) parents: HashMap<WidgetId, Option<WidgetId>>,
    pub(super) depths: HashMap<WidgetId, usize>,
}

impl<VM: 'static> ResolvedSceneLayout<VM> {
    pub(crate) fn dependencies(&self) -> &DependencyGraph {
        &self.dependencies
    }

    pub(crate) fn root_id(&self) -> WidgetId {
        self.root_id
    }

    pub(crate) fn path_for(&self, widget_id: WidgetId) -> Option<&[usize]> {
        self.paths.get(&widget_id).map(Vec::as_slice)
    }

    pub(crate) fn parent_of(&self, widget_id: WidgetId) -> Option<WidgetId> {
        self.parents.get(&widget_id).copied().flatten()
    }

    pub(crate) fn depth_of(&self, widget_id: WidgetId) -> usize {
        self.depths.get(&widget_id).copied().unwrap_or_default()
    }

    pub(crate) fn subtree_widget_ids(&self, widget_id: WidgetId) -> Vec<WidgetId> {
        let Some(path) = self.path_for(widget_id) else {
            return Vec::new();
        };
        let mut ids = Vec::new();
        let node = self.resolved_at_path(path);
        collect_resolved_widget_ids(node, &mut ids);
        ids
    }

    pub(crate) fn resolved_widget(&self, widget_id: WidgetId) -> Option<&ResolvedElement<VM>> {
        let path = self.path_for(widget_id)?;
        Some(self.resolved_at_path(path))
    }

    pub(crate) fn widget_bounds(&self, widget_id: WidgetId) -> Option<Rect> {
        let path = self.path_for(widget_id)?;
        let mut node = &self.layout_root;
        let mut x = 0.0;
        let mut y = 0.0;
        for child_index in path {
            let child = node.children.get(*child_index)?;
            let layout = self.taffy.layout(child.node).ok()?;
            x += layout.location.x;
            y += layout.location.y;
            node = child;
        }
        let layout = self.taffy.layout(node.node).ok()?;
        Some(Rect::new(x, y, layout.size.width, layout.size.height))
    }

    /// 所有 widget id 的迭代器，用于全局扫描（例如全局快捷键派发）。
    pub(crate) fn all_widget_ids(&self) -> impl Iterator<Item = WidgetId> + '_ {
        self.paths.keys().copied()
    }

    pub(crate) fn can_patch_layout_dependency_as_scene(&self, widget_id: WidgetId) -> bool {
        let Some(node) = self.resolved_widget(widget_id) else {
            return false;
        };
        match &node.kind {
            ResolvedWidgetKind::Text { text } => {
                !text.user_select
                    && node.background.is_none()
                    && !node.interactions.has_any()
                    && !node.lifecycle_events.has_any()
                    && !node.media_events.has_any()
                    && node.visual.border_color.is_none()
                    && node.visual.border_radius.is_none()
                    && node.visual.border_width.is_none()
                    && node.visual.background_brush.is_none()
                    && node.visual.background_image.is_none()
                    && node.visual.background_blur.resolve() == Dp::ZERO
                    && node.visual.shadow.is_none()
                    && node.visual.opacity.resolve() == 1.0
                    && node.visual.offset.resolve() == Point::ZERO
                    && node.visual.scale.resolve() == 1.0
            }
            _ => false,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn query_canvas_scene_at_widget(
        &self,
        widget_id: WidgetId,
        font_manager: &FontManager,
        units: UnitContext,
        scene_position: Point,
    ) -> Option<CanvasSceneHit> {
        self.query_canvas_scene_all_at_widget(widget_id, font_manager, units, scene_position)
            .into_iter()
            .next()
    }

    #[allow(dead_code)]
    pub(crate) fn query_canvas_scene_all_at_widget(
        &self,
        widget_id: WidgetId,
        font_manager: &FontManager,
        units: UnitContext,
        scene_position: Point,
    ) -> Vec<CanvasSceneHit> {
        let Some(node) = self.resolved_widget(widget_id) else {
            return Vec::new();
        };
        let ResolvedWidgetKind::Canvas { scene, .. } = &node.kind else {
            return Vec::new();
        };
        scene
            .resolve()
            .query_point_all_with_runtime_context(font_manager, units, scene_position)
    }

    pub(crate) fn rebuild_indexes(&mut self) {
        self.root_id = self.resolved_root.id;
        let mut path = Vec::new();
        let mut paths = HashMap::new();
        let mut parents = HashMap::new();
        let mut depths = HashMap::new();
        collect_indexes(
            &self.resolved_root,
            None,
            0,
            &mut path,
            &mut paths,
            &mut parents,
            &mut depths,
        );
        self.paths = paths;
        self.parents = parents;
        self.depths = depths;
    }

    fn resolved_at_path(&self, path: &[usize]) -> &ResolvedElement<VM> {
        resolved_at_path(&self.resolved_root, path)
    }

    fn layout_at_path(&self, path: &[usize]) -> &LayoutNode {
        layout_at_path(&self.layout_root, path)
    }

    pub(crate) fn collect_scene_cache_for_widget_with_focus_value(
        &self,
        widget_id: WidgetId,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        reduced_motion: bool,
        visual_context: VisualContextSnapshot,
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
        active_tooltip: Option<ActiveTooltipState>,
        active_hover_popover: Option<WidgetId>,
    ) -> Option<CollectedSceneCache<VM>> {
        let path = self.path_for(widget_id)?;
        let tooltip_hover_started_at: HashMap<WidgetId, std::time::Instant> = HashMap::new();
        let next_tooltip_wakeup: std::cell::Cell<Option<std::time::Instant>> =
            std::cell::Cell::new(None);
        let next_toast_wakeup: std::cell::Cell<Option<std::time::Instant>> =
            std::cell::Cell::new(None);
        let ((mut computed, lifecycle_states, chunks, chunk_parts, visual_contexts), dependencies): (
            (
                ComputedScene<VM>,
                HashMap<WidgetId, LifecycleEventState<VM>>,
                HashMap<WidgetId, ComputedScene<VM>>,
                HashMap<WidgetId, SceneChunkParts<VM>>,
                HashMap<WidgetId, VisualContextSnapshot>,
            ),
            DependencyGraph,
        ) = with_widget_stack(|| {
            with_dependency_collection(|| {
                let cap = self.resolved_at_path(path).estimated_node_count();
                let mut lifecycle_states = HashMap::with_capacity(cap / 4);
                let mut chunks = HashMap::with_capacity(cap);
                let mut chunk_parts = HashMap::with_capacity(cap / 2);
                let mut visual_contexts = HashMap::with_capacity(cap);
                let empty_menu_open_states = HashMap::<WidgetId, bool>::new();
                let empty_menubar_active_states = HashMap::<u64, Option<usize>>::new();
                let empty_context_menu_anchor_states = HashMap::<WidgetId, Point>::new();
                let mut context = CollectContext {
                    taffy: &self.taffy,
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
                    menu_open_states: &empty_menu_open_states,
                    menubar_active_states: &empty_menubar_active_states,
                    context_menu_anchor_states: &empty_context_menu_anchor_states,
                    scroll_offsets,
                    virtual_states,
                    viewport,
                    units: self.units,
                    animations,
                    reduced_motion,
                    now: std::time::Instant::now(),
                    focus: super::scene::FocusCollectState::default(),
                    tooltip_hover_started_at: &tooltip_hover_started_at,
                    next_tooltip_wakeup: &next_tooltip_wakeup,
                    next_toast_wakeup: &next_toast_wakeup,
                    active_tooltip,
                    active_hover_popover,
                };
                let root_id = self.resolved_at_path(path).collect_subtree_cache(
                    self.layout_at_path(path),
                    visual_context.into(),
                    &mut context,
                    &mut lifecycle_states,
                    &mut chunks,
                    &mut chunk_parts,
                    &mut visual_contexts,
                );
                // 根节点的合并场景已存进 chunks;取出一份 owned 副本作为返回值
                // (整个收集过程中唯一一次必要的子树克隆)。
                let computed = chunks
                    .get(&root_id)
                    .cloned()
                    .unwrap_or_default();
                (
                    computed,
                    lifecycle_states,
                    chunks,
                    chunk_parts,
                    visual_contexts,
                )
            })
        });
        computed.dependencies = dependencies.clone();
        Some(CollectedSceneCache {
            computed,
            lifecycle_states,
            chunks,
            chunk_parts,
            visual_contexts,
            dependencies,
            next_tooltip_wakeup: next_tooltip_wakeup.get(),
            next_toast_wakeup: next_toast_wakeup.get(),
        })
    }

    pub(crate) fn recompose_scene_chunk(
        &self,
        widget_id: WidgetId,
        chunk_parts: &HashMap<WidgetId, SceneChunkParts<VM>>,
        chunks: &mut HashMap<WidgetId, ComputedScene<VM>>,
    ) -> Option<()> {
        let path = self.path_for(widget_id)?;
        let node = self.resolved_at_path(path);
        let parts = chunk_parts.get(&widget_id)?;
        let mut composed = parts.before_children.clone();
        if let ResolvedWidgetKind::Container { children, .. }
        | ResolvedWidgetKind::Virtual { children, .. } = &node.kind
        {
            for child in children {
                let child_chunk = chunks.get(&child.id)?;
                composed.extend(&child_chunk);
            }
        }
        composed.extend(&parts.after_children);
        chunks.insert(widget_id, composed);
        Some(())
    }

    pub(crate) fn patch_layout_roots(
        &mut self,
        roots: &[WidgetId],
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        viewport: Rect,
        now: std::time::Instant,
    ) -> Result<HashSet<WidgetId>, taffy::TaffyError> {
        let units = self.units;
        let (result, dependencies) = with_dependency_collection(
            || -> Result<(HashSet<WidgetId>, HashSet<u64>), taffy::TaffyError> {
                super::tree::with_widget_stack(|| {
                    let mut removed_ids = HashSet::new();
                    let mut touched_owner_ids = HashSet::new();

                    for root_id in roots {
                        let Some(path) = self.path_for(*root_id).map(|path| path.to_vec()) else {
                            continue;
                        };

                        let previous_ids = self.subtree_widget_ids(*root_id);
                        touched_owner_ids.extend(previous_ids.iter().map(|id| id.raw()));

                        let Some(next) = resolve_subtree_from_source_path(
                            &self.source_root,
                            Some(&self.resolved_root),
                            theme,
                            &path,
                        ) else {
                            continue;
                        };
                        let next_ids = {
                            let mut ids = Vec::new();
                            collect_resolved_widget_ids(&next, &mut ids);
                            ids
                        };
                        let next_id_set: HashSet<_> = next_ids.into_iter().collect();
                        removed_ids.extend(
                            previous_ids
                                .into_iter()
                                .filter(|id| !next_id_set.contains(id)),
                        );

                        patch_layout_at_path(
                            &mut self.resolved_root,
                            &mut self.layout_root,
                            &path,
                            next,
                            &mut self.taffy,
                            animations,
                            theme,
                            units,
                            viewport,
                            now,
                            None,
                            true,
                        )?;
                        self.rebuild_indexes();
                    }

                    self.taffy.compute_layout_with_measure(
                        self.layout_root.node,
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
                    )?;

                    Ok((removed_ids, touched_owner_ids))
                })
            },
        );
        let (removed_ids, touched_owner_ids) = result?;
        self.dependencies.remove_widget_owners(&touched_owner_ids);
        self.dependencies.merge_from(&dependencies);
        self.rebuild_indexes();
        Ok(removed_ids)
    }

    pub(crate) fn patch_resolved_roots(&mut self, roots: &[WidgetId], theme: &Theme) -> bool {
        for root_id in roots {
            let Some(path) = self.path_for(*root_id).map(|path| path.to_vec()) else {
                continue;
            };
            let Some(next) = resolve_subtree_from_source_path(
                &self.source_root,
                Some(&self.resolved_root),
                theme,
                &path,
            ) else {
                return false;
            };
            if !patch_resolved_at_path(&mut self.resolved_root, &path, next) {
                return false;
            }
        }
        self.rebuild_indexes();
        true
    }
}
