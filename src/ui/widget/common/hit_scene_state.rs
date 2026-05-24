use super::*;
use crate::runtime::overlay::{AnchorKey, AnchorSource};
use crate::ui::widget::VirtualSceneStateUpdate;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ScrollRegion {
    pub id: WidgetId,
    pub content_viewport: Rect,
    pub visible_frame: Rect,
    pub content_bounds: Rect,
    pub scroll_offset: Point,
    pub overflow_x: Overflow,
    pub overflow_y: Overflow,
    pub horizontal_track: Option<Rect>,
    pub horizontal_thumb: Option<Rect>,
    pub vertical_track: Option<Rect>,
    pub vertical_thumb: Option<Rect>,
}

impl ScrollRegion {
    pub(crate) fn max_offset(self) -> Point {
        Point {
            x: (self.content_bounds.right() - self.content_viewport.right()).max(0.0),
            y: (self.content_bounds.bottom() - self.content_viewport.bottom()).max(0.0),
        }
    }

    pub(crate) fn can_scroll_x(self) -> bool {
        self.overflow_x == Overflow::Scroll && self.max_offset().x > Dp::ZERO
    }

    pub(crate) fn can_scroll_y(self) -> bool {
        self.overflow_y == Overflow::Scroll && self.max_offset().y > Dp::ZERO
    }
}

pub(crate) struct ComputedScene<VM> {
    pub scene: ScenePrimitives,
    pub hit_regions: Vec<HitRegion<VM>>,
    pub overlay_hit_regions: Vec<HitRegion<VM>>,
    pub overlay_close_handlers: Vec<crate::runtime::overlay::OverlayCloseHandle<VM>>,
    pub focus_scopes: Vec<FocusScopeState>,
    pub overlay_anchors: HashMap<AnchorKey, Rect>,
    /// 每个 `OverlayLayer` 的暂存桶。`emit_overlay` 写入此处，
    /// `finalize_overlay_layers` 在 collect 收尾时按 layer 顺序合并到 `scene.overlay_*` /
    /// `overlay_hit_regions` / `overlay_close_handlers`，从而强制 z-order
    /// （Tooltip < Popover < Menu < Modal）。
    pub overlay_layers: [OverlayLayerBucket<VM>; OVERLAY_LAYER_COUNT],
    pub scroll_regions: Vec<ScrollRegion>,
    pub ime_cursor_area: Option<Rect>,
    pub virtual_state_updates: Vec<VirtualSceneStateUpdate>,
    pub(crate) dependencies: DependencyGraph,
}

impl<VM> Clone for ComputedScene<VM> {
    fn clone(&self) -> Self {
        Self {
            scene: self.scene.clone(),
            hit_regions: self.hit_regions.clone(),
            overlay_hit_regions: self.overlay_hit_regions.clone(),
            overlay_close_handlers: self.overlay_close_handlers.clone(),
            focus_scopes: self.focus_scopes.clone(),
            overlay_anchors: self.overlay_anchors.clone(),
            overlay_layers: std::array::from_fn(|i| self.overlay_layers[i].clone()),
            scroll_regions: self.scroll_regions.clone(),
            ime_cursor_area: self.ime_cursor_area,
            virtual_state_updates: self.virtual_state_updates.clone(),
            dependencies: self.dependencies.clone(),
        }
    }
}

pub(crate) const OVERLAY_LAYER_COUNT: usize = 4;

/// 单个 `OverlayLayer` 的暂存桶。
pub(crate) struct OverlayLayerBucket<VM> {
    pub commands: Vec<RenderCommand>,
    pub backdrop_blurs: Vec<BackdropBlurPrimitive>,
    pub shapes: Vec<RenderPrimitive>,
    pub textures: Vec<TexturePrimitive>,
    pub meshes: Vec<MeshPrimitive>,
    pub texts: Vec<TextPrimitive>,
    pub hits: Vec<HitRegion<VM>>,
    pub close_handlers: Vec<crate::runtime::overlay::OverlayCloseHandle<VM>>,
    pub focus_scopes: Vec<FocusScopeState>,
}

impl<VM> Default for OverlayLayerBucket<VM> {
    fn default() -> Self {
        Self {
            commands: Vec::new(),
            backdrop_blurs: Vec::new(),
            shapes: Vec::new(),
            textures: Vec::new(),
            meshes: Vec::new(),
            texts: Vec::new(),
            hits: Vec::new(),
            close_handlers: Vec::new(),
            focus_scopes: Vec::new(),
        }
    }
}

impl<VM> Clone for OverlayLayerBucket<VM> {
    fn clone(&self) -> Self {
        Self {
            commands: self.commands.clone(),
            backdrop_blurs: self.backdrop_blurs.clone(),
            shapes: self.shapes.clone(),
            textures: self.textures.clone(),
            meshes: self.meshes.clone(),
            texts: self.texts.clone(),
            hits: self.hits.clone(),
            close_handlers: self.close_handlers.clone(),
            focus_scopes: self.focus_scopes.clone(),
        }
    }
}

impl<VM> OverlayLayerBucket<VM> {
    fn extend_from(&mut self, other: &Self) {
        self.commands.extend(other.commands.iter().cloned());
        self.backdrop_blurs
            .extend(other.backdrop_blurs.iter().copied());
        self.shapes.extend(other.shapes.iter().copied());
        self.textures.extend(other.textures.iter().cloned());
        self.meshes.extend(other.meshes.iter().cloned());
        self.texts.extend(other.texts.iter().cloned());
        self.hits.extend(other.hits.iter().cloned());
        self.close_handlers
            .extend(other.close_handlers.iter().cloned());
        self.focus_scopes.extend(other.focus_scopes.iter().cloned());
    }
}

#[derive(Clone, Default)]
pub(crate) struct WidgetStateMap {
    states: HashMap<WidgetId, WidgetState>,
    select_option_states: HashMap<(WidgetId, usize), WidgetState>,
}

impl WidgetStateMap {
    pub(crate) fn set(&mut self, id: WidgetId, state: WidgetState) {
        self.states.insert(id, state);
    }

    pub(crate) fn get(&self, id: WidgetId) -> WidgetState {
        self.states.get(&id).copied().unwrap_or_default()
    }

    pub(crate) fn set_select_option(
        &mut self,
        widget_id: WidgetId,
        option_index: usize,
        state: WidgetState,
    ) {
        self.select_option_states
            .insert((widget_id, option_index), state);
    }

    pub(crate) fn get_select_option(
        &self,
        widget_id: WidgetId,
        option_index: usize,
    ) -> WidgetState {
        self.select_option_states
            .get(&(widget_id, option_index))
            .copied()
            .unwrap_or_default()
    }
}

impl<VM> Default for ComputedScene<VM> {
    fn default() -> Self {
        Self {
            scene: ScenePrimitives::default(),
            hit_regions: Vec::new(),
            overlay_hit_regions: Vec::new(),
            overlay_close_handlers: Vec::new(),
            focus_scopes: Vec::new(),
            overlay_anchors: HashMap::new(),
            overlay_layers: std::array::from_fn(|_| OverlayLayerBucket::default()),
            scroll_regions: Vec::new(),
            ime_cursor_area: None,
            virtual_state_updates: Vec::new(),
            dependencies: DependencyGraph::default(),
        }
    }
}

impl<VM> ComputedScene<VM> {
    pub(crate) fn extend(&mut self, other: &ComputedScene<VM>) {
        self.scene.extend(&other.scene);
        self.hit_regions.extend(other.hit_regions.iter().cloned());
        self.overlay_hit_regions
            .extend(other.overlay_hit_regions.iter().cloned());
        self.overlay_close_handlers
            .extend(other.overlay_close_handlers.iter().cloned());
        self.focus_scopes.extend(other.focus_scopes.iter().cloned());
        self.overlay_anchors
            .extend(other.overlay_anchors.iter().map(|(k, v)| (*k, *v)));
        for i in 0..OVERLAY_LAYER_COUNT {
            self.overlay_layers[i].extend_from(&other.overlay_layers[i]);
        }
        self.scroll_regions
            .extend(other.scroll_regions.iter().copied());
        if self.ime_cursor_area.is_none() {
            self.ime_cursor_area = other.ime_cursor_area;
        }
        self.virtual_state_updates
            .extend(other.virtual_state_updates.iter().cloned());
        self.dependencies.merge_from(&other.dependencies);
    }

    pub(crate) fn finalize_overlay_layers(&mut self) {
        for layer in crate::runtime::overlay::OverlayLayer::ALL {
            let bucket = std::mem::take(&mut self.overlay_layers[layer.index()]);
            for blur in bucket.backdrop_blurs {
                self.scene.backdrop_blurs.push(blur);
            }
            for shape in bucket.shapes {
                self.scene.overlay_shapes.push(shape);
            }
            for tex in bucket.textures {
                self.scene.overlay_textures.push(tex);
            }
            for mesh in bucket.meshes {
                self.scene.overlay_meshes.push(mesh);
            }
            for text in bucket.texts {
                self.scene.overlay_texts.push(text);
            }
            self.scene.overlay_commands.extend(bucket.commands);
            self.overlay_hit_regions.extend(bucket.hits);
            self.overlay_close_handlers.extend(bucket.close_handlers);
            self.focus_scopes.extend(bucket.focus_scopes);
        }
    }

    pub(crate) fn register_overlay_anchor(&mut self, key: AnchorKey, rect: Rect) {
        self.overlay_anchors.insert(key, rect);
    }

    pub(crate) fn register_widget_overlay_anchor(&mut self, widget_id: WidgetId, rect: Rect) {
        self.register_overlay_anchor(AnchorKey::widget(widget_id), rect);
    }

    pub(crate) fn register_caret_overlay_anchor(&mut self, widget_id: WidgetId, rect: Rect) {
        self.register_overlay_anchor(AnchorKey::caret(widget_id), rect);
    }

    pub(crate) fn register_focus_scope(&mut self, scope: FocusScopeState) {
        self.focus_scopes.push(scope);
    }

    pub(crate) fn resolve_overlay_anchor(&self, key: AnchorKey) -> Option<Rect> {
        self.overlay_anchors.get(&key).copied().or_else(|| match key.source() {
            AnchorSource::Caret(_) => None,
            AnchorSource::Widget(_) | AnchorSource::Point => None,
        })
    }

    #[cfg(test)]
    pub(crate) fn rendered(&self) -> RenderedWidgetScene {
        RenderedWidgetScene {
            primitives: self.scene.clone(),
            scroll_regions: self.scroll_regions.clone(),
            ime_cursor_area: self.ime_cursor_area,
        }
    }
}
