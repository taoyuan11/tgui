use super::*;

pub(crate) fn canvas_bounds(items: &[CanvasItem]) -> Option<RectBounds> {
    let mut bounds: Option<RectBounds> = None;
    for item in items {
        if let Some(item_bounds) = item.layout_bounds() {
            bounds = Some(match bounds {
                Some(existing) => existing.union(item_bounds),
                None => item_bounds,
            });
        }
    }
    bounds
}

#[derive(Clone, Default, PartialEq)]
pub struct CanvasScene {
    pub(crate) items: Vec<CanvasItem>,
}

impl CanvasScene {
    pub const STABLE_JSON_FORMAT: &str = "tgui.canvas.scene";
    pub const STABLE_JSON_VERSION: u32 = 1;

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn from_items(items: impl Into<Vec<CanvasItem>>) -> Self {
        Self {
            items: items.into(),
        }
    }

    pub fn items(&self) -> &[CanvasItem] {
        &self.items
    }

    pub fn items_mut(&mut self) -> &mut Vec<CanvasItem> {
        &mut self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn bounds(&self) -> Option<Rect> {
        canvas_scene_bounds(self).map(rect_from_bounds)
    }

    pub fn push(&mut self, item: impl Into<CanvasItem>) {
        self.items.push(item.into());
    }

    pub fn insert(&mut self, index: usize, item: impl Into<CanvasItem>) {
        self.items.insert(index.min(self.items.len()), item.into());
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn remove(&mut self, id: CanvasItemId) -> Option<CanvasItem> {
        remove_item_by_id(&mut self.items, id)
    }

    pub fn contains_id(&self, id: CanvasItemId) -> bool {
        self.find(id).is_some()
    }

    pub fn contains_name(&self, name: &str) -> bool {
        self.find_named(name).is_some()
    }

    pub fn find(&self, id: CanvasItemId) -> Option<&CanvasItem> {
        find_item_by_id(&self.items, id)
    }

    pub fn find_mut(&mut self, id: CanvasItemId) -> Option<&mut CanvasItem> {
        find_item_mut_by_id(&mut self.items, id)
    }

    pub fn find_named(&self, name: &str) -> Option<&CanvasItem> {
        find_item_by_name(&self.items, name)
    }

    pub fn find_named_mut(&mut self, name: &str) -> Option<&mut CanvasItem> {
        find_item_mut_by_name(&mut self.items, name)
    }

    pub fn visit(&self, mut visitor: impl FnMut(CanvasSceneVisit<'_>)) {
        let mut index_path = Vec::new();
        visit_scene_items(&self.items, 0, &mut index_path, &mut visitor);
    }

    pub fn debug_info(&self) -> CanvasSceneDebugInfo {
        build_canvas_scene_debug_info(self)
    }

    pub fn query_point(&self, scene_position: Point) -> Option<CanvasSceneHit> {
        self.query_point_with(&CanvasSceneQueryOptions::default(), scene_position)
    }

    pub fn query_point_all(&self, scene_position: Point) -> Vec<CanvasSceneHit> {
        self.query_point_all_with(&CanvasSceneQueryOptions::default(), scene_position)
    }

    pub fn query_point_with(
        &self,
        options: &CanvasSceneQueryOptions,
        scene_position: Point,
    ) -> Option<CanvasSceneHit> {
        self.query_point_all_with(options, scene_position)
            .into_iter()
            .next()
    }

    pub fn query_point_all_with(
        &self,
        options: &CanvasSceneQueryOptions,
        scene_position: Point,
    ) -> Vec<CanvasSceneHit> {
        query_canvas_scene_hits(self, options.font_manager(), options.units(), scene_position)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn query_point_with_runtime_context(
        &self,
        font_manager: &FontManager,
        units: UnitContext,
        scene_position: Point,
    ) -> Option<CanvasSceneHit> {
        self.query_point_all_with_runtime_context(font_manager, units, scene_position)
            .into_iter()
            .next()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn query_point_all_with_runtime_context(
        &self,
        font_manager: &FontManager,
        units: UnitContext,
        scene_position: Point,
    ) -> Vec<CanvasSceneHit> {
        query_canvas_scene_hits(self, font_manager, units, scene_position)
    }

    pub fn export_json(&self) -> String {
        export_canvas_scene_json(self)
    }

    pub fn export_debug_text(&self) -> String {
        self.debug_info().to_pretty_text()
    }

    pub fn export_debug_json(&self) -> String {
        self.debug_info().to_pretty_json()
    }
}

#[derive(Debug)]
pub struct CanvasSceneVisit<'a> {
    pub depth: usize,
    pub index_path: Vec<usize>,
    pub item: &'a CanvasItem,
}

fn visit_scene_items<'a>(
    items: &'a [CanvasItem],
    depth: usize,
    index_path: &mut Vec<usize>,
    visitor: &mut impl FnMut(CanvasSceneVisit<'a>),
) {
    for (index, item) in items.iter().enumerate() {
        index_path.push(index);
        visitor(CanvasSceneVisit {
            depth,
            index_path: index_path.clone(),
            item,
        });
        if let CanvasItem::Group(group) = item {
            visit_scene_items(&group.items, depth + 1, index_path, visitor);
        }
        index_path.pop();
    }
}

fn find_item_by_id(items: &[CanvasItem], id: CanvasItemId) -> Option<&CanvasItem> {
    for item in items {
        if item.id() == id {
            return Some(item);
        }
        if let CanvasItem::Group(group) = item {
            if let Some(found) = find_item_by_id(&group.items, id) {
                return Some(found);
            }
        }
    }
    None
}

fn find_item_mut_by_id(items: &mut [CanvasItem], id: CanvasItemId) -> Option<&mut CanvasItem> {
    for item in items.iter_mut() {
        if item.id() == id {
            return Some(item);
        }
        if let CanvasItem::Group(group) = item {
            if let Some(found) = find_item_mut_by_id(&mut group.items, id) {
                return Some(found);
            }
        }
    }
    None
}

fn find_item_by_name<'a>(items: &'a [CanvasItem], name: &str) -> Option<&'a CanvasItem> {
    for item in items {
        if item.name() == Some(name) {
            return Some(item);
        }
        if let CanvasItem::Group(group) = item {
            if let Some(found) = find_item_by_name(&group.items, name) {
                return Some(found);
            }
        }
    }
    None
}

fn find_item_mut_by_name<'a>(
    items: &'a mut [CanvasItem],
    name: &str,
) -> Option<&'a mut CanvasItem> {
    for item in items.iter_mut() {
        if item.name() == Some(name) {
            return Some(item);
        }
        if let CanvasItem::Group(group) = item {
            if let Some(found) = find_item_mut_by_name(&mut group.items, name) {
                return Some(found);
            }
        }
    }
    None
}

fn remove_item_by_id(items: &mut Vec<CanvasItem>, id: CanvasItemId) -> Option<CanvasItem> {
    let mut index = 0;
    while index < items.len() {
        if items[index].id() == id {
            return Some(items.remove(index));
        }
        if let CanvasItem::Group(group) = &mut items[index] {
            if let Some(removed) = remove_item_by_id(&mut group.items, id) {
                return Some(removed);
            }
        }
        index += 1;
    }
    None
}

pub(crate) fn canvas_scene_bounds(scene: &CanvasScene) -> Option<RectBounds> {
    canvas_bounds(scene.items())
}
