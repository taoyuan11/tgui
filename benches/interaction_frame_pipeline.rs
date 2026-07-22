#[cfg(feature = "bench-support")]
use std::hint::black_box;
#[cfg(feature = "bench-support")]
use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
#[cfg(feature = "bench-support")]
use tgui::core::Rect;
#[cfg(feature = "bench-support")]
use tgui::mvvm::CommandEffect;
#[cfg(feature = "bench-support")]
use tgui::widgets::{
    RuntimeButtonHoverBenchmarkContext, RuntimeDataGridBenchmarkContext,
    RuntimeDataGridHoverTarget, RuntimeFocusBenchmarkContext, RuntimeFocusNavigationCacheStats,
    RuntimeInteractionFrameStats, RuntimeRowHoverBenchmarkContext, RuntimeRowHoverKind,
    RuntimeRowSelectionBenchmarkContext, RuntimeRowSelectionKind, RuntimeRowSelectionMode,
    RuntimeSliderValueBenchmarkContext, RuntimeTextContentBenchmarkContext,
    RuntimeToastBenchmarkContext, RuntimeTreeCheckedBenchmarkContext,
};

#[cfg(feature = "bench-support")]
const ENTER_MIDPOINT: Duration = Duration::from_millis(135);
#[cfg(feature = "bench-support")]
const EXIT_MIDPOINT: Duration = Duration::from_millis(74);

#[cfg(feature = "bench-support")]
fn viewport() -> Rect {
    Rect::new(0.0, 0.0, 960.0, 720.0)
}

#[cfg(feature = "bench-support")]
fn assert_retained_scene_update(label: &str, stats: RuntimeInteractionFrameStats) {
    assert!(stats.rendered, "{label}: interaction must render one frame");
    assert!(
        stats.is_retained_scene_update(),
        "{label}: expected a retained scene update with no layout refresh, got {stats:?}"
    );
    assert_eq!(stats.scene_recollects + stats.retained_scene_patches, 1);
    if stats.scene_recollects > 0 {
        assert_eq!(
            stats.layout_reuses, 1,
            "{label}: expected exactly one root-layout reuse"
        );
    }
    assert_eq!(stats.layout_builds, 0, "{label}: rebuilt the root layout");
}

#[cfg(feature = "bench-support")]
fn assert_pixel_identical(label: &str, retained: &[u8], full: &[u8]) {
    if retained == full {
        return;
    }
    assert_eq!(
        retained.len(),
        full.len(),
        "{label}: retained and full-layout frame byte lengths differ"
    );
    assert_eq!(
        retained.len() % 4,
        0,
        "{label}: RGBA readback length is not pixel-aligned"
    );

    let width = viewport().width.get().round() as usize;
    let mut first_difference = None;
    let mut min_x = usize::MAX;
    let mut min_y = usize::MAX;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut mismatched_pixels = 0;
    let mut mismatched_bytes = 0;
    for (pixel_index, (retained_rgba, full_rgba)) in retained
        .chunks_exact(4)
        .zip(full.chunks_exact(4))
        .enumerate()
    {
        if retained_rgba == full_rgba {
            continue;
        }

        let x = pixel_index % width;
        let y = pixel_index / width;
        first_difference.get_or_insert_with(|| {
            (
                x,
                y,
                <[u8; 4]>::try_from(retained_rgba).expect("RGBA pixel"),
                <[u8; 4]>::try_from(full_rgba).expect("RGBA pixel"),
            )
        });
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
        mismatched_pixels += 1;
        mismatched_bytes += retained_rgba
            .iter()
            .zip(full_rgba)
            .filter(|(retained, full)| retained != full)
            .count();
    }

    let (first_x, first_y, retained_rgba, full_rgba) =
        first_difference.expect("different RGBA buffers contain a differing pixel");
    panic!(
        "{label}: retained and full-layout frames differ; first_pixel=({first_x},{first_y}) retained_rgba={retained_rgba:?} full_rgba={full_rgba:?} bbox=({min_x},{min_y})..=({max_x},{max_y}) mismatched_pixels={mismatched_pixels} mismatched_bytes={mismatched_bytes}"
    );
}

#[cfg(feature = "bench-support")]
fn assert_pixels_changed(label: &str, before: &[u8], after: &[u8]) {
    assert_eq!(
        before.len(),
        after.len(),
        "{label}: output dimensions changed"
    );
    if before == after {
        panic!("{label}: selection state changed without changing the rendered output");
    }
}

#[cfg(feature = "bench-support")]
fn percentile(samples: &[Duration], percentile: f64) -> Duration {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let index = ((sorted.len().saturating_sub(1)) as f64 * percentile)
        .round()
        .clamp(0.0, sorted.len().saturating_sub(1) as f64) as usize;
    sorted[index]
}

#[cfg(feature = "bench-support")]
fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

#[cfg(feature = "bench-support")]
fn print_budget(label: &str, samples: &[Duration]) {
    let p50 = percentile(samples, 0.50);
    let p95 = percentile(samples, 0.95);
    let max = samples.iter().copied().max().unwrap_or_default();
    eprintln!(
        "interaction_frame_budget: path={label} samples={} p50_ms={:.4} p95_ms={:.4} max_ms={:.4} p95_120hz_pct={:.1}% p95_144hz_pct={:.1}%",
        samples.len(),
        milliseconds(p50),
        milliseconds(p95),
        milliseconds(max),
        milliseconds(p95) / (1_000.0 / 120.0) * 100.0,
        milliseconds(p95) / (1_000.0 / 144.0) * 100.0,
    );
}

#[cfg(feature = "bench-support")]
fn profile_data_grid(context: &mut RuntimeDataGridBenchmarkContext) {
    let first = context
        .move_hover(RuntimeDataGridHoverTarget::FirstRowStartPinned)
        .expect("first DataGrid hover frame");
    assert_retained_scene_update("DataGrid first-row enter", first);
    let retained_pixels = context
        .read_output_rgba()
        .expect("DataGrid retained output readback");
    let full = context
        .render_full_layout_control()
        .expect("DataGrid full-layout control");
    assert_eq!(full.layout_builds, 1);
    assert_eq!(full.layout_reuses, 0);
    let full_pixels = context
        .read_output_rgba()
        .expect("DataGrid full-layout output readback");
    assert_pixel_identical(
        "DataGrid row-hover fallback equivalence",
        &retained_pixels,
        &full_pixels,
    );

    for target in [
        RuntimeDataGridHoverTarget::FirstRowUnpinned,
        RuntimeDataGridHoverTarget::FirstRowEndPinned,
        RuntimeDataGridHoverTarget::FirstRowStartPinned,
    ] {
        let cross = context
            .move_hover(target)
            .expect("same-row DataGrid boundary crossing");
        assert!(!cross.event_changed, "same-row hover target changed");
        assert!(
            !cross.rendered,
            "same-row boundary crossing rendered a frame"
        );
        assert_eq!(cross.hover_epoch_delta, 0);
        assert_eq!(cross.invalidation_revision_delta, 0);
        assert_eq!(cross.scene_recollects, 0);
        assert_eq!(cross.layout_builds, 0);
    }

    let mut samples = Vec::with_capacity(180);
    for index in 0..180 {
        let target = if index % 2 == 0 {
            RuntimeDataGridHoverTarget::SecondRowStartPinned
        } else {
            RuntimeDataGridHoverTarget::FirstRowStartPinned
        };
        let stats = context
            .move_hover(target)
            .expect("DataGrid row-transition profile frame");
        assert_retained_scene_update("DataGrid row transition", stats);
        assert_eq!(stats.hover_epoch_delta, 1);
        samples.push(stats.total);
    }
    print_budget("data_grid/10000_rows/row_transition", &samples);
    eprintln!(
        "interaction_frame_path: widget=data_grid adapter='{}' backend={} rows=10000 cross_pinned_rendered=false row_transition={first:?} full_control={full:?}",
        context.adapter_name, context.backend,
    );
}

#[cfg(feature = "bench-support")]
fn profile_data_grid_ab(
    optimized: &mut RuntimeDataGridBenchmarkContext,
    control: &mut RuntimeDataGridBenchmarkContext,
) {
    let _ = optimized
        .move_hover(RuntimeDataGridHoverTarget::FirstRowStartPinned)
        .expect("optimized DataGrid A/B baseline");
    let _ = control
        .move_hover_scene_recollect_control(RuntimeDataGridHoverTarget::FirstRowStartPinned)
        .expect("control DataGrid A/B baseline");
    let mut optimized_samples = Vec::with_capacity(120);
    let mut control_samples = Vec::with_capacity(120);
    for index in 0..120 {
        let target = if index % 2 == 0 {
            RuntimeDataGridHoverTarget::SecondRowStartPinned
        } else {
            RuntimeDataGridHoverTarget::FirstRowStartPinned
        };
        let optimized_stats = optimized
            .move_hover(target)
            .expect("optimized DataGrid A/B frame");
        assert_eq!(optimized_stats.retained_scene_patches, 1);
        assert_eq!(optimized_stats.scene_recollects, 0);
        let control_stats = control
            .move_hover_scene_recollect_control(target)
            .expect("control DataGrid A/B frame");
        assert!(control_stats.is_scene_only_recollect());
        assert_eq!(control_stats.retained_scene_patches, 0);
        optimized_samples.push(optimized_stats.total);
        control_samples.push(control_stats.total);
    }
    let optimized_pixels = optimized
        .read_output_rgba()
        .expect("optimized DataGrid A/B output readback");
    let control_pixels = control
        .read_output_rgba()
        .expect("control DataGrid A/B output readback");
    assert_pixel_identical(
        "DataGrid two-row patch/scene-recollect equivalence",
        &optimized_pixels,
        &control_pixels,
    );
    print_budget("data_grid/10000_rows/two_row_patch_ab", &optimized_samples);
    print_budget(
        "data_grid/10000_rows/scene_recollect_control_ab",
        &control_samples,
    );
    eprintln!(
        "interaction_data_grid_ab: optimized_p50={:?} optimized_p95={:?} control_p50={:?} control_p95={:?}",
        percentile(&optimized_samples, 0.50),
        percentile(&optimized_samples, 0.95),
        percentile(&control_samples, 0.50),
        percentile(&control_samples, 0.95),
    );
}

#[cfg(feature = "bench-support")]
fn profile_row_hover_ab(
    optimized: &mut RuntimeRowHoverBenchmarkContext,
    control: &mut RuntimeRowHoverBenchmarkContext,
) {
    let optimized_first = optimized
        .move_hover(false)
        .expect("optimized row-hover first-row frame");
    assert!(optimized_first.is_scene_only_recollect());
    let control_first = control
        .move_hover_scene_recollect_control(false)
        .expect("control row-hover first-row frame");
    assert!(control_first.is_scene_only_recollect());
    let mut optimized_samples = Vec::with_capacity(180);
    let mut control_samples = Vec::with_capacity(180);
    for index in 0..180 {
        let optimized_stats = optimized
            .move_hover(index % 2 == 0)
            .expect("optimized row-hover transition frame");
        assert_eq!(optimized_stats.retained_scene_patches, 1);
        let control_stats = control
            .move_hover_scene_recollect_control(index % 2 == 0)
            .expect("control row-hover transition frame");
        assert!(control_stats.is_scene_only_recollect());
        optimized_samples.push(optimized_stats.total);
        control_samples.push(control_stats.total);
    }
    print_budget(
        &format!("{:?}/10000_rows/two_row_patch", optimized.kind).to_lowercase(),
        &optimized_samples,
    );
    print_budget(
        &format!("{:?}/10000_rows/scene_recollect_control", control.kind).to_lowercase(),
        &control_samples,
    );
    eprintln!(
        "interaction_row_hover_ab: kind={:?} adapter='{}' backend={} optimized_p50={:?} optimized_p95={:?} control_p50={:?} control_p95={:?}",
        optimized.kind,
        optimized.adapter_name,
        optimized.backend,
        percentile(&optimized_samples, 0.50),
        percentile(&optimized_samples, 0.95),
        percentile(&control_samples, 0.50),
        percentile(&control_samples, 0.95),
    );
}

#[cfg(feature = "bench-support")]
fn verify_row_hover_equivalence(kind: RuntimeRowHoverKind) {
    let mut optimized =
        RuntimeRowHoverBenchmarkContext::new_with_reduced_motion(kind, 10_000, viewport(), true)
            .expect("reduced-motion optimized row-hover context");
    let mut control =
        RuntimeRowHoverBenchmarkContext::new_with_reduced_motion(kind, 10_000, viewport(), true)
            .expect("reduced-motion control row-hover context");
    let _ = optimized
        .move_hover(false)
        .expect("optimized equivalence baseline");
    let _ = control
        .move_hover_scene_recollect_control(false)
        .expect("control equivalence baseline");
    let optimized_stats = optimized
        .move_hover(true)
        .expect("optimized equivalence transition");
    assert_eq!(optimized_stats.retained_scene_patches, 1);
    let control_stats = control
        .move_hover_scene_recollect_control(true)
        .expect("control equivalence transition");
    assert!(control_stats.is_scene_only_recollect());
    let optimized_pixels = optimized
        .read_output_rgba()
        .expect("optimized row-hover output readback");
    let control_pixels = control
        .read_output_rgba()
        .expect("control row-hover output readback");
    assert_pixel_identical(
        "List/Tree two-row patch/scene-recollect equivalence",
        &optimized_pixels,
        &control_pixels,
    );
}

#[cfg(feature = "bench-support")]
fn assert_button_hover_patch(label: &str, stats: RuntimeInteractionFrameStats) {
    assert!(stats.event_changed, "{label}: hover target did not change");
    assert!(stats.rendered, "{label}: hover frame was not rendered");
    assert_eq!(stats.hover_epoch_delta, 1, "{label}: hover epoch delta");
    assert_eq!(
        stats.invalidation_revision_delta, 0,
        "{label}: hover unexpectedly dirtied reactive state"
    );
    assert_eq!(
        stats.retained_scene_patches, 1,
        "{label}: patch count; stats={stats:?}"
    );
    assert_eq!(stats.scene_recollects, 0, "{label}: scene recollect count");
    assert_eq!(stats.layout_builds, 0, "{label}: layout build count");
    assert_eq!(stats.layout_patch_actions, 0, "{label}: layout patch count");
    assert_eq!(stats.full_rebuild_actions, 0, "{label}: full rebuild count");
}

#[cfg(feature = "bench-support")]
fn verify_button_hover_equivalence(buttons: usize) {
    let mut optimized =
        RuntimeButtonHoverBenchmarkContext::new_with_reduced_motion(buttons, viewport(), true)
            .expect("reduced-motion optimized button-hover context");
    let mut control =
        RuntimeButtonHoverBenchmarkContext::new_with_reduced_motion(buttons, viewport(), true)
            .expect("reduced-motion control button-hover context");
    let optimized_enter = optimized
        .move_hover(false)
        .expect("optimized button-hover equivalence initial enter");
    assert_retained_scene_update("button hover equivalence initial enter", optimized_enter);
    let control_enter = control
        .move_hover_scene_recollect_control(false)
        .expect("control button-hover equivalence initial enter");
    assert!(control_enter.is_scene_only_recollect());
    assert_eq!(control_enter.retained_scene_patches, 0);

    let optimized_move = optimized
        .move_hover(true)
        .expect("optimized button-hover equivalence transition");
    assert_button_hover_patch("button hover equivalence transition", optimized_move);
    let control_move = control
        .move_hover_scene_recollect_control(true)
        .expect("control button-hover equivalence transition");
    assert!(control_move.is_scene_only_recollect());
    assert_eq!(control_move.retained_scene_patches, 0);
    assert_eq!(control_move.layout_builds, 0);
    assert_eq!(control_move.invalidation_revision_delta, 0);

    let optimized_pixels = optimized
        .read_output_rgba()
        .expect("optimized button-hover output readback");
    let control_pixels = control
        .read_output_rgba()
        .expect("control button-hover output readback");
    assert_pixel_identical(
        "Button two-root hover patch/scene-recollect equivalence",
        &optimized_pixels,
        &control_pixels,
    );
}

#[cfg(feature = "bench-support")]
fn profile_button_hover_ab(buttons: usize) {
    const WARMUP_FRAMES: usize = 12;
    const SAMPLES: usize = 80;

    let mut optimized = RuntimeButtonHoverBenchmarkContext::new(buttons, viewport())
        .expect("optimized production button-hover context");
    let mut control = RuntimeButtonHoverBenchmarkContext::new(buttons, viewport())
        .expect("scene-recollect control button-hover context");
    eprintln!(
        "interaction_button_hover_adapter: buttons={buttons} adapter='{}' backend={}",
        optimized.adapter_name, optimized.backend,
    );

    let optimized_enter = optimized
        .move_hover(false)
        .expect("optimized button-hover initial enter");
    assert_retained_scene_update("button hover initial enter", optimized_enter);
    let control_enter = control
        .move_hover_scene_recollect_control(false)
        .expect("control button-hover initial enter");
    assert!(control_enter.is_scene_only_recollect());
    assert_eq!(control_enter.retained_scene_patches, 0);

    for index in 0..WARMUP_FRAMES {
        let second = index % 2 == 0;
        let optimized_stats = optimized
            .move_hover(second)
            .expect("optimized button-hover warmup");
        assert_button_hover_patch("button hover warmup", optimized_stats);
        let control_stats = control
            .move_hover_scene_recollect_control(second)
            .expect("control button-hover warmup");
        assert!(control_stats.is_scene_only_recollect());
        assert_eq!(control_stats.retained_scene_patches, 0);
        assert_eq!(control_stats.layout_builds, 0);
    }

    let mut optimized_samples = Vec::with_capacity(SAMPLES);
    let mut control_samples = Vec::with_capacity(SAMPLES);
    for index in 0..SAMPLES {
        let second = index % 2 == 0;
        let optimized_stats = optimized
            .move_hover(second)
            .expect("optimized button-hover profile frame");
        assert_button_hover_patch("button hover profile", optimized_stats);
        let control_stats = control
            .move_hover_scene_recollect_control(second)
            .expect("control button-hover profile frame");
        assert!(control_stats.is_scene_only_recollect());
        assert_eq!(control_stats.retained_scene_patches, 0);
        assert_eq!(control_stats.layout_builds, 0);
        assert_eq!(control_stats.invalidation_revision_delta, 0);
        optimized_samples.push(optimized_stats.total);
        control_samples.push(control_stats.total);
    }

    print_budget(
        &format!("button/{buttons}_buttons/two_root_hover_patch"),
        &optimized_samples,
    );
    print_budget(
        &format!("button/{buttons}_buttons/scene_recollect_control"),
        &control_samples,
    );
    let optimized_p50 = percentile(&optimized_samples, 0.50);
    let control_p50 = percentile(&control_samples, 0.50);
    let reduction = if control_p50.is_zero() {
        0.0
    } else {
        (1.0 - optimized_p50.as_secs_f64() / control_p50.as_secs_f64()) * 100.0
    };
    eprintln!(
        "interaction_button_hover_ab: buttons={buttons} optimized_p50_ms={:.4} optimized_p95_ms={:.4} control_p50_ms={:.4} control_p95_ms={:.4} reduction_pct={reduction:.2}",
        milliseconds(optimized_p50),
        milliseconds(percentile(&optimized_samples, 0.95)),
        milliseconds(control_p50),
        milliseconds(percentile(&control_samples, 0.95)),
    );
}

#[cfg(feature = "bench-support")]
fn assert_button_pressed_patch(label: &str, stats: RuntimeInteractionFrameStats) {
    assert!(stats.event_changed, "{label}: pressed state did not change");
    assert!(stats.rendered, "{label}: pressed frame was not rendered");
    assert_eq!(stats.command_dispatches_delta, 0, "{label}: command delta");
    assert_eq!(stats.hover_epoch_delta, 0, "{label}: hover epoch delta");
    assert_eq!(
        stats.invalidation_revision_delta, 0,
        "{label}: reactive revision delta"
    );
    assert_eq!(
        stats.retained_scene_patches, 1,
        "{label}: patch count; stats={stats:?}"
    );
    assert_eq!(stats.scene_recollects, 0, "{label}: scene recollect count");
    assert_eq!(stats.layout_builds, 0, "{label}: layout build count");
    assert_eq!(stats.layout_patch_actions, 0, "{label}: layout patch count");
    assert_eq!(stats.full_rebuild_actions, 0, "{label}: full rebuild count");
}

#[cfg(feature = "bench-support")]
fn verify_button_pressed_equivalence(buttons: usize) {
    let mut optimized =
        RuntimeButtonHoverBenchmarkContext::new_with_reduced_motion(buttons, viewport(), true)
            .expect("reduced-motion optimized Button pressed context");
    let mut control =
        RuntimeButtonHoverBenchmarkContext::new_with_reduced_motion(buttons, viewport(), true)
            .expect("reduced-motion control Button pressed context");
    optimized
        .prime_focus(false)
        .expect("optimized Button pressed focus prime");
    control
        .prime_focus(false)
        .expect("control Button pressed focus prime");
    let released_baseline = optimized
        .read_output_rgba()
        .expect("released Button baseline readback");

    let optimized_down = optimized
        .pointer_down(false)
        .expect("optimized Button pressed equivalence down");
    assert_button_pressed_patch("Button pressed equivalence down", optimized_down);
    let control_down = control
        .pointer_down_scene_recollect_control(false)
        .expect("control Button pressed equivalence down");
    assert!(control_down.is_scene_only_recollect());
    assert_eq!(control_down.retained_scene_patches, 0);
    assert_eq!(control_down.invalidation_revision_delta, 0);
    let optimized_pressed = optimized
        .read_output_rgba()
        .expect("optimized pressed Button readback");
    let control_pressed = control
        .read_output_rgba()
        .expect("control pressed Button readback");
    assert_pixel_identical(
        "Button pressed patch/scene-recollect equivalence",
        &optimized_pressed,
        &control_pressed,
    );
    assert_ne!(
        optimized_pressed, released_baseline,
        "pressed Button pixels must differ from the released hover state"
    );

    let optimized_up = optimized
        .pointer_up(false)
        .expect("optimized Button pressed equivalence up");
    assert_button_pressed_patch("Button released equivalence up", optimized_up);
    let control_up = control
        .pointer_up_scene_recollect_control(false)
        .expect("control Button pressed equivalence up");
    assert!(control_up.is_scene_only_recollect());
    assert_eq!(control_up.retained_scene_patches, 0);
    let optimized_released = optimized
        .read_output_rgba()
        .expect("optimized released Button readback");
    let control_released = control
        .read_output_rgba()
        .expect("control released Button readback");
    assert_pixel_identical(
        "Button released patch/scene-recollect equivalence",
        &optimized_released,
        &control_released,
    );
    assert_pixel_identical(
        "Button release restores the pre-press hover pixels",
        &optimized_released,
        &released_baseline,
    );
}

#[cfg(feature = "bench-support")]
fn profile_button_pressed_ab(buttons: usize) {
    const WARMUP_CYCLES: usize = 12;
    const SAMPLES: usize = 80;

    let mut optimized = RuntimeButtonHoverBenchmarkContext::new(buttons, viewport())
        .expect("optimized production Button pressed context");
    let mut control = RuntimeButtonHoverBenchmarkContext::new(buttons, viewport())
        .expect("scene-recollect control Button pressed context");
    optimized
        .prime_focus(false)
        .expect("optimized Button pressed focus prime");
    control
        .prime_focus(false)
        .expect("control Button pressed focus prime");

    for _ in 0..WARMUP_CYCLES {
        assert_button_pressed_patch(
            "Button pressed warmup down",
            optimized
                .pointer_down(false)
                .expect("optimized warmup down"),
        );
        let control_down = control
            .pointer_down_scene_recollect_control(false)
            .expect("control warmup down");
        assert!(control_down.is_scene_only_recollect());
        assert_button_pressed_patch(
            "Button pressed warmup up",
            optimized.pointer_up(false).expect("optimized warmup up"),
        );
        let control_up = control
            .pointer_up_scene_recollect_control(false)
            .expect("control warmup up");
        assert!(control_up.is_scene_only_recollect());
    }

    let mut optimized_down_samples = Vec::with_capacity(SAMPLES);
    let mut control_down_samples = Vec::with_capacity(SAMPLES);
    let mut optimized_up_samples = Vec::with_capacity(SAMPLES);
    let mut control_up_samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let optimized_down = optimized
            .pointer_down(false)
            .expect("optimized Button pressed profile down");
        assert_button_pressed_patch("Button pressed profile down", optimized_down);
        let control_down = control
            .pointer_down_scene_recollect_control(false)
            .expect("control Button pressed profile down");
        assert!(control_down.is_scene_only_recollect());
        assert_eq!(control_down.retained_scene_patches, 0);
        optimized_down_samples.push(optimized_down.total);
        control_down_samples.push(control_down.total);

        let optimized_up = optimized
            .pointer_up(false)
            .expect("optimized Button pressed profile up");
        assert_button_pressed_patch("Button pressed profile up", optimized_up);
        let control_up = control
            .pointer_up_scene_recollect_control(false)
            .expect("control Button pressed profile up");
        assert!(control_up.is_scene_only_recollect());
        assert_eq!(control_up.retained_scene_patches, 0);
        optimized_up_samples.push(optimized_up.total);
        control_up_samples.push(control_up.total);
    }

    for (phase, optimized_samples, control_samples) in [
        ("down", &optimized_down_samples, &control_down_samples),
        ("up", &optimized_up_samples, &control_up_samples),
    ] {
        print_budget(
            &format!("button/{buttons}_buttons/{phase}_pressed_patch"),
            optimized_samples,
        );
        print_budget(
            &format!("button/{buttons}_buttons/{phase}_scene_recollect_control"),
            control_samples,
        );
        let optimized_p50 = percentile(optimized_samples, 0.50);
        let control_p50 = percentile(control_samples, 0.50);
        let reduction = if control_p50.is_zero() {
            0.0
        } else {
            (1.0 - optimized_p50.as_secs_f64() / control_p50.as_secs_f64()) * 100.0
        };
        eprintln!(
            "interaction_button_pressed_ab: phase={phase} buttons={buttons} optimized_p50_ms={:.4} optimized_p95_ms={:.4} control_p50_ms={:.4} control_p95_ms={:.4} reduction_pct={reduction:.2}",
            milliseconds(optimized_p50),
            milliseconds(percentile(optimized_samples, 0.95)),
            milliseconds(control_p50),
            milliseconds(percentile(control_samples, 0.95)),
        );
    }
}

#[cfg(feature = "bench-support")]
fn profile_toast_stage(
    context: &mut RuntimeToastBenchmarkContext,
    count: usize,
    stage: &str,
    samples: usize,
    legacy_double_layout: bool,
) -> Vec<Duration> {
    let mut durations = Vec::with_capacity(samples);
    let mut sync_bindings = Duration::ZERO;
    let mut scene_update = Duration::ZERO;
    let mut measure = Duration::ZERO;
    let mut collect = Duration::ZERO;
    let mut compose = Duration::ZERO;
    let mut liveness = Duration::ZERO;
    let mut prepare_upload = Duration::ZERO;
    let mut encode = Duration::ZERO;
    let mut submit = Duration::ZERO;
    let mut gpu_wait = Duration::ZERO;
    let mut measured_cards = 0_usize;
    let mut collected_cards = 0_usize;
    let mut layout_passes = 0_usize;
    for _ in 0..samples {
        let stats = match stage {
            "insert" => {
                context.prepare_empty().expect("empty Toast baseline");
                if legacy_double_layout {
                    context
                        .render_insert_frame_legacy_double_layout(count, ENTER_MIDPOINT)
                        .expect("legacy Toast insert frame")
                } else {
                    context
                        .render_insert_frame(count, ENTER_MIDPOINT)
                        .expect("Toast insert frame")
                }
            }
            "mid_enter_dismiss" => {
                context
                    .prepare_entering(count, ENTER_MIDPOINT)
                    .expect("entering Toast baseline");
                if legacy_double_layout {
                    context
                        .render_dismiss_frame_legacy_double_layout(count / 2, Duration::ZERO)
                        .expect("legacy mid-enter Toast dismiss frame")
                } else {
                    context
                        .render_dismiss_frame(count / 2, Duration::ZERO)
                        .expect("mid-enter Toast dismiss frame")
                }
            }
            "reflow" => {
                context
                    .prepare_settled(count)
                    .expect("settled Toast baseline");
                if legacy_double_layout {
                    context
                        .render_dismiss_frame_legacy_double_layout(count / 2, EXIT_MIDPOINT)
                        .expect("legacy Toast reflow frame")
                } else {
                    context
                        .render_dismiss_frame(count / 2, EXIT_MIDPOINT)
                        .expect("Toast reflow frame")
                }
            }
            _ => unreachable!("unknown Toast stage"),
        };
        assert_retained_scene_update(&format!("Toast {stage}/{count}"), stats);
        sync_bindings += stats.sync_bindings;
        scene_update += stats.scene_update;
        measure += stats.toast_measure;
        collect += stats.toast_collect;
        compose += stats.toast_compose;
        liveness += stats.renderer_liveness;
        prepare_upload += stats.renderer_prepare_upload;
        encode += stats.renderer_encode;
        submit += stats.queue_submit;
        gpu_wait += stats.gpu_wait;
        measured_cards += stats.toast_measured_cards;
        collected_cards += stats.toast_collected_cards;
        layout_passes += stats.toast_layout_passes;
        durations.push(stats.total);
    }
    let divisor = u32::try_from(samples).expect("Toast profile sample count fits u32");
    let retained_accounted = measure + collect + compose;
    let recompose_other = sync_bindings.saturating_sub(retained_accounted);
    let mode = if legacy_double_layout {
        "legacy_double_layout"
    } else {
        "single_layout"
    };
    eprintln!(
        "interaction_toast_profile_avg: cards={count} stage={stage} mode={mode} samples={samples} sync={:?} measure={:?} collect={:?} compose={:?} recompose_other={:?} scene_finalize={:?} liveness={:?} prepare_upload={:?} encode={:?} submit={:?} gpu_wait={:?} measured_cards_per_frame={:.1} collected_cards_per_frame={:.1} layout_passes_per_frame={:.1}",
        sync_bindings / divisor,
        measure / divisor,
        collect / divisor,
        compose / divisor,
        recompose_other / divisor,
        scene_update / divisor,
        liveness / divisor,
        prepare_upload / divisor,
        encode / divisor,
        submit / divisor,
        gpu_wait / divisor,
        measured_cards as f64 / samples as f64,
        collected_cards as f64 / samples as f64,
        layout_passes as f64 / samples as f64,
    );
    durations
}

#[cfg(feature = "bench-support")]
fn verify_toast_fallback(count: usize) {
    // Reduced motion makes both samples exact lifecycle endpoints, so this is a strict per-byte
    // equivalence check rather than a tolerance comparison between two advancing timestamps.
    let mut context = RuntimeToastBenchmarkContext::new(viewport(), true)
        .expect("reduced-motion Toast fallback context");
    context
        .prepare_empty()
        .expect("empty Toast fallback baseline");
    let retained = context
        .render_insert_frame(count, Duration::ZERO)
        .expect("retained Toast fallback probe");
    assert_retained_scene_update("Toast retained fallback probe", retained);
    assert_eq!(retained.toast_layout_passes, count);
    let retained_pixels = context
        .read_output_rgba()
        .expect("retained Toast output readback");
    let full = context
        .render_full_layout_control()
        .expect("Toast full-layout fallback control");
    assert_eq!(full.layout_builds, 1);
    assert_eq!(full.layout_reuses, 0);
    assert_eq!(full.toast_layout_passes, count);
    let full_pixels = context
        .read_output_rgba()
        .expect("full-layout Toast output readback");
    assert_pixel_identical(
        &format!("Toast {count}-card fallback equivalence"),
        &retained_pixels,
        &full_pixels,
    );
    let legacy = context
        .render_full_layout_legacy_double_layout_control()
        .expect("legacy double-layout Toast control");
    assert_eq!(legacy.layout_builds, 1);
    assert_eq!(legacy.layout_reuses, 0);
    assert_eq!(legacy.toast_measured_cards, count);
    assert_eq!(legacy.toast_collected_cards, count);
    assert_eq!(legacy.toast_layout_passes, count * 2);
    let legacy_pixels = context
        .read_output_rgba()
        .expect("legacy double-layout Toast output readback");
    assert_pixel_identical(
        &format!("Toast {count}-card single/double-layout equivalence"),
        &retained_pixels,
        &legacy_pixels,
    );
}

#[cfg(feature = "bench-support")]
fn assert_row_selection_full_layout_equivalent(
    label: &str,
    context: &mut RuntimeRowSelectionBenchmarkContext,
    before: &[u8],
    retained_stats: RuntimeInteractionFrameStats,
) {
    let retained = context
        .read_output_rgba()
        .expect("retained row-selection output readback");
    assert_pixels_changed(label, before, &retained);
    let retained_summary = context.scene_debug_summary();
    let full = context
        .render_full_layout_control()
        .expect("row-selection full-layout control");
    assert_eq!(full.layout_builds, 1, "{label}: full control path");
    assert_eq!(full.layout_reuses, 0, "{label}: full control path");
    let full_pixels = context
        .read_output_rgba()
        .expect("full-layout row-selection output readback");
    let full_summary = context.scene_debug_summary();
    if retained != full_pixels {
        eprintln!(
            "row_selection_equivalence_failure: label={label} retained_stats={retained_stats:?} full_stats={full:?}\nretained_scene={retained_summary}\nfull_scene={full_summary}"
        );
    }
    assert_pixel_identical(label, &retained, &full_pixels);
}

#[cfg(feature = "bench-support")]
fn verify_row_selection_equivalence(kind: RuntimeRowSelectionKind, rows: usize) {
    let label = format!("{kind:?}/{rows}_rows").to_lowercase();

    let mut pointer = RuntimeRowSelectionBenchmarkContext::new_with_reduced_motion(
        kind,
        rows,
        viewport(),
        true,
        true,
    )
    .expect("pointer selection correctness context");
    let pointer_before = pointer
        .read_output_rgba()
        .expect("pointer selection baseline readback");
    let pointer_stats = pointer
        .pointer_down(true)
        .expect("pointer selection correctness frame");
    assert_eq!(pointer_stats.command_dispatches_delta, 1);
    assert_eq!(pointer.selected_keys(), vec!["row-1".into()]);
    assert_row_selection_full_layout_equivalent(
        &format!("{label}/pointer_selection"),
        &mut pointer,
        &pointer_before,
        pointer_stats,
    );

    let mut keyboard = RuntimeRowSelectionBenchmarkContext::new_with_reduced_motion(
        kind,
        rows,
        viewport(),
        true,
        true,
    )
    .expect("keyboard selection correctness context");
    let initial = keyboard
        .pointer_down(false)
        .expect("keyboard selection correctness focus prime");
    assert_eq!(initial.command_dispatches_delta, 1);
    let _ = keyboard
        .pointer_up()
        .expect("keyboard selection correctness focus release");
    let _ = keyboard
        .render_full_layout_control()
        .expect("keyboard selection correctness baseline layout");
    let keyboard_before = keyboard
        .read_output_rgba()
        .expect("keyboard selection baseline readback");
    let keyboard_stats = keyboard
        .keyboard_move(true)
        .expect("keyboard selection correctness frame");
    assert_eq!(keyboard_stats.command_dispatches_delta, 1);
    assert_eq!(keyboard.selected_keys(), vec!["row-1".into()]);
    assert_row_selection_full_layout_equivalent(
        &format!("{label}/keyboard_selection"),
        &mut keyboard,
        &keyboard_before,
        keyboard_stats,
    );

    let mut signal = RuntimeRowSelectionBenchmarkContext::new_with_reduced_motion(
        kind,
        rows,
        viewport(),
        true,
        true,
    )
    .expect("signal-only selection correctness context");
    let signal_before = signal
        .read_output_rgba()
        .expect("signal-only selection baseline readback");
    let signal_stats = signal
        .signal_only_select(true)
        .expect("signal-only selection correctness frame");
    assert_eq!(signal_stats.command_dispatches_delta, 0);
    assert_eq!(signal.selected_keys(), vec!["row-1".into()]);
    assert_row_selection_full_layout_equivalent(
        &format!("{label}/signal_only_selection"),
        &mut signal,
        &signal_before,
        signal_stats,
    );
}

#[cfg(feature = "bench-support")]
fn profile_row_selection(kind: RuntimeRowSelectionKind) {
    const ROWS: usize = 10_000;
    const SAMPLES: usize = 80;

    let new_context = || {
        RuntimeRowSelectionBenchmarkContext::new(kind, ROWS, viewport(), true)
            .expect("row selection profile context")
    };

    let mut pointer = new_context();
    let initial = pointer
        .pointer_down(false)
        .expect("pointer selection profile baseline");
    assert_eq!(initial.command_dispatches_delta, 1);
    let _ = pointer
        .pointer_up()
        .expect("pointer selection profile baseline release");
    let mut pointer_samples = Vec::with_capacity(SAMPLES);
    let mut pointer_path = None;
    for index in 0..SAMPLES {
        let second_row = index % 2 == 0;
        let stats = pointer
            .pointer_down(second_row)
            .expect("pointer selection profile frame");
        assert_eq!(stats.command_dispatches_delta, 1);
        assert_eq!(
            pointer.selected_keys(),
            vec![if second_row { "row-1" } else { "row-0" }.into()]
        );
        pointer_path.get_or_insert(stats);
        pointer_samples.push(stats.total);
        let _ = pointer
            .pointer_up()
            .expect("pointer selection profile release");
    }

    let mut keyboard = new_context();
    let initial = keyboard
        .pointer_down(false)
        .expect("keyboard selection profile focus prime");
    assert_eq!(initial.command_dispatches_delta, 1);
    let _ = keyboard
        .pointer_up()
        .expect("keyboard selection profile focus release");
    let mut keyboard_samples = Vec::with_capacity(SAMPLES);
    let mut keyboard_path = None;
    for index in 0..SAMPLES {
        let next_row = index % 2 == 0;
        let stats = keyboard
            .keyboard_move(next_row)
            .expect("keyboard selection profile frame");
        assert_eq!(stats.command_dispatches_delta, 1);
        assert_eq!(
            keyboard.selected_keys(),
            vec![if next_row { "row-1" } else { "row-0" }.into()]
        );
        keyboard_path.get_or_insert(stats);
        keyboard_samples.push(stats.total);
    }

    let mut signal = new_context();
    let _ = signal
        .signal_only_select(false)
        .expect("signal-only selection profile baseline");
    let mut signal_samples = Vec::with_capacity(SAMPLES);
    let mut signal_path = None;
    for index in 0..SAMPLES {
        let second_row = index % 2 == 0;
        let stats = signal
            .signal_only_select(second_row)
            .expect("signal-only selection profile frame");
        assert_eq!(stats.command_dispatches_delta, 0);
        assert_eq!(
            signal.selected_keys(),
            vec![if second_row { "row-1" } else { "row-0" }.into()]
        );
        signal_path.get_or_insert(stats);
        signal_samples.push(stats.total);
    }

    let prefix = format!("{kind:?}/{ROWS}_rows").to_lowercase();
    print_budget(
        &format!("{prefix}/pointer_press_selection"),
        &pointer_samples,
    );
    print_budget(&format!("{prefix}/keyboard_selection"), &keyboard_samples);
    print_budget(&format!("{prefix}/signal_only_selection"), &signal_samples);
    eprintln!(
        "interaction_row_selection_path: kind={kind:?} adapter='{}' backend={} pointer={:?} keyboard={:?} signal={:?}",
        pointer.adapter_name,
        pointer.backend,
        pointer_path.expect("pointer path sample"),
        keyboard_path.expect("keyboard path sample"),
        signal_path.expect("signal path sample"),
    );
}

#[cfg(feature = "bench-support")]
fn profile_large_multiple_selection(kind: RuntimeRowSelectionKind) {
    const ROWS: usize = 10_000;
    const SELECTED: usize = 5_000;
    const SAMPLES: usize = 80;

    let even = (0..SELECTED)
        .map(|index| format!("row-{}", index * 2).into())
        .collect::<Vec<_>>();
    let odd = (0..SELECTED)
        .map(|index| format!("row-{}", index * 2 + 1).into())
        .collect::<Vec<_>>();
    let mut context = RuntimeRowSelectionBenchmarkContext::new_with_initial_selection(
        kind,
        ROWS,
        viewport(),
        RuntimeRowSelectionMode::Multiple,
        even.clone(),
        true,
    )
    .expect("large multiple-selection profile context");
    let before = context
        .read_output_rgba()
        .expect("large multiple-selection baseline readback");
    let first = context
        .signal_only_set_selected_keys(odd.clone())
        .expect("large multiple-selection correctness frame");
    assert_eq!(context.selected_keys().len(), SELECTED);
    assert_row_selection_full_layout_equivalent(
        &format!("{kind:?}/large_multiple_selection"),
        &mut context,
        &before,
        first,
    );

    let mut totals = Vec::with_capacity(SAMPLES);
    let mut mutations = Vec::with_capacity(SAMPLES);
    let mut syncs = Vec::with_capacity(SAMPLES);
    let mut scene_updates = Vec::with_capacity(SAMPLES);
    let mut renderer = Vec::with_capacity(SAMPLES);
    let mut first_path = None;
    for index in 0..SAMPLES {
        let selected = if index % 2 == 0 { &even } else { &odd };
        let stats = context
            .signal_only_set_selected_keys(selected.clone())
            .expect("large multiple-selection profile frame");
        assert_eq!(context.selected_keys().len(), SELECTED);
        first_path.get_or_insert(stats);
        totals.push(stats.total);
        mutations.push(stats.state_mutation);
        syncs.push(stats.sync_bindings);
        scene_updates.push(stats.scene_update);
        renderer.push(stats.renderer_total());
    }

    let prefix = format!("{kind:?}/{ROWS}_rows/{SELECTED}_selected").to_lowercase();
    print_budget(&format!("{prefix}/total"), &totals);
    print_budget(&format!("{prefix}/state_mutation"), &mutations);
    print_budget(&format!("{prefix}/sync_bindings"), &syncs);
    print_budget(&format!("{prefix}/scene_update"), &scene_updates);
    print_budget(&format!("{prefix}/renderer"), &renderer);
    eprintln!(
        "interaction_large_selection_path: kind={kind:?} adapter='{}' backend={} first_path={:?}",
        context.adapter_name,
        context.backend,
        first_path.expect("large selection path sample"),
    );
}

#[cfg(feature = "bench-support")]
fn profile_large_tree_checked_state() {
    const ROWS: usize = 10_000;
    const CHECKED: usize = 5_000;
    const SAMPLES: usize = 80;

    let even = (0..CHECKED)
        .map(|index| format!("row-{}", index * 2).into())
        .collect::<Vec<_>>();
    let odd = (0..CHECKED)
        .map(|index| format!("row-{}", index * 2 + 1).into())
        .collect::<Vec<_>>();
    let mut context = RuntimeTreeCheckedBenchmarkContext::new(ROWS, viewport(), even.clone())
        .expect("large Tree checked-state profile context");
    let first = context.set_checked_keys(odd.clone());
    assert_eq!(context.checked_keys().len(), CHECKED);
    assert!(first.rendered);
    assert!(first.sync_bindings > Duration::ZERO);

    let mut totals = Vec::with_capacity(SAMPLES);
    let mut mutations = Vec::with_capacity(SAMPLES);
    let mut syncs = Vec::with_capacity(SAMPLES);
    let mut scene_updates = Vec::with_capacity(SAMPLES);
    let mut renderer = Vec::with_capacity(SAMPLES);
    let mut first_path = None;
    for index in 0..SAMPLES {
        let checked = if index % 2 == 0 { &even } else { &odd };
        let stats = context.set_checked_keys(checked.clone());
        assert_eq!(context.checked_keys().len(), CHECKED);
        first_path.get_or_insert(stats);
        totals.push(stats.total);
        mutations.push(stats.state_mutation);
        syncs.push(stats.sync_bindings);
        scene_updates.push(stats.scene_update);
        renderer.push(stats.renderer_total());
    }

    let prefix = format!("tree/{ROWS}_rows/{CHECKED}_checked");
    print_budget(&format!("{prefix}/total"), &totals);
    print_budget(&format!("{prefix}/state_mutation"), &mutations);
    print_budget(&format!("{prefix}/sync_bindings"), &syncs);
    print_budget(&format!("{prefix}/scene_update"), &scene_updates);
    print_budget(&format!("{prefix}/renderer"), &renderer);
    eprintln!(
        "interaction_large_tree_checked_path: cpu_only=true first_path={:?}",
        first_path.expect("large Tree checked path sample"),
    );
}

#[cfg(feature = "bench-support")]
fn assert_slider_value_frame(
    label: &str,
    stats: RuntimeInteractionFrameStats,
    expect_slot_write: bool,
) {
    assert!(
        stats.event_changed,
        "{label}: signal update must change state"
    );
    assert!(stats.rendered, "{label}: signal update must render");
    assert!(
        stats.invalidation_revision_delta > 0,
        "{label}: signal update must advance invalidation"
    );
    assert_eq!(stats.layout_builds, 0, "{label}: rebuilt layout");
    assert_eq!(stats.full_rebuild_actions, 0, "{label}: rebuilt full UI");
    if expect_slot_write {
        assert_eq!(
            stats.reactive_property_slot_writes, 1,
            "{label}: expected one batched SliderValue slot write"
        );
        assert_eq!(
            stats.scene_recollects, 0,
            "{label}: retained SliderValue write recollected the scene"
        );
        assert!(
            stats.is_retained_scene_update(),
            "{label}: expected retained scene update, got {stats:?}"
        );
    } else {
        assert_eq!(
            stats.reactive_property_slot_writes, 0,
            "{label}: shadow control unexpectedly used SliderValue slot write"
        );
        assert!(
            stats.scene_recollects > 0,
            "{label}: correctness control must recollect the shadow scene"
        );
    }
}

#[cfg(feature = "bench-support")]
fn print_slider_value_profile(
    label: &str,
    slider_count: usize,
    samples: &[RuntimeInteractionFrameStats],
) {
    let totals = samples.iter().map(|stats| stats.total).collect::<Vec<_>>();
    let syncs = samples
        .iter()
        .map(|stats| stats.sync_bindings)
        .collect::<Vec<_>>();
    let scene_updates = samples
        .iter()
        .map(|stats| stats.scene_update)
        .collect::<Vec<_>>();
    print_budget(&format!("slider/{slider_count}/{label}/total"), &totals);
    print_budget(
        &format!("slider/{slider_count}/{label}/sync_bindings"),
        &syncs,
    );
    print_budget(
        &format!("slider/{slider_count}/{label}/scene_update"),
        &scene_updates,
    );
}

#[cfg(feature = "bench-support")]
fn profile_slider_value_ab(slider_count: usize) {
    const WARMUP_FRAMES: usize = 16;
    const SAMPLES: usize = 180;

    let mut shadow = RuntimeSliderValueBenchmarkContext::new(slider_count, viewport(), true)
        .expect("shadow SliderValue profile context");
    let mut flat = RuntimeSliderValueBenchmarkContext::new(slider_count, viewport(), false)
        .expect("flat SliderValue profile context");
    assert_eq!(shadow.slider_count(), slider_count);
    assert_eq!(flat.slider_count(), slider_count);
    assert_eq!(
        shadow.texture_count(),
        slider_count,
        "each shadow Slider should emit one texture"
    );
    assert_eq!(
        flat.texture_count(),
        0,
        "flat Sliders should not emit shadow textures"
    );

    let shadow_first = shadow.set_value_full_recollect_control(0.75);
    let flat_first = flat.set_value(0.75);
    assert_slider_value_frame("shadow Slider correctness frame", shadow_first, false);
    assert_slider_value_frame("flat Slider correctness frame", flat_first, true);
    shadow
        .assert_full_recollect_equivalent()
        .expect("shadow Slider retained/full scene equivalence");
    flat.assert_full_recollect_equivalent()
        .expect("flat Slider retained/full scene equivalence");

    for index in 0..WARMUP_FRAMES {
        let value = if index % 2 == 0 { 0.25 } else { 0.75 };
        let shadow_stats = shadow.set_value_full_recollect_control(value);
        let flat_stats = flat.set_value(value);
        assert_slider_value_frame("shadow Slider warmup", shadow_stats, false);
        assert_slider_value_frame("flat Slider warmup", flat_stats, true);
    }

    let mut shadow_samples = Vec::with_capacity(SAMPLES);
    let mut flat_samples = Vec::with_capacity(SAMPLES);
    for index in 0..SAMPLES {
        let value = if index % 2 == 0 { 0.25 } else { 0.75 };
        let shadow_stats = shadow.set_value_full_recollect_control(value);
        let flat_stats = flat.set_value(value);
        assert_slider_value_frame("shadow Slider profile", shadow_stats, false);
        assert_slider_value_frame("flat Slider profile", flat_stats, true);
        shadow_samples.push(shadow_stats);
        flat_samples.push(flat_stats);
    }
    assert_eq!(shadow.value(), 0.75);
    assert_eq!(flat.value(), 0.75);

    print_slider_value_profile("shadow_control", slider_count, &shadow_samples);
    print_slider_value_profile("flat_candidate", slider_count, &flat_samples);
    let (shadow_p50, shadow_p95) = duration_percentiles(&shadow_samples, |stats| stats.total);
    let (flat_p50, flat_p95) = duration_percentiles(&flat_samples, |stats| stats.total);
    let reduction = |control: Duration, candidate: Duration| {
        if control.is_zero() {
            0.0
        } else {
            (1.0 - candidate.as_secs_f64() / control.as_secs_f64()) * 100.0
        }
    };
    eprintln!(
        "interaction_slider_value_ab: cpu_only=true sliders={slider_count} samples={SAMPLES} shadow_textures={} flat_textures={} shadow_p50_ms={:.4} shadow_p95_ms={:.4} flat_p50_ms={:.4} flat_p95_ms={:.4} p50_reduction_pct={:.2} p95_reduction_pct={:.2} shadow_first={shadow_first:?} flat_first={flat_first:?}",
        shadow.texture_count(),
        flat.texture_count(),
        milliseconds(shadow_p50),
        milliseconds(shadow_p95),
        milliseconds(flat_p50),
        milliseconds(flat_p95),
        reduction(shadow_p50, flat_p50),
        reduction(shadow_p95, flat_p95),
    );
}

#[cfg(feature = "bench-support")]
fn assert_text_content_frame(label: &str, stats: RuntimeInteractionFrameStats) {
    assert!(
        stats.event_changed,
        "{label}: signal update was not observed"
    );
    assert!(stats.rendered, "{label}: signal update did not render");
    assert_eq!(
        stats.reactive_property_slot_writes, 1,
        "{label}: expected one batched TextContent slot write"
    );
    assert_eq!(
        stats.scene_recollects, 0,
        "{label}: retained TextContent update recollected the scene"
    );
    assert_eq!(stats.layout_builds, 0, "{label}: rebuilt layout");
    assert!(
        stats.is_retained_scene_update(),
        "{label}: expected retained scene update, got {stats:?}"
    );
}

#[cfg(feature = "bench-support")]
fn profile_text_content_resolve_ab(text_count: usize) {
    const WARMUP_FRAMES: usize = 24;
    const SAMPLES: usize = 240;
    const FIRST: &str = "Frame 111111";
    const SECOND: &str = "Frame 999999";

    let mut direct = RuntimeTextContentBenchmarkContext::new(text_count, viewport())
        .expect("direct TextContent benchmark context");
    let mut legacy = RuntimeTextContentBenchmarkContext::new(text_count, viewport())
        .expect("legacy TextContent benchmark context");
    assert_eq!(direct.text_count(), text_count);
    assert_eq!(legacy.text_count(), text_count);

    let direct_first = direct.set_content(FIRST);
    let legacy_first = legacy.set_content_legacy_full_visual(FIRST);
    assert_text_content_frame("direct TextContent correctness frame", direct_first);
    assert_text_content_frame("legacy TextContent correctness frame", legacy_first);
    direct
        .assert_scene_equivalent(&mut legacy)
        .expect("direct/legacy TextContent scene equivalence");
    direct
        .assert_full_recollect_equivalent()
        .expect("direct TextContent full-recollect equivalence");
    legacy
        .assert_full_recollect_equivalent()
        .expect("legacy TextContent full-recollect equivalence");

    for index in 0..WARMUP_FRAMES {
        let content = if index % 2 == 0 { SECOND } else { FIRST };
        assert_text_content_frame("direct TextContent warmup", direct.set_content(content));
        assert_text_content_frame(
            "legacy TextContent warmup",
            legacy.set_content_legacy_full_visual(content),
        );
    }

    let mut direct_samples = Vec::with_capacity(SAMPLES);
    let mut legacy_samples = Vec::with_capacity(SAMPLES);
    for index in 0..SAMPLES {
        let content = if index % 2 == 0 { SECOND } else { FIRST };
        let direct_stats = direct.set_content(content);
        let legacy_stats = legacy.set_content_legacy_full_visual(content);
        assert_text_content_frame("direct TextContent profile", direct_stats);
        assert_text_content_frame("legacy TextContent profile", legacy_stats);
        direct_samples.push(direct_stats);
        legacy_samples.push(legacy_stats);
    }
    assert_eq!(direct.content(), FIRST);
    assert_eq!(legacy.content(), FIRST);
    direct
        .assert_scene_equivalent(&mut legacy)
        .expect("profiled direct/legacy TextContent scene equivalence");

    let (direct_p50, direct_p95) = duration_percentiles(&direct_samples, |stats| stats.total);
    let (legacy_p50, legacy_p95) = duration_percentiles(&legacy_samples, |stats| stats.total);
    let (direct_sync_p50, direct_sync_p95) =
        duration_percentiles(&direct_samples, |stats| stats.sync_bindings);
    let (legacy_sync_p50, legacy_sync_p95) =
        duration_percentiles(&legacy_samples, |stats| stats.sync_bindings);
    let reduction = |control: Duration, candidate: Duration| {
        if control.is_zero() {
            0.0
        } else {
            (1.0 - candidate.as_secs_f64() / control.as_secs_f64()) * 100.0
        }
    };
    eprintln!(
        "interaction_text_content_resolve_ab: cpu_only=true texts={text_count} samples={SAMPLES} direct_p50_ms={:.4} direct_p95_ms={:.4} legacy_p50_ms={:.4} legacy_p95_ms={:.4} total_p50_reduction_pct={:.2} total_p95_reduction_pct={:.2} direct_sync_p50_ms={:.4} direct_sync_p95_ms={:.4} legacy_sync_p50_ms={:.4} legacy_sync_p95_ms={:.4} sync_p50_reduction_pct={:.2} sync_p95_reduction_pct={:.2} direct_first={direct_first:?} legacy_first={legacy_first:?}",
        milliseconds(direct_p50),
        milliseconds(direct_p95),
        milliseconds(legacy_p50),
        milliseconds(legacy_p95),
        reduction(legacy_p50, direct_p50),
        reduction(legacy_p95, direct_p95),
        milliseconds(direct_sync_p50),
        milliseconds(direct_sync_p95),
        milliseconds(legacy_sync_p50),
        milliseconds(legacy_sync_p95),
        reduction(legacy_sync_p50, direct_sync_p50),
        reduction(legacy_sync_p95, direct_sync_p95),
    );
}

#[cfg(feature = "bench-support")]
fn duration_percentiles(
    samples: &[RuntimeInteractionFrameStats],
    getter: impl Fn(RuntimeInteractionFrameStats) -> Duration,
) -> (Duration, Duration) {
    let samples = samples.iter().copied().map(getter).collect::<Vec<_>>();
    (percentile(&samples, 0.50), percentile(&samples, 0.95))
}

#[cfg(feature = "bench-support")]
fn print_focus_phase_profile(
    label: &str,
    candidates: usize,
    samples: &[RuntimeInteractionFrameStats],
    cache: RuntimeFocusNavigationCacheStats,
) {
    let totals = samples.iter().map(|stats| stats.total).collect::<Vec<_>>();
    print_budget(label, &totals);
    let (total_p50, total_p95) = duration_percentiles(samples, |stats| stats.total);
    let (event_p50, event_p95) = duration_percentiles(samples, |stats| stats.event_handling);
    let (sync_p50, sync_p95) = duration_percentiles(samples, |stats| stats.sync_bindings);
    let (scene_p50, scene_p95) = duration_percentiles(samples, |stats| stats.scene_update);
    let (scene_layout_p50, scene_layout_p95) = duration_percentiles(
        samples,
        RuntimeInteractionFrameStats::scene_and_layout_total,
    );
    let (renderer_p50, renderer_p95) =
        duration_percentiles(samples, RuntimeInteractionFrameStats::renderer_total);
    let (liveness_p50, liveness_p95) =
        duration_percentiles(samples, |stats| stats.renderer_liveness);
    let (prepare_p50, prepare_p95) =
        duration_percentiles(samples, |stats| stats.renderer_prepare_upload);
    let (encode_p50, encode_p95) = duration_percentiles(samples, |stats| stats.renderer_encode);
    let (submit_p50, submit_p95) = duration_percentiles(samples, |stats| stats.queue_submit);
    let (wait_p50, wait_p95) = duration_percentiles(samples, |stats| stats.gpu_wait);
    let first = samples
        .first()
        .copied()
        .expect("focus profile requires at least one sample");
    eprintln!(
        "interaction_focus_phase: path={label} candidates={candidates} samples={} cache_builds={} cache_validations={} cache_hits={} total_p50_ms={:.4} total_p95_ms={:.4} event_p50_ms={:.4} event_p95_ms={:.4} sync_p50_ms={:.4} sync_p95_ms={:.4} scene_update_p50_ms={:.4} scene_update_p95_ms={:.4} scene_layout_p50_ms={:.4} scene_layout_p95_ms={:.4} renderer_p50_ms={:.4} renderer_p95_ms={:.4} liveness_p50_ms={:.4} liveness_p95_ms={:.4} prepare_p50_ms={:.4} prepare_p95_ms={:.4} encode_p50_ms={:.4} encode_p95_ms={:.4} submit_p50_ms={:.4} submit_p95_ms={:.4} gpu_wait_p50_ms={:.4} gpu_wait_p95_ms={:.4} first_path={first:?}",
        samples.len(),
        cache.builds,
        cache.validations,
        cache.hits,
        milliseconds(total_p50),
        milliseconds(total_p95),
        milliseconds(event_p50),
        milliseconds(event_p95),
        milliseconds(sync_p50),
        milliseconds(sync_p95),
        milliseconds(scene_p50),
        milliseconds(scene_p95),
        milliseconds(scene_layout_p50),
        milliseconds(scene_layout_p95),
        milliseconds(renderer_p50),
        milliseconds(renderer_p95),
        milliseconds(liveness_p50),
        milliseconds(liveness_p95),
        milliseconds(prepare_p50),
        milliseconds(prepare_p95),
        milliseconds(encode_p50),
        milliseconds(encode_p95),
        milliseconds(submit_p50),
        milliseconds(submit_p95),
        milliseconds(wait_p50),
        milliseconds(wait_p95),
    );
}

#[cfg(feature = "bench-support")]
fn assert_focus_pipeline_frame(
    label: &str,
    stats: RuntimeInteractionFrameStats,
    expected_dispatches: usize,
) {
    assert!(
        stats.event_changed,
        "{label}: event must change runtime state"
    );
    assert!(
        stats.rendered,
        "{label}: event must render a production frame"
    );
    assert_eq!(
        stats.command_dispatches_delta, expected_dispatches,
        "{label}: unexpected command dispatch count"
    );
}

#[cfg(feature = "bench-support")]
fn assert_no_ui_command_frame(label: &str, stats: RuntimeInteractionFrameStats) {
    assert!(
        !stats.event_changed,
        "{label}: a NoUiChange command must not dirty runtime UI state"
    );
    assert!(
        !stats.rendered,
        "{label}: a NoUiChange command must not submit a redundant frame"
    );
    assert_eq!(stats.command_dispatches_delta, 1);
    assert_eq!(stats.invalidation_revision_delta, 0);
    assert_eq!(stats.layout_builds, 0);
    assert_eq!(stats.layout_reuses, 0);
    assert_eq!(stats.scene_recollects, 0);
    assert_eq!(stats.retained_scene_patches, 0);
    assert_eq!(stats.renderer_total(), Duration::ZERO);
}

#[cfg(feature = "bench-support")]
fn profile_no_ui_command_effect_path(
    label: &str,
    buttons: usize,
    activate: fn(&mut RuntimeFocusBenchmarkContext) -> Result<RuntimeInteractionFrameStats, String>,
) {
    const WARMUP_FRAMES: usize = 12;
    const SAMPLES: usize = 80;

    let mut conservative = RuntimeFocusBenchmarkContext::new(buttons, viewport())
        .expect("conservative command-effect benchmark context");
    let mut no_ui = RuntimeFocusBenchmarkContext::new_with_command_effect(
        buttons,
        viewport(),
        CommandEffect::NoUiChange,
    )
    .expect("NoUiChange command-effect benchmark context");
    let conservative_focus = conservative
        .tab_forward()
        .expect("conservative command-effect initial focus");
    let no_ui_focus = no_ui
        .tab_forward()
        .expect("NoUiChange command-effect initial focus");
    assert_focus_pipeline_frame(
        "conservative command-effect initial focus",
        conservative_focus,
        0,
    );
    assert_focus_pipeline_frame("NoUiChange command-effect initial focus", no_ui_focus, 0);

    for _ in 0..WARMUP_FRAMES {
        let conservative_stats =
            activate(&mut conservative).expect("conservative command-effect warmup");
        assert_focus_pipeline_frame("conservative command-effect warmup", conservative_stats, 1);
        let no_ui_stats = activate(&mut no_ui).expect("NoUiChange command-effect warmup");
        assert_no_ui_command_frame("NoUiChange command-effect warmup", no_ui_stats);
    }

    let mut conservative_samples = Vec::with_capacity(SAMPLES);
    let mut no_ui_samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let conservative_stats =
            activate(&mut conservative).expect("conservative command-effect profile frame");
        assert_focus_pipeline_frame("conservative command-effect profile", conservative_stats, 1);
        let no_ui_stats = activate(&mut no_ui).expect("NoUiChange command-effect profile frame");
        assert_no_ui_command_frame("NoUiChange command-effect profile", no_ui_stats);
        conservative_samples.push(conservative_stats);
        no_ui_samples.push(no_ui_stats);
    }

    assert_eq!(conservative.focused_index(), Some(0));
    assert_eq!(no_ui.focused_index(), Some(0));
    assert_eq!(conservative.last_activated_index(), Some(0));
    assert_eq!(no_ui.last_activated_index(), Some(0));
    assert_eq!(
        conservative.activation_dispatches(),
        no_ui.activation_dispatches(),
        "command effects must dispatch the same number of handlers"
    );
    let conservative_pixels = conservative
        .read_output_rgba()
        .expect("conservative command-effect output readback");
    let no_ui_pixels = no_ui
        .read_output_rgba()
        .expect("NoUiChange command-effect output readback");
    assert_pixel_identical(
        &format!("{label} NoUiChange/conservative output"),
        &no_ui_pixels,
        &conservative_pixels,
    );

    print_focus_phase_profile(
        &format!("focus/{buttons}_buttons/{label}/conservative"),
        buttons,
        &conservative_samples,
        conservative.focus_navigation_cache_stats(),
    );
    print_focus_phase_profile(
        &format!("focus/{buttons}_buttons/{label}/no_ui_change"),
        buttons,
        &no_ui_samples,
        no_ui.focus_navigation_cache_stats(),
    );
    let conservative_p50 = duration_percentiles(&conservative_samples, |stats| stats.total).0;
    let no_ui_p50 = duration_percentiles(&no_ui_samples, |stats| stats.total).0;
    let reduction = if conservative_p50.is_zero() {
        0.0
    } else {
        (1.0 - no_ui_p50.as_secs_f64() / conservative_p50.as_secs_f64()) * 100.0
    };
    eprintln!(
        "interaction_command_effect_ab: path={label} buttons={buttons} conservative_p50_ms={:.4} no_ui_p50_ms={:.4} reduction_pct={reduction:.2}",
        milliseconds(conservative_p50),
        milliseconds(no_ui_p50),
    );
}

#[cfg(feature = "bench-support")]
fn profile_no_ui_command_effect(buttons: usize) {
    profile_no_ui_command_effect_path(
        "enter_activation",
        buttons,
        RuntimeFocusBenchmarkContext::activate_enter,
    );
    profile_no_ui_command_effect_path(
        "space_activation",
        buttons,
        RuntimeFocusBenchmarkContext::activate_space,
    );
}

#[cfg(feature = "bench-support")]
fn profile_focus_pipeline(buttons: usize) {
    const WARMUP_FRAMES: usize = 12;
    const SAMPLES: usize = 80;

    let mut tab = RuntimeFocusBenchmarkContext::new(buttons, viewport())
        .expect("Tab focus benchmark context");
    assert_eq!(tab.focusable_count(), buttons);
    for _ in 0..WARMUP_FRAMES {
        let stats = tab.tab_forward().expect("Tab focus warmup frame");
        assert_focus_pipeline_frame("Tab warmup", stats, 0);
    }
    let mut tab_samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let stats = tab.tab_forward().expect("Tab focus profile frame");
        assert_focus_pipeline_frame("Tab profile", stats, 0);
        tab_samples.push(stats);
    }
    print_focus_phase_profile(
        &format!("focus/{buttons}_buttons/tab_forward"),
        buttons,
        &tab_samples,
        tab.focus_navigation_cache_stats(),
    );

    let mut enter = RuntimeFocusBenchmarkContext::new_no_ui_change(buttons, viewport())
        .expect("Enter activation benchmark context");
    let initial_focus = enter.tab_forward().expect("Enter initial focus frame");
    assert_focus_pipeline_frame("Enter initial focus", initial_focus, 0);
    for _ in 0..WARMUP_FRAMES {
        let stats = enter
            .activate_enter()
            .expect("Enter activation warmup frame");
        assert_no_ui_command_frame("Enter warmup", stats);
    }
    let mut enter_samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let stats = enter
            .activate_enter()
            .expect("Enter activation profile frame");
        assert_no_ui_command_frame("Enter profile", stats);
        enter_samples.push(stats);
    }
    assert_eq!(enter.focused_index(), Some(0));
    assert_eq!(enter.last_activated_index(), Some(0));
    print_focus_phase_profile(
        &format!("focus/{buttons}_buttons/enter_activation/no_ui_change"),
        buttons,
        &enter_samples,
        enter.focus_navigation_cache_stats(),
    );

    let mut space = RuntimeFocusBenchmarkContext::new_no_ui_change(buttons, viewport())
        .expect("Space activation benchmark context");
    let initial_focus = space.tab_forward().expect("Space initial focus frame");
    assert_focus_pipeline_frame("Space initial focus", initial_focus, 0);
    for _ in 0..WARMUP_FRAMES {
        let stats = space
            .activate_space()
            .expect("Space activation warmup frame");
        assert_no_ui_command_frame("Space warmup", stats);
    }
    let mut space_samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let stats = space
            .activate_space()
            .expect("Space activation profile frame");
        assert_no_ui_command_frame("Space profile", stats);
        space_samples.push(stats);
    }
    assert_eq!(space.focused_index(), Some(0));
    assert_eq!(space.last_activated_index(), Some(0));
    print_focus_phase_profile(
        &format!("focus/{buttons}_buttons/space_activation/no_ui_change"),
        buttons,
        &space_samples,
        space.focus_navigation_cache_stats(),
    );

    eprintln!(
        "interaction_focus_context: buttons={buttons} candidates={} adapter='{}' backend={} state_assertions=exact_focus_order_and_single_focused_handler",
        tab.focusable_count(), tab.adapter_name, tab.backend,
    );
}

#[cfg(feature = "bench-support")]
fn bench_focus_pipeline(c: &mut Criterion, buttons: usize) {
    let mut tab = RuntimeFocusBenchmarkContext::new(buttons, viewport())
        .expect("criterion Tab focus benchmark context");
    let mut enter = RuntimeFocusBenchmarkContext::new_no_ui_change(buttons, viewport())
        .expect("criterion Enter activation benchmark context");
    let mut space = RuntimeFocusBenchmarkContext::new_no_ui_change(buttons, viewport())
        .expect("criterion Space activation benchmark context");
    let initial_enter_focus = enter.tab_forward().expect("criterion Enter initial focus");
    assert_focus_pipeline_frame("criterion Enter initial focus", initial_enter_focus, 0);
    let initial_space_focus = space.tab_forward().expect("criterion Space initial focus");
    assert_focus_pipeline_frame("criterion Space initial focus", initial_space_focus, 0);

    let mut group = c.benchmark_group("interaction_focus_frame");
    group.sample_size(20);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(4));
    group.throughput(Throughput::Elements(buttons as u64));
    group.bench_function(format!("tab_forward/{buttons}_buttons"), |b| {
        b.iter_custom(|iterations| {
            let mut total = Duration::ZERO;
            for _ in 0..iterations {
                let stats = tab.tab_forward().expect("criterion Tab focus frame");
                assert_focus_pipeline_frame("criterion Tab", stats, 0);
                total += stats.total;
            }
            total
        });
    });
    group.bench_function(format!("enter_activation/{buttons}_buttons"), |b| {
        b.iter_custom(|iterations| {
            let mut total = Duration::ZERO;
            for _ in 0..iterations {
                let stats = enter
                    .activate_enter()
                    .expect("criterion Enter activation frame");
                assert_no_ui_command_frame("criterion Enter", stats);
                total += stats.total;
            }
            total
        });
    });
    group.bench_function(format!("space_activation/{buttons}_buttons"), |b| {
        b.iter_custom(|iterations| {
            let mut total = Duration::ZERO;
            for _ in 0..iterations {
                let stats = space
                    .activate_space()
                    .expect("criterion Space activation frame");
                assert_no_ui_command_frame("criterion Space", stats);
                total += stats.total;
            }
            total
        });
    });
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_interaction_frame_pipeline(c: &mut Criterion) {
    if std::env::var_os("TGUI_INTERACTION_BUTTON_HOVER_PROFILE").is_some() {
        let buttons = std::env::var("TGUI_INTERACTION_FOCUS_BUTTONS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(10_000);
        verify_button_hover_equivalence(buttons);
        profile_button_hover_ab(buttons);
        verify_button_pressed_equivalence(buttons);
        profile_button_pressed_ab(buttons);
        let mut group = c.benchmark_group("interaction_button_hover_profile");
        group.sample_size(10);
        group.warm_up_time(Duration::from_millis(100));
        group.measurement_time(Duration::from_millis(100));
        group.bench_function("completed", |b| b.iter(|| black_box(1_u8)));
        group.finish();
        return;
    }

    if std::env::var_os("TGUI_INTERACTION_COMMAND_EFFECT_PROFILE").is_some() {
        let buttons = std::env::var("TGUI_INTERACTION_FOCUS_BUTTONS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(10_000);
        profile_no_ui_command_effect(buttons);
        let mut group = c.benchmark_group("interaction_command_effect_profile");
        group.sample_size(10);
        group.warm_up_time(Duration::from_millis(100));
        group.measurement_time(Duration::from_millis(100));
        group.bench_function("completed", |b| b.iter(|| black_box(1_u8)));
        group.finish();
        return;
    }

    if std::env::var_os("TGUI_INTERACTION_FOCUS_PROFILE").is_some() {
        let buttons = std::env::var("TGUI_INTERACTION_FOCUS_BUTTONS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(10_000);
        profile_focus_pipeline(buttons);
        let mut group = c.benchmark_group("interaction_focus_profile");
        group.sample_size(10);
        group.warm_up_time(Duration::from_millis(100));
        group.measurement_time(Duration::from_millis(100));
        group.bench_function("completed", |b| b.iter(|| black_box(1_u8)));
        group.finish();
        return;
    }

    if std::env::var_os("TGUI_INTERACTION_ROW_SELECTION_PROFILE").is_some() {
        for kind in [
            RuntimeRowSelectionKind::List,
            RuntimeRowSelectionKind::Tree,
            RuntimeRowSelectionKind::DataGrid,
        ] {
            for rows in [10, 10_000] {
                verify_row_selection_equivalence(kind, rows);
            }
            profile_row_selection(kind);
        }
        let mut group = c.benchmark_group("interaction_row_selection_profile");
        group.sample_size(10);
        group.warm_up_time(Duration::from_millis(100));
        group.measurement_time(Duration::from_millis(100));
        group.bench_function("completed", |b| b.iter(|| black_box(1_u8)));
        group.finish();
        return;
    }

    if std::env::var_os("TGUI_INTERACTION_LARGE_SELECTION_PROFILE").is_some() {
        for kind in [
            RuntimeRowSelectionKind::List,
            RuntimeRowSelectionKind::Tree,
            RuntimeRowSelectionKind::DataGrid,
        ] {
            profile_large_multiple_selection(kind);
        }
        let mut group = c.benchmark_group("interaction_large_selection_profile");
        group.sample_size(10);
        group.warm_up_time(Duration::from_millis(100));
        group.measurement_time(Duration::from_millis(100));
        group.bench_function("completed", |b| b.iter(|| black_box(1_u8)));
        group.finish();
        return;
    }

    if std::env::var_os("TGUI_INTERACTION_TREE_CHECKED_PROFILE").is_some() {
        profile_large_tree_checked_state();
        let mut group = c.benchmark_group("interaction_tree_checked_profile");
        group.sample_size(10);
        group.warm_up_time(Duration::from_millis(100));
        group.measurement_time(Duration::from_millis(100));
        group.bench_function("completed", |b| b.iter(|| black_box(1_u8)));
        group.finish();
        return;
    }

    if std::env::var_os("TGUI_INTERACTION_SLIDER_VALUE_PROFILE").is_some() {
        for slider_count in [1, 24] {
            profile_slider_value_ab(slider_count);
        }
        let mut group = c.benchmark_group("interaction_slider_value_profile");
        group.sample_size(10);
        group.warm_up_time(Duration::from_millis(100));
        group.measurement_time(Duration::from_millis(100));
        group.bench_function("completed", |b| b.iter(|| black_box(1_u8)));
        group.finish();
        return;
    }

    if std::env::var_os("TGUI_INTERACTION_TEXT_CONTENT_PROFILE").is_some() {
        let text_count = std::env::var("TGUI_INTERACTION_TEXT_COUNT")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(1_000);
        profile_text_content_resolve_ab(text_count);
        let mut group = c.benchmark_group("interaction_text_content_profile");
        group.sample_size(10);
        group.warm_up_time(Duration::from_millis(100));
        group.measurement_time(Duration::from_millis(100));
        group.bench_function("completed", |b| b.iter(|| black_box(1_u8)));
        group.finish();
        return;
    }

    bench_focus_pipeline(c, 10_000);

    for kind in [RuntimeRowHoverKind::List, RuntimeRowHoverKind::Tree] {
        verify_row_hover_equivalence(kind);
        let mut optimized = RuntimeRowHoverBenchmarkContext::new(kind, 10_000, viewport())
            .expect("optimized production row-hover benchmark context");
        let mut control = RuntimeRowHoverBenchmarkContext::new(kind, 10_000, viewport())
            .expect("control production row-hover benchmark context");
        profile_row_hover_ab(&mut optimized, &mut control);
    }
    let mut data_grid = RuntimeDataGridBenchmarkContext::new(10_000, viewport())
        .expect("production DataGrid benchmark context");
    profile_data_grid(&mut data_grid);
    let mut data_grid_control = RuntimeDataGridBenchmarkContext::new(10_000, viewport())
        .expect("production DataGrid scene-recollect control context");
    profile_data_grid_ab(&mut data_grid, &mut data_grid_control);

    let _ = data_grid
        .move_hover(RuntimeDataGridHoverTarget::FirstRowStartPinned)
        .expect("DataGrid benchmark starting hover");
    let mut next_row = true;
    let mut control_next_row = true;
    let mut grid_group = c.benchmark_group("interaction_data_grid_frame");
    grid_group.sample_size(20);
    grid_group.warm_up_time(Duration::from_secs(1));
    grid_group.measurement_time(Duration::from_secs(4));
    grid_group.throughput(Throughput::Elements(10_000));
    grid_group.bench_function("row_hover_scene_only/10000_rows", |b| {
        b.iter_custom(|iterations| {
            let mut total = Duration::ZERO;
            for _ in 0..iterations {
                let target = if next_row {
                    RuntimeDataGridHoverTarget::SecondRowStartPinned
                } else {
                    RuntimeDataGridHoverTarget::FirstRowStartPinned
                };
                next_row = !next_row;
                let stats = data_grid
                    .move_hover(black_box(target))
                    .expect("DataGrid benchmark hover frame");
                assert_retained_scene_update("DataGrid benchmark row hover", stats);
                total += stats.total;
            }
            total
        });
    });
    grid_group.bench_function("row_hover_scene_recollect_control/10000_rows", |b| {
        b.iter_custom(|iterations| {
            let mut total = Duration::ZERO;
            for _ in 0..iterations {
                let target = if control_next_row {
                    RuntimeDataGridHoverTarget::SecondRowStartPinned
                } else {
                    RuntimeDataGridHoverTarget::FirstRowStartPinned
                };
                control_next_row = !control_next_row;
                let stats = data_grid_control
                    .move_hover_scene_recollect_control(black_box(target))
                    .expect("DataGrid scene-recollect control frame");
                assert!(stats.is_scene_only_recollect());
                total += stats.total;
            }
            total
        });
    });
    grid_group.finish();

    for count in [10_usize, 50] {
        verify_toast_fallback(count);
        let mut context = RuntimeToastBenchmarkContext::new(viewport(), false)
            .expect("production Toast benchmark context");
        eprintln!(
            "interaction_frame_path: widget=toast adapter='{}' backend={} cards={count} expanded=true",
            context.adapter_name, context.backend,
        );
        for stage in ["insert", "mid_enter_dismiss", "reflow"] {
            let samples = profile_toast_stage(&mut context, count, stage, 40, false);
            print_budget(&format!("toast/{count}_cards/{stage}"), &samples);
        }
        if count == 50 {
            for stage in ["insert", "mid_enter_dismiss", "reflow"] {
                let samples = profile_toast_stage(&mut context, count, stage, 40, true);
                print_budget(
                    &format!("toast/{count}_cards/{stage}/legacy_double_layout"),
                    &samples,
                );
            }

            context
                .prepare_settled(count)
                .expect("settled Toast prepared-cache baseline");
            context
                .prime_prepared_card_cache()
                .expect("prime Toast prepared-card cache");
            let optimized = context
                .render_prepared_card_cache_frame()
                .expect("Toast prepared-card equivalence frame");
            let optimized_pixels = context
                .read_output_rgba()
                .expect("Toast prepared-card output readback");
            let control = context
                .render_prepared_card_control_frame()
                .expect("Toast prepared-card control frame");
            let control_pixels = context
                .read_output_rgba()
                .expect("Toast prepared-card control output readback");
            assert_pixel_identical(
                "Toast 50-card prepared-cache equivalence",
                &optimized_pixels,
                &control_pixels,
            );
            eprintln!(
                "interaction_toast_prepared_cache_ab: cards=50 optimized_total={:?} optimized_measure={:?} optimized_collect={:?} control_total={:?} control_measure={:?} control_collect={:?}",
                optimized.total,
                optimized.toast_measure,
                optimized.toast_collect,
                control.total,
                control.toast_measure,
                control.toast_collect,
            );

            context
                .prime_toast_base_scene_cache()
                .expect("prime Toast canonical base-scene cache");
            let optimized = context
                .render_toast_base_scene_cache_frame()
                .expect("Toast base-scene replay equivalence frame");
            let optimized_pixels = context
                .read_output_rgba()
                .expect("Toast base-scene replay output readback");
            let control = context
                .render_toast_base_scene_control_frame()
                .expect("Toast base-scene collector control frame");
            let control_pixels = context
                .read_output_rgba()
                .expect("Toast base-scene collector output readback");
            assert_pixel_identical(
                "Toast 50-card base-scene replay equivalence",
                &optimized_pixels,
                &control_pixels,
            );
            eprintln!(
                "interaction_toast_base_scene_ab: cards=50 optimized_total={:?} optimized_collect={:?} control_total={:?} control_collect={:?}",
                optimized.total,
                optimized.toast_collect,
                control.total,
                control.toast_collect,
            );
        }

        let mut toast_group = c.benchmark_group(format!("interaction_toast_frame/{count}_cards"));
        toast_group.sample_size(20);
        toast_group.warm_up_time(Duration::from_secs(1));
        toast_group.measurement_time(Duration::from_secs(4));
        toast_group.throughput(Throughput::Elements(count as u64));
        for stage in ["insert", "mid_enter_dismiss", "reflow"] {
            toast_group.bench_function(stage, |b| {
                b.iter_custom(|iterations| {
                    let mut total = Duration::ZERO;
                    for _ in 0..iterations {
                        let stats = match stage {
                            "insert" => {
                                context.prepare_empty().expect("empty Toast baseline");
                                context
                                    .render_insert_frame(count, ENTER_MIDPOINT)
                                    .expect("Toast insert benchmark frame")
                            }
                            "mid_enter_dismiss" => {
                                context
                                    .prepare_entering(count, ENTER_MIDPOINT)
                                    .expect("entering Toast benchmark baseline");
                                context
                                    .render_dismiss_frame(count / 2, Duration::ZERO)
                                    .expect("Toast mid-enter dismiss benchmark frame")
                            }
                            "reflow" => {
                                context
                                    .prepare_settled(count)
                                    .expect("settled Toast benchmark baseline");
                                context
                                    .render_dismiss_frame(count / 2, EXIT_MIDPOINT)
                                    .expect("Toast reflow benchmark frame")
                            }
                            _ => unreachable!(),
                        };
                        assert_retained_scene_update("Toast benchmark frame", stats);
                        total += stats.total;
                    }
                    total
                });
            });
        }
        if count == 50 {
            context
                .prepare_settled(count)
                .expect("settled Toast prepared-cache benchmark baseline");
            context
                .prime_prepared_card_cache()
                .expect("prime Toast prepared-card benchmark cache");
            toast_group.bench_function("motion_prepared_card_cache", |b| {
                b.iter_custom(|iterations| {
                    let mut total = Duration::ZERO;
                    for _ in 0..iterations {
                        total += context
                            .render_prepared_card_cache_frame()
                            .expect("Toast prepared-card benchmark frame")
                            .total;
                    }
                    total
                });
            });
            toast_group.bench_function("motion_prepared_card_control", |b| {
                b.iter_custom(|iterations| {
                    let mut total = Duration::ZERO;
                    for _ in 0..iterations {
                        total += context
                            .render_prepared_card_control_frame()
                            .expect("Toast prepared-card control benchmark frame")
                            .total;
                    }
                    total
                });
            });
            context
                .prime_toast_base_scene_cache()
                .expect("prime Toast base-scene benchmark cache");
            toast_group.bench_function("motion_base_scene_cache", |b| {
                b.iter_custom(|iterations| {
                    let mut total = Duration::ZERO;
                    for _ in 0..iterations {
                        total += context
                            .render_toast_base_scene_cache_frame()
                            .expect("Toast base-scene benchmark frame")
                            .total;
                    }
                    total
                });
            });
            toast_group.bench_function("motion_base_scene_control", |b| {
                b.iter_custom(|iterations| {
                    let mut total = Duration::ZERO;
                    for _ in 0..iterations {
                        total += context
                            .render_toast_base_scene_control_frame()
                            .expect("Toast base-scene control frame")
                            .total;
                    }
                    total
                });
            });
        }
        toast_group.finish();
    }
}

#[cfg(not(feature = "bench-support"))]
fn bench_interaction_frame_pipeline(_c: &mut Criterion) {}

criterion_group!(benches, bench_interaction_frame_pipeline);
criterion_main!(benches);
