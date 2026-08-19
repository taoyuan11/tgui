use tgui::core::{FontHandle, ResourceId};
use tgui::text::{
    GlyphAtlas, GlyphAtlasConfig, GlyphAtlasKey, GlyphCompletionOutcome, GlyphContentType,
    GlyphKey, GlyphLookup, GlyphRaster, GlyphRasterCompletion, GlyphRasterRequest, GlyphVariant,
    PhysicalFontSize,
};

fn atlas_key(
    font_slot: u32,
    physical_pixels: f32,
    variant: u64,
    content_type: GlyphContentType,
) -> GlyphAtlasKey {
    GlyphAtlasKey::new(
        FontHandle::from_parts(font_slot, 1),
        PhysicalFontSize::from_pixels(physical_pixels).unwrap(),
        GlyphVariant::new(variant),
        content_type,
    )
}

fn raster_request(lookup: GlyphLookup) -> GlyphRasterRequest {
    let GlyphLookup::Rasterize(request) = lookup else {
        panic!("expected raster request")
    };
    request
}

fn ready(outcome: GlyphCompletionOutcome) -> GlyphRasterCompletion {
    let GlyphCompletionOutcome::Ready(completion) = outcome else {
        panic!("expected ready glyph")
    };
    completion
}

#[test]
fn atlas_identity_includes_font_physical_size_variant_and_pixel_kind() {
    let base = atlas_key(1, 16.0, 0, GlyphContentType::Mask);
    assert_ne!(base, atlas_key(2, 16.0, 0, GlyphContentType::Mask));
    assert_ne!(base, atlas_key(1, 20.0, 0, GlyphContentType::Mask));
    assert_ne!(base, atlas_key(1, 16.0, 7, GlyphContentType::Mask));
    assert_ne!(base, atlas_key(1, 16.0, 0, GlyphContentType::Color));

    let fractional = PhysicalFontSize::from_pixels(16.25).unwrap();
    assert_eq!(fractional.subpixels(), 1040);
    assert_eq!(fractional.pixels(), 16.25);
}

#[test]
fn pages_are_allocated_per_key_and_rectangles_do_not_overlap() {
    let mut atlas = GlyphAtlas::new(
        GlyphAtlasConfig::new(8, 8, 3)
            .with_padding(0)
            .with_max_bytes(8 * 8 * 6),
    )
    .unwrap();
    let run = ResourceId::from_parts(1, 1);
    let key = atlas_key(1, 16.0, 0, GlyphContentType::Mask);
    let first = GlyphKey::new(key, 1);
    let second = GlyphKey::new(key, 2);

    let first_request = raster_request(atlas.lookup(first, run).unwrap());
    let first = ready(
        atlas
            .complete_raster(first_request, GlyphRaster::new(5, 5, vec![1; 25]))
            .unwrap(),
    );
    let second_request = raster_request(atlas.lookup(second, run).unwrap());
    let second = ready(
        atlas
            .complete_raster(second_request, GlyphRaster::new(5, 5, vec![2; 25]))
            .unwrap(),
    );

    assert_ne!(first.placement.page, second.placement.page);
    assert_eq!(first.placement.page.generation(), 1);
    assert_eq!(second.placement.page.generation(), 1);
    assert_eq!(atlas.stats().active_pages, 2);
}

#[test]
fn eviction_changes_page_generation_and_stale_worker_result_is_ignored() {
    let mut atlas = GlyphAtlas::new(
        GlyphAtlasConfig::new(8, 8, 1)
            .with_padding(0)
            .with_max_bytes(64),
    )
    .unwrap();
    let key = atlas_key(1, 16.0, 0, GlyphContentType::Mask);
    let run_a = ResourceId::from_parts(10, 1);
    let run_b = ResourceId::from_parts(11, 1);
    let unrelated = ResourceId::from_parts(12, 1);
    let glyph_a = GlyphKey::new(key, 1);
    let glyph_b = GlyphKey::new(key, 2);

    let request_a = raster_request(atlas.lookup(glyph_a, run_a).unwrap());
    let first = ready(
        atlas
            .complete_raster(request_a, GlyphRaster::new(5, 5, vec![1; 25]))
            .unwrap(),
    );
    // A duplicate completion from the worker is rejected before page mutation.
    assert!(matches!(
        atlas
            .complete_raster(request_a, GlyphRaster::new(5, 5, vec![1; 25]))
            .unwrap(),
        GlyphCompletionOutcome::Stale(request) if request == request_a
    ));

    let request_b = raster_request(atlas.lookup(glyph_b, run_b).unwrap());
    let second = ready(
        atlas
            .complete_raster(request_b, GlyphRaster::new(5, 5, vec![2; 25]))
            .unwrap(),
    );
    assert_eq!(first.placement.page.slot(), second.placement.page.slot());
    assert!(second.placement.page.generation() > first.placement.page.generation());
    assert!(!atlas.is_page_current(first.placement.page));
    assert!(!atlas.is_placement_current(first.placement));
    assert!(atlas.is_placement_current(second.placement));

    // The completion reports the newly ready run and the run affected by the
    // page eviction, but never dirties unrelated text or logical layout.
    assert_eq!(second.invalidation.runs, vec![run_a, run_b]);
    assert!(!second.invalidation.runs.contains(&unrelated));
    assert!(second.invalidation.phases.resource());
    assert!(second.invalidation.phases.paint());
    assert!(!second.invalidation.phases.layout());

    // Shaped glyph identity survives independently; only this evicted glyph
    // asks for another raster request when its run is painted again.
    assert!(matches!(
        atlas.lookup(glyph_a, run_a).unwrap(),
        GlyphLookup::Rasterize(_)
    ));
    assert!(matches!(
        atlas.lookup(glyph_b, run_b).unwrap(),
        GlyphLookup::Resident(_)
    ));
    assert_eq!(atlas.stats().page_evictions, 1);
    assert_eq!(atlas.stats().glyph_evictions, 1);
}

#[test]
fn pending_requests_are_deduplicated_and_only_dependent_runs_are_invalidated() {
    let mut atlas = GlyphAtlas::new(GlyphAtlasConfig::new(16, 16, 1)).unwrap();
    let glyph = GlyphKey::new(atlas_key(1, 14.0, 9, GlyphContentType::Mask), 42);
    let run_a = ResourceId::from_parts(1, 1);
    let run_b = ResourceId::from_parts(2, 1);
    let first = raster_request(atlas.lookup(glyph, run_a).unwrap());
    let coalesced = raster_request(atlas.lookup(glyph, run_b).unwrap());
    assert_eq!(first, coalesced);

    let completion = ready(
        atlas
            .complete_raster(first, GlyphRaster::new(2, 3, vec![255; 6]))
            .unwrap(),
    );
    assert_eq!(completion.invalidation.runs, vec![run_a, run_b]);
    assert_eq!(atlas.stats().raster_requests, 1);
    assert_eq!(atlas.stats().coalesced_requests, 1);
    assert_eq!(atlas.stats().raster_completions, 1);
}

#[test]
fn byte_budget_accounts_for_mask_and_color_page_formats() {
    let mut atlas = GlyphAtlas::new(
        GlyphAtlasConfig::new(4, 4, 2)
            .with_padding(0)
            .with_max_bytes(64),
    )
    .unwrap();
    let run = ResourceId::from_parts(1, 1);
    let color = GlyphKey::new(atlas_key(1, 16.0, 0, GlyphContentType::Color), 1);
    let color_request = raster_request(atlas.lookup(color, run).unwrap());
    ready(
        atlas
            .complete_raster(color_request, GlyphRaster::new(1, 1, vec![0; 4]))
            .unwrap(),
    );
    assert_eq!(atlas.stats().resident_bytes, 64);

    let mask = GlyphKey::new(atlas_key(1, 16.0, 0, GlyphContentType::Mask), 2);
    let mask_request = raster_request(atlas.lookup(mask, run).unwrap());
    let mask = ready(
        atlas
            .complete_raster(mask_request, GlyphRaster::new(1, 1, vec![0]))
            .unwrap(),
    );
    assert_eq!(atlas.stats().resident_bytes, 16);
    assert_eq!(atlas.stats().page_evictions, 1);
    assert_eq!(mask.placement.page.generation(), 2);
    assert_eq!(atlas.stats().peak_bytes, 64);
}
