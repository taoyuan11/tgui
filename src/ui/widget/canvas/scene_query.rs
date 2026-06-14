use std::cell::OnceCell;
use std::collections::HashMap;

use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasSceneHit {
    pub item_id: CanvasItemId,
    pub name: Option<String>,
    pub kind: CanvasItemKind,
    pub depth: usize,
    pub index_path: Vec<usize>,
    pub scene_position: Point,
    pub local_position: Point,
    pub bounds: Option<Rect>,
    pub text_hit: Option<CanvasTextHit>,
}

pub struct CanvasSceneQueryOptions {
    font_catalog: FontCatalog,
    scale_factor: f32,
    font_scale: f32,
    include_text_hits: bool,
    cached_font_manager: OnceCell<FontManager>,
}

impl Default for CanvasSceneQueryOptions {
    fn default() -> Self {
        Self {
            font_catalog: FontCatalog::default(),
            scale_factor: 1.0,
            font_scale: 1.0,
            include_text_hits: true,
            cached_font_manager: OnceCell::new(),
        }
    }
}

thread_local! {
    static DEFAULT_QUERY_OPTIONS: CanvasSceneQueryOptions = CanvasSceneQueryOptions::default();
}

pub(crate) fn with_default_canvas_scene_query_options<T>(
    f: impl FnOnce(&CanvasSceneQueryOptions) -> T,
) -> T {
    DEFAULT_QUERY_OPTIONS.with(f)
}

impl CanvasSceneQueryOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn scale_factor(mut self, scale_factor: f32) -> Self {
        self.scale_factor = scale_factor;
        self
    }

    pub fn font_scale(mut self, font_scale: f32) -> Self {
        self.font_scale = font_scale;
        self
    }

    pub fn include_text_hits(mut self, include_text_hits: bool) -> Self {
        self.include_text_hits = include_text_hits;
        self
    }

    pub fn without_text_hits(self) -> Self {
        self.include_text_hits(false)
    }

    pub fn font_bytes(mut self, name: impl Into<String>, bytes: &'static [u8]) -> Self {
        self.font_catalog.register_font(name, bytes);
        self.cached_font_manager = OnceCell::new();
        self
    }

    pub fn font_file(
        mut self,
        name: impl Into<String>,
        path: impl Into<std::path::PathBuf>,
    ) -> Self {
        self.font_catalog.register_font_file(name, path);
        self.cached_font_manager = OnceCell::new();
        self
    }

    pub fn default_font(mut self, name: impl Into<String>) -> Self {
        self.font_catalog.set_default_font(name);
        self.cached_font_manager = OnceCell::new();
        self
    }

    pub(crate) fn font_manager(&self) -> &FontManager {
        self.cached_font_manager
            .get_or_init(|| FontManager::new(&self.font_catalog))
    }

    pub(crate) fn should_include_text_hits(&self) -> bool {
        self.include_text_hits
    }

    pub(crate) fn units(&self) -> UnitContext {
        UnitContext::new(self.scale_factor, self.font_scale)
    }
}

pub(crate) fn query_canvas_scene_hits(
    scene: &CanvasScene,
    font_manager: Option<&FontManager>,
    units: UnitContext,
    include_text_hits: bool,
    scene_position: Point,
) -> Vec<CanvasSceneHit> {
    let query_session = CanvasSceneQuerySession::new(font_manager, units, include_text_hits);
    let mut hits = Vec::new();
    let mut index_path = Vec::new();
    collect_query_hits_recursive(
        scene.items(),
        scene_position,
        &mut index_path,
        &query_session,
        &mut hits,
    );
    hits
}

fn collect_query_hits_recursive(
    items: &[CanvasItem],
    scene_position: Point,
    index_path: &mut Vec<usize>,
    query_session: &CanvasSceneQuerySession<'_>,
    hits: &mut Vec<CanvasSceneHit>,
) {
    for index in (0..items.len()).rev() {
        let item = &items[index];
        index_path.push(index);
        if !item.style().visible || !item.style().hit_test {
            index_path.pop();
            continue;
        }

        let contains = item_contains_scene_point(item, scene_position);
        if let CanvasItem::Group(group) = item {
            if contains {
                collect_query_hits_recursive(
                    &group.items,
                    scene_position,
                    index_path,
                    query_session,
                    hits,
                );
            }
        }

        if contains {
            let local_position = item_event_local_position(item, scene_position);
            let depth = index_path.len().saturating_sub(1);
            let stored_path = index_path.clone();
            let kind = item.kind();
            let name = item.name().map(ToOwned::to_owned);
            let bounds = item.hit_bounds_rect();
            let text_hit = item_text_hit_at_point(item, scene_position, query_session);
            hits.push(CanvasSceneHit {
                item_id: item.id(),
                name,
                kind,
                depth,
                index_path: stored_path,
                scene_position,
                local_position,
                bounds,
                text_hit,
            });
        }

        index_path.pop();
    }
}

struct CanvasSceneQuerySession<'a> {
    font_manager: Option<&'a FontManager>,
    units: UnitContext,
    include_text_hits: bool,
    text_hit_cache: RefCell<HashMap<u64, Arc<[CanvasTextHitEntry]>>>,
}

impl<'a> CanvasSceneQuerySession<'a> {
    fn new(
        font_manager: Option<&'a FontManager>,
        units: UnitContext,
        include_text_hits: bool,
    ) -> Self {
        Self {
            font_manager,
            units,
            include_text_hits,
            text_hit_cache: RefCell::new(HashMap::new()),
        }
    }

    fn text_hits_for_item(&self, item: &CanvasItem) -> Arc<[CanvasTextHitEntry]> {
        let Some(font_manager) = self.font_manager else {
            return Arc::from([]);
        };
        let cache_key = canvas_text_hit_cache_key(item, self.units);
        if let Some(cached) = self.text_hit_cache.borrow().get(&cache_key).cloned() {
            return cached;
        }
        let computed = item_text_hits(item, font_manager, Point::ZERO, self.units);
        self.text_hit_cache
            .borrow_mut()
            .insert(cache_key, Arc::clone(&computed));
        computed
    }
}

fn item_contains_scene_point(item: &CanvasItem, scene_position: Point) -> bool {
    let Some(bounds) = item.hit_bounds_rect() else {
        return false;
    };
    if !bounds.contains(scene_position) {
        return false;
    }

    match scene_hit_geometry_for_item(item) {
        Some(geometry) => hit_geometry_contains(&geometry, scene_position),
        None => true,
    }
}

fn scene_hit_geometry_for_item(item: &CanvasItem) -> Option<CanvasHitGeometry> {
    match item {
        CanvasItem::Path(path) => path_scene_hit_geometry(path),
        CanvasItem::Text(text) => Some(CanvasHitGeometry::Quad(
            if text.style.transform == CanvasTransform2D::IDENTITY {
                rect_to_quad(text.frame)
            } else {
                transform_rect_quad(text.frame, text.style.transform, Point::ZERO)
            },
        )),
        CanvasItem::Image(image) => Some(CanvasHitGeometry::Quad(
            if image.style.transform == CanvasTransform2D::IDENTITY {
                rect_to_quad(image.frame)
            } else {
                transform_rect_quad(image.frame, image.style.transform, Point::ZERO)
            },
        )),
        CanvasItem::Group(group) => group_scene_hit_geometry(group),
    }
}

fn path_scene_hit_geometry(path: &CanvasPath) -> Option<CanvasHitGeometry> {
    let mut triangles = Vec::new();
    let lyon_path = path.path.to_lyon_path();
    let clip = CanvasClipContext::default();

    if path.fill.is_some() {
        if let Some(mesh) = tessellate_fill(
            &lyon_path,
            path.fill_rule,
            &CanvasBrush::Solid(Color::BLACK),
            1.0,
            Point::ZERO,
            clip,
        ) {
            triangles.extend(mesh.triangles.iter().copied());
        }
    }

    if let Some(stroke) = path.stroke.as_ref() {
        if let Some(mesh) = tessellate_stroke(&lyon_path, stroke, 1.0, Point::ZERO, clip) {
            triangles.extend(mesh.triangles.iter().copied());
        }
    }

    if triangles.is_empty() {
        return None;
    }

    let geometry = CanvasHitGeometry::Triangles(Arc::from(triangles));
    Some(transform_hit_geometry(
        &geometry,
        path.style.transform,
        Point::ZERO,
    ))
}

fn group_scene_hit_geometry(group: &CanvasGroup) -> Option<CanvasHitGeometry> {
    let CanvasGroupShape::Path { path, fill_rule } = &group.shape;
    let lyon_path = path.to_lyon_path();
    let mesh = tessellate_fill(
        &lyon_path,
        *fill_rule,
        &CanvasBrush::Solid(Color::BLACK),
        1.0,
        Point::ZERO,
        CanvasClipContext::default(),
    )?;
    let geometry = CanvasHitGeometry::Triangles(mesh.triangles);
    Some(transform_hit_geometry(
        &geometry,
        group.style.transform,
        Point::ZERO,
    ))
}

fn transform_hit_geometry(
    geometry: &CanvasHitGeometry,
    transform: CanvasTransform2D,
    origin: Point,
) -> CanvasHitGeometry {
    if transform == CanvasTransform2D::IDENTITY {
        return geometry.clone();
    }

    match geometry {
        CanvasHitGeometry::Quad(quad) => CanvasHitGeometry::Quad(quad.map(|point_value| {
            transform.apply(Point::new(
                point_value.x - origin.x,
                point_value.y - origin.y,
            ))
        })),
        CanvasHitGeometry::Triangles(triangles) => {
            let transformed = triangles
                .iter()
                .map(|triangle| {
                    triangle.map(|point_value| {
                        let local = Point::new(point_value.x - origin.x, point_value.y - origin.y);
                        transform.apply(local)
                    })
                })
                .collect::<Vec<_>>();
            CanvasHitGeometry::Triangles(Arc::from(transformed))
        }
    }
}

fn hit_geometry_contains(geometry: &CanvasHitGeometry, point: Point) -> bool {
    match geometry {
        CanvasHitGeometry::Quad(quad) => {
            point_in_triangle(point, quad[0], quad[1], quad[2])
                || point_in_triangle(point, quad[0], quad[2], quad[3])
        }
        CanvasHitGeometry::Triangles(triangles) => triangles
            .iter()
            .any(|triangle| point_in_triangle(point, triangle[0], triangle[1], triangle[2])),
    }
}

fn point_in_triangle(point: Point, a: Point, b: Point, c: Point) -> bool {
    let point_sign = |lhs: Point, rhs: Point, other: Point| {
        (lhs.x.get() - other.x.get()) * (rhs.y.get() - other.y.get())
            - (rhs.x.get() - other.x.get()) * (lhs.y.get() - other.y.get())
    };

    let d1 = point_sign(point, a, b);
    let d2 = point_sign(point, b, c);
    let d3 = point_sign(point, c, a);
    let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(has_neg && has_pos)
}

fn item_event_local_position(item: &CanvasItem, scene_position: Point) -> Point {
    let item_origin = item_local_origin(item);
    let local = Point::new(
        scene_position.x - item_origin.x,
        scene_position.y - item_origin.y,
    );
    let [a, b, c, d, e, f] = item
        .style()
        .transform
        .inverse()
        .unwrap_or(CanvasTransform2D::IDENTITY)
        .matrix;
    Point::new(
        a * local.x.get() + c * local.y.get() + e,
        b * local.x.get() + d * local.y.get() + f,
    )
}

fn item_text_hit_at_point(
    item: &CanvasItem,
    scene_position: Point,
    query_session: &CanvasSceneQuerySession<'_>,
) -> Option<CanvasTextHit> {
    let CanvasItem::Text(_) = item else {
        return None;
    };
    if !query_session.include_text_hits {
        return None;
    }
    let text_hits = query_session.text_hits_for_item(item);
    text_hits
        .iter()
        .find(|entry| hit_geometry_contains(&CanvasHitGeometry::Quad(entry.quad), scene_position))
        .map(|entry| entry.hit)
}
