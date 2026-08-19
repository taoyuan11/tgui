use std::env;
use std::hint::black_box;
use std::rc::Rc;
use std::time::{Duration, Instant};

use tgui::Result;
use tgui::animation::{Animated, AnimationImpact, AnimationKey, AnimationSpec, Timeline};
#[cfg(not(feature = "text"))]
use tgui::core::Size;
use tgui::core::{
    Color, DpiScale, ElementId, FontHandle, ItemKey, PropertyId, Rect, ResourceId, SceneRevision,
};
use tgui::diagnostics::FrameMetrics;
use tgui::media::{
    DecodedImage, ImageCompletion, ImageDecodeResult, ImageRegistry, ImageRequestKey, ImageSize,
    ImageSource,
};
use tgui::render::{
    BackdropFilter, CompileContext, LayerSpec, PaintCommand, RenderCompiler, RendererCapabilities,
};
use tgui::test_support::FakeClock;
#[cfg(not(feature = "text"))]
use tgui::test_support::RenderHarness;
use tgui::text::{
    GlyphAtlas, GlyphAtlasConfig, GlyphAtlasKey, GlyphCompletionOutcome, GlyphContentType,
    GlyphKey, GlyphLookup, GlyphRaster, GlyphVariant, PhysicalFontSize,
};
use tgui::virtualization::{VirtualList, VirtualListDataSource};
#[cfg(not(feature = "text"))]
use tgui::widget::Widget;
use tgui::widget::{BuildContext, WidgetNode};
#[cfg(not(feature = "text"))]
use tgui::widgets::{Container, Text};

const DEFAULT_SAMPLES: usize = 5;

struct Rows(usize);
struct Row;

impl VirtualListDataSource for Rows {
    fn len(&self) -> usize {
        self.0
    }

    fn item_key(&self, index: usize) -> ItemKey {
        ItemKey::numeric(index as u64)
    }

    fn build_item(
        &self,
        index: usize,
        _key: &ItemKey,
        _context: &mut BuildContext,
    ) -> Result<WidgetNode> {
        Ok(WidgetNode::new::<Row>().with_key(index as u64))
    }
}

#[derive(Clone)]
struct Sample {
    duration: Duration,
    metrics: FrameMetrics,
}

fn virtual_list_anchor() -> Result<Sample> {
    let started = Instant::now();
    let mut list = VirtualList::new(Rows(100_000), 20.0)?;
    list.set_viewport(320.0, 500_000.0)?;
    let before = list.scroll_offset();
    let update = list.report_item_height(&ItemKey::numeric(10), 40.0)?;
    assert_eq!(update.scroll_adjustment, 20.0);
    assert_eq!(list.scroll_offset(), before + 20.0);
    assert!(list.materialized_count() <= 40);
    let mut metrics = FrameMetrics::default();
    metrics.phases.layout = started.elapsed();
    metrics.virtualization = list.metrics().into();
    Ok(Sample {
        duration: started.elapsed(),
        metrics,
    })
}

fn animation_tick() -> Result<Sample> {
    let clock = Rc::new(FakeClock::new());
    let mut timeline = Timeline::new(clock.clone());
    let value = Animated::new(0.0_f32);
    for index in 0..10_000_u32 {
        timeline.animate(
            AnimationKey::new(ElementId::from_parts(index, 1), PropertyId::new(1)),
            &value,
            1.0,
            AnimationSpec::new(Duration::from_millis(100), AnimationImpact::Paint),
        );
    }
    clock.advance(Duration::from_millis(50))?;
    let started = Instant::now();
    let frame = timeline.tick();
    let elapsed = started.elapsed();
    assert_eq!(frame.sampled(), 10_000);
    let mut metrics = FrameMetrics::default();
    metrics.phases.update = elapsed;
    metrics.animation.active = frame.active() as u64;
    metrics.animation.sampled = frame.sampled() as u64;
    metrics.animation.completed = frame.completed().len() as u64;
    metrics.animation.cancelled = frame.cancelled().len() as u64;
    metrics.animation.tick_time = elapsed;
    Ok(Sample {
        duration: elapsed,
        metrics,
    })
}

#[cfg(feature = "text")]
fn text_replacement_and_dpi() -> Result<Sample> {
    use tgui::text::{TextRequest, TextStyle, TextSystem};

    let mut text = TextSystem::with_cache_capacity(8);
    let started = Instant::now();
    text.layout(&TextRequest::new("first 中文", TextStyle::new(16.0)))?;
    text.layout(
        &TextRequest::new("replacement مرحبا", TextStyle::new(16.0))
            .with_content_generation(1)
            .with_dpi(DpiScale::new(2.0)?),
    )?;
    let elapsed = started.elapsed();
    let stats = text.cache_stats();
    let mut metrics = FrameMetrics::default();
    metrics.phases.layout = elapsed;
    metrics.resources.hits = stats.hits;
    metrics.resources.misses = stats.misses;
    metrics.resources.evictions = stats.evictions;
    Ok(Sample {
        duration: elapsed,
        metrics,
    })
}

#[cfg(not(feature = "text"))]
fn text_replacement_and_dpi() -> Result<Sample> {
    let mut context = BuildContext::new();
    let first = Container::new()
        .with_child(Text::new("first 中文").build(&mut context)?)
        .build(&mut context)?;
    let mut harness = RenderHarness::new();
    harness.mount(first)?;
    harness.render(Size::new(320.0, 200.0), DpiScale::ONE)?;
    let replacement = Container::new()
        .with_child(Text::new("replacement مرحبا").build(&mut context)?)
        .build(&mut context)?;
    let started = Instant::now();
    harness.reconcile(replacement)?;
    let frame = harness.render(Size::new(320.0, 200.0), DpiScale::new(2.0)?)?;
    let elapsed = started.elapsed();
    let mut metrics = FrameMetrics::default();
    metrics.phases.layout = elapsed;
    metrics.scene.paint_commands = frame.scene.command_count() as u64;
    metrics.scene.render_chunks = frame.scene.chunk_count() as u64;
    metrics.scene.batches = frame.compiled.batches as u64;
    metrics.scene.passes = frame.compiled.passes as u64;
    Ok(Sample {
        duration: elapsed,
        metrics,
    })
}

fn image_replacement() -> Result<Sample> {
    let started = Instant::now();
    let mut images = ImageRegistry::new();
    let first_key = ImageRequestKey::new(ImageSource::bytes([1_u8].as_slice()));
    let first = images.request(first_key.clone());
    let image = DecodedImage::new(ImageSize::new(4, 4)?, vec![255_u8; 64])?;
    assert!(matches!(
        images.complete(
            first.handle.stamp(),
            &ImageDecodeResult {
                handle: first.handle,
                key: first_key,
                decoded: Ok(image.clone()),
            },
        ),
        ImageCompletion::Ready { .. }
    ));
    let second_key = ImageRequestKey::new(ImageSource::bytes([2_u8].as_slice()));
    let second = images
        .replace(first.handle, second_key.clone())
        .expect("live image can be replaced");
    assert!(matches!(
        images.complete(
            second.handle.stamp(),
            &ImageDecodeResult {
                handle: second.handle,
                key: second_key,
                decoded: Ok(image),
            },
        ),
        ImageCompletion::Ready { .. }
    ));
    let elapsed = started.elapsed();
    let mut metrics = FrameMetrics::default();
    metrics.phases.update = elapsed;
    metrics.resources.misses = 2;
    metrics.resources.upload_bytes = 128;
    Ok(Sample {
        duration: elapsed,
        metrics,
    })
}

fn atlas_key() -> GlyphAtlasKey {
    GlyphAtlasKey::new(
        FontHandle::from_parts(1, 1),
        PhysicalFontSize::from_pixels(16.0).expect("valid size"),
        GlyphVariant::new(0),
        GlyphContentType::Mask,
    )
}

fn glyph_eviction_and_rasterization() -> Result<Sample> {
    let started = Instant::now();
    let mut atlas = GlyphAtlas::new(
        GlyphAtlasConfig::new(8, 8, 1)
            .with_padding(0)
            .with_max_bytes(64),
    )?;
    let run = ResourceId::from_parts(1, 1);
    let first = GlyphKey::new(atlas_key(), 1);
    let second = GlyphKey::new(atlas_key(), 2);
    let GlyphLookup::Rasterize(first_request) = atlas.lookup(first, run)? else {
        unreachable!()
    };
    assert!(matches!(
        atlas.complete_raster(first_request, GlyphRaster::new(5, 5, vec![1; 25]))?,
        GlyphCompletionOutcome::Ready(_)
    ));
    let GlyphLookup::Rasterize(second_request) = atlas.lookup(second, run)? else {
        unreachable!()
    };
    assert!(matches!(
        atlas.complete_raster(second_request, GlyphRaster::new(5, 5, vec![2; 25]))?,
        GlyphCompletionOutcome::Ready(_)
    ));
    assert!(matches!(
        atlas.lookup(first, run)?,
        GlyphLookup::Rasterize(_)
    ));
    let elapsed = started.elapsed();
    let stats = atlas.stats();
    assert_eq!(stats.page_evictions, 1);
    let mut metrics = FrameMetrics::default();
    metrics.phases.update = elapsed;
    metrics.resources.hits = stats.hits;
    metrics.resources.misses = stats.misses;
    metrics.resources.evictions = stats.glyph_evictions;
    Ok(Sample {
        duration: elapsed,
        metrics,
    })
}

fn layer_backdrop_native_stress() -> Result<Sample> {
    let bounds = Rect::from_xywh(0.0, 0.0, 256.0, 256.0);
    let mut commands = Vec::new();
    for index in 0..32_u32 {
        commands.extend([
            PaintCommand::BeginLayer(
                LayerSpec::new(bounds).with_backdrop(BackdropFilter::Blur { radius: 8.0 }),
            ),
            PaintCommand::FillRect {
                rect: bounds,
                color: Color::rgba8(20, 40, 80, 192),
            },
            PaintCommand::EndLayer,
            PaintCommand::NativeSurface {
                rect: Rect::from_xywh(index as f32, index as f32, 64.0, 64.0),
                surface: ResourceId::from_parts(index, 1),
                opaque: true,
            },
        ]);
    }
    let capabilities = RendererCapabilities {
        supports_native_surface: true,
        supports_backdrop: true,
        ..RendererCapabilities::default()
    };
    let context = CompileContext::new(capabilities, DpiScale::ONE)
        .with_scene_revision(SceneRevision::new(1))
        .with_transient_budget(64 * 1024 * 1024);
    let started = Instant::now();
    let compiled = RenderCompiler::default().compile(&commands, &context)?;
    let elapsed = started.elapsed();
    assert!(compiled.pass_count() > 1);
    let mut metrics = FrameMetrics::default();
    metrics.phases.compile = elapsed;
    metrics.scene.paint_commands = commands.len() as u64;
    metrics.scene.batches = compiled.batch_count() as u64;
    metrics.scene.passes = compiled.pass_count() as u64;
    metrics.scene.gpu_upload_bytes = compiled.upload_bytes();
    metrics.scene.transient_vram_bytes = compiled.offscreen_cost.transient_vram_bytes;
    Ok(Sample {
        duration: elapsed,
        metrics,
    })
}

fn percentile(samples: &[Sample], rank: usize) -> u64 {
    let mut values = samples
        .iter()
        .map(|sample| u64::try_from(sample.duration.as_nanos()).unwrap_or(u64::MAX))
        .collect::<Vec<_>>();
    values.sort_unstable();
    let index = rank
        .saturating_mul(values.len().saturating_sub(1))
        .saturating_add(99)
        / 100;
    values[index.min(values.len() - 1)]
}

fn emit(name: &str, samples: &[Sample]) {
    let metrics = &samples[samples.len() / 2].metrics;
    println!(
        "{name},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
        samples.len(),
        percentile(samples, 50),
        percentile(samples, 95),
        percentile(samples, 99),
        metrics.phases.update.as_nanos(),
        metrics.phases.layout.as_nanos(),
        metrics.phases.paint.as_nanos(),
        metrics.phases.compile.as_nanos(),
        metrics.virtualization.total_items,
        metrics.virtualization.materialized_items,
        metrics.virtualization.peak_materialized_items,
        metrics.animation.active,
        metrics.animation.sampled,
        metrics.scene.paint_commands,
        metrics.scene.render_chunks,
        metrics.scene.batches,
        metrics.scene.passes,
        metrics.scene.gpu_upload_bytes,
        metrics.scene.transient_vram_bytes,
        metrics.resources.hits,
        metrics.resources.misses,
        metrics.resources.evictions,
    );
}

fn sample_count() -> usize {
    env::var("TGUI_BENCH_SAMPLES")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|count| *count > 0)
        .unwrap_or(DEFAULT_SAMPLES)
}

fn main() -> Result<()> {
    type Scenario = (&'static str, fn() -> Result<Sample>);
    let scenarios: [Scenario; 6] = [
        ("virtual_list_variable_anchor", virtual_list_anchor),
        ("animation_tick_10000", animation_tick),
        ("text_replacement_font_dpi", text_replacement_and_dpi),
        ("image_replacement", image_replacement),
        (
            "glyph_eviction_rerasterization",
            glyph_eviction_and_rasterization,
        ),
        (
            "layer_backdrop_native_surface",
            layer_backdrop_native_stress,
        ),
    ];
    let samples = sample_count();
    println!("# schema=tgui-p7-stress-v1 samples={samples}");
    println!(
        "scenario,samples,total_p50_ns,total_p95_ns,total_p99_ns,update_ns,layout_ns,paint_ns,compile_ns,virtual_total,virtual_materialized,virtual_peak,animations_active,animations_sampled,paint_commands,render_chunks,batches,passes,gpu_upload_bytes,transient_vram_bytes,resource_hits,resource_misses,resource_evictions"
    );
    for (name, scenario) in scenarios {
        let mut results = Vec::with_capacity(samples);
        for _ in 0..samples {
            results.push(scenario()?);
        }
        black_box(&results);
        emit(name, &results);
    }
    Ok(())
}
