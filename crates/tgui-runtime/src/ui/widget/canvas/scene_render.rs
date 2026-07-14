use super::*;

#[derive(Default)]
pub(crate) struct CanvasRenderOutput {
    pub textures: Vec<TexturePrimitive>,
    pub meshes: Vec<MeshPrimitive>,
    pub texts: Vec<TextPrimitive>,
    pub commands: Vec<RenderCommand>,
}

#[derive(Clone)]
pub(crate) enum CanvasHitGeometry {
    Quad([Point; 4]),
    Triangles(Arc<[[Point; 3]]>),
}

#[derive(Clone)]
pub(crate) struct CanvasTextHitEntry {
    pub hit: CanvasTextHit,
    pub quad: [Point; 4],
}

pub(crate) struct CanvasSceneItemRender {
    pub item_id: CanvasItemId,
    pub cursor: Option<CursorStyle>,
    pub hit_bounds: Option<RectBounds>,
    pub hit_geometry: Option<CanvasHitGeometry>,
    pub local_origin: Point,
    pub inverse_transform: CanvasTransform2D,
    pub text_hits: Arc<[CanvasTextHitEntry]>,
    pub output: CanvasRenderOutput,
}

pub(crate) fn tessellate_canvas_scene_items(
    scene: &CanvasScene,
    origin: Point,
    opacity: f32,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    collect_hit_metadata: bool,
    font_manager: &FontManager,
    media: &MediaManager,
    units: UnitContext,
) -> Vec<CanvasSceneItemRender> {
    let clip = CanvasClipContext {
        clip_rect,
        clip_mask,
    };
    scene
        .items()
        .iter()
        .map(|item| {
            let output = item.tessellate(origin, opacity, clip, media, units);
            let text_hits = if collect_hit_metadata {
                item_text_hits(item, font_manager, origin, units)
            } else {
                Arc::from([])
            };
            CanvasSceneItemRender {
                item_id: item.id(),
                cursor: collect_hit_metadata
                    .then_some(item.style().cursor)
                    .flatten(),
                hit_bounds: collect_hit_metadata.then(|| item.hit_bounds()).flatten(),
                hit_geometry: collect_hit_metadata
                    .then(|| item_hit_geometry(item, &output, origin, text_hits.as_ref()))
                    .flatten(),
                local_origin: collect_hit_metadata
                    .then(|| item_local_origin(item))
                    .unwrap_or(Point::ZERO),
                inverse_transform: collect_hit_metadata
                    .then(|| {
                        item.style()
                            .transform
                            .inverse()
                            .unwrap_or(CanvasTransform2D::IDENTITY)
                    })
                    .unwrap_or(CanvasTransform2D::IDENTITY),
                text_hits,
                output,
            }
        })
        .collect()
}

pub(crate) fn item_local_origin(item: &CanvasItem) -> Point {
    match item {
        CanvasItem::Path(path) => path
            .path
            .control_bounds()
            .map(|bounds| Point::new(bounds.min.x, bounds.min.y))
            .unwrap_or(Point::ZERO),
        CanvasItem::Text(text) => Point::new(text.frame.x, text.frame.y),
        CanvasItem::Image(image) => Point::new(image.frame.x, image.frame.y),
        CanvasItem::Group(group) => group_shape_bounds(&group.shape)
            .map(|bounds| Point::new(bounds.min_x, bounds.min_y))
            .unwrap_or(Point::ZERO),
    }
}

fn item_hit_geometry(
    item: &CanvasItem,
    output: &CanvasRenderOutput,
    origin: Point,
    text_hits: &[CanvasTextHitEntry],
) -> Option<CanvasHitGeometry> {
    match item {
        CanvasItem::Path(_) | CanvasItem::Group(_) => {
            let triangles = output
                .meshes
                .iter()
                .flat_map(|mesh| mesh.triangles.iter().copied())
                .collect::<Vec<_>>();
            (!triangles.is_empty()).then(|| CanvasHitGeometry::Triangles(Arc::from(triangles)))
        }
        CanvasItem::Text(text) => {
            if !text_hits.is_empty() {
                let triangles = text_hits
                    .iter()
                    .flat_map(|entry| {
                        let quad = entry.quad;
                        [[quad[0], quad[1], quad[2]], [quad[0], quad[2], quad[3]]]
                    })
                    .collect::<Vec<_>>();
                return Some(CanvasHitGeometry::Triangles(Arc::from(triangles)));
            }

            output
                .texts
                .first()
                .map(|primitive| {
                    primitive
                        .quad
                        .unwrap_or_else(|| rect_to_quad(primitive.frame))
                })
                .or_else(|| Some(rect_to_quad(offset_rect(text.frame, origin))))
                .map(CanvasHitGeometry::Quad)
        }
        CanvasItem::Image(image) => output
            .textures
            .first()
            .map(|primitive| {
                primitive
                    .quad
                    .unwrap_or_else(|| rect_to_quad(primitive.frame))
            })
            .or_else(|| Some(rect_to_quad(offset_rect(image.frame, origin))))
            .map(CanvasHitGeometry::Quad),
    }
}
