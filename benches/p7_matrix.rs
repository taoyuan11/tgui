use std::env;
use std::hint::black_box;
use std::process::Command;
use std::time::{Duration, Instant};

use tgui::core::{DpiScale, PropertyId, Size};
use tgui::diagnostics::FrameMetrics;
use tgui::layout::{Dimension, FlexWrap, LayoutStyle};
use tgui::widget::{PropertyImpact, WidgetNode};
use tgui::{Application, Result, WindowSpec};

const NODE_COUNTS: [usize; 6] = [10, 100, 1_000, 5_000, 10_000, 50_000];
const PAINT_REVISION: PropertyId = PropertyId::new(7);
const DEFAULT_SAMPLES: usize = 3;

struct MatrixRoot;
struct MatrixLeaf;

#[derive(Clone, Copy, Debug)]
enum Scenario {
    InitialRender,
    Idle,
    SinglePaint,
    LocalPaint100,
    IsolatedLayout,
    KeyedReorder,
    FullLayoutFallback,
}

impl Scenario {
    const ALL: [Self; 7] = [
        Self::InitialRender,
        Self::Idle,
        Self::SinglePaint,
        Self::LocalPaint100,
        Self::IsolatedLayout,
        Self::KeyedReorder,
        Self::FullLayoutFallback,
    ];

    const fn name(self) -> &'static str {
        match self {
            Self::InitialRender => "initial_render",
            Self::Idle => "idle",
            Self::SinglePaint => "single_paint",
            Self::LocalPaint100 => "local_paint_100",
            Self::IsolatedLayout => "isolated_layout",
            Self::KeyedReorder => "keyed_reorder",
            Self::FullLayoutFallback => "full_layout_fallback",
        }
    }
}

#[derive(Clone, Copy)]
enum Declaration {
    Base,
    Paint(usize),
    Layout,
    Reordered,
    AmbiguousKeys,
}

#[derive(Clone)]
struct Sample {
    total: Duration,
    update: Duration,
    frame: FrameMetrics,
    rss_bytes: Option<u64>,
}

fn declaration(node_count: usize, declaration: Declaration) -> WidgetNode {
    assert!(node_count >= 3);
    let mut root_style =
        LayoutStyle::default().with_size(Dimension::Length(1_000.0), Dimension::Length(800.0));
    root_style.flex_wrap = FlexWrap::Wrap;
    let leaf_count = node_count - 1;
    let children = (0..leaf_count).map(|offset| {
        let index = if matches!(declaration, Declaration::Reordered) {
            leaf_count - offset - 1
        } else {
            offset
        };
        let key = if matches!(declaration, Declaration::AmbiguousKeys) && offset + 1 == leaf_count {
            leaf_count - 2
        } else {
            index
        };
        let width = if matches!(declaration, Declaration::Layout) && index == 0 {
            11.0
        } else {
            10.0
        };
        let changed = match declaration {
            Declaration::Paint(changed) => index < changed,
            _ => false,
        };
        WidgetNode::new::<MatrixLeaf>()
            .with_key(key as u64)
            .with_property(PAINT_REVISION, u64::from(changed))
            .with_property_impact(PAINT_REVISION, PropertyImpact::PAINT)
            .with_layout_style(
                LayoutStyle::default().with_size(Dimension::Length(width), Dimension::Length(10.0)),
            )
    });
    WidgetNode::new::<MatrixRoot>()
        .with_layout_style(root_style)
        .with_children(children)
}

fn setup(node_count: usize) -> Result<(Application, tgui::WindowId)> {
    let mut application = Application::new();
    let window = application.create_window(
        WindowSpec::new("P7 benchmark")
            .with_inner_size(Size::new(1_000.0, 800.0))
            .with_dpi_scale(DpiScale::ONE),
    )?;
    application.mount_widget(window, declaration(node_count, Declaration::Base))?;
    application.render_window(window)?;
    Ok((application, window))
}

fn sample(node_count: usize, scenario: Scenario) -> Result<Sample> {
    if matches!(scenario, Scenario::InitialRender) {
        let started = Instant::now();
        let mut application = Application::new();
        let window = application.create_window(
            WindowSpec::new("P7 benchmark")
                .with_inner_size(Size::new(1_000.0, 800.0))
                .with_dpi_scale(DpiScale::ONE),
        )?;
        let update_started = Instant::now();
        application.mount_widget(window, declaration(node_count, Declaration::Base))?;
        let update = update_started.elapsed();
        application.render_window(window)?;
        let frame = application
            .frame_metrics(window)
            .expect("rendered window has metrics")
            .clone();
        return Ok(Sample {
            total: started.elapsed(),
            update,
            frame,
            rss_bytes: resident_set_bytes(),
        });
    }

    let (mut application, window) = setup(node_count)?;
    let started = Instant::now();
    let update_started = Instant::now();
    match scenario {
        Scenario::InitialRender | Scenario::Idle => {}
        Scenario::SinglePaint => {
            application.reconcile_widget(window, declaration(node_count, Declaration::Paint(1)))?;
        }
        Scenario::LocalPaint100 => {
            application.reconcile_widget(
                window,
                declaration(node_count, Declaration::Paint(100.min(node_count - 1))),
            )?;
        }
        Scenario::IsolatedLayout => {
            application.reconcile_widget(window, declaration(node_count, Declaration::Layout))?;
        }
        Scenario::KeyedReorder => {
            application
                .reconcile_widget(window, declaration(node_count, Declaration::Reordered))?;
        }
        Scenario::FullLayoutFallback => {
            let report = application
                .reconcile_widget(window, declaration(node_count, Declaration::AmbiguousKeys))?;
            assert!(report.used_safe_fallback());
        }
    }
    let update = update_started.elapsed();
    application.render_window(window)?;
    let frame = application
        .frame_metrics(window)
        .expect("rendered window has metrics")
        .clone();
    Ok(Sample {
        total: started.elapsed(),
        update,
        frame,
        rss_bytes: resident_set_bytes(),
    })
}

fn percentile(values: impl IntoIterator<Item = u64>, percentile: usize) -> u64 {
    let mut values = values.into_iter().collect::<Vec<_>>();
    assert!(!values.is_empty());
    values.sort_unstable();
    let rank = percentile
        .saturating_mul(values.len().saturating_sub(1))
        .saturating_add(99)
        / 100;
    values[rank.min(values.len() - 1)]
}

fn nanos(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

fn p(samples: &[Sample], field: impl Fn(&Sample) -> Duration, rank: usize) -> u64 {
    percentile(samples.iter().map(|sample| nanos(field(sample))), rank)
}

fn emit(node_count: usize, scenario: Scenario, samples: &[Sample]) {
    let representative = &samples[samples.len() / 2].frame;
    let budget = |snapshot: Option<tgui::diagnostics::CacheBudgetSnapshot>| {
        snapshot.map_or((0, 0, 0, 0), |snapshot| {
            (
                snapshot.current_bytes,
                snapshot.peak_bytes,
                snapshot.evictions,
                snapshot.failures.values().copied().sum(),
            )
        })
    };
    let (cpu_current, cpu_peak, cpu_evict, cpu_fail) = budget(representative.cpu_budget.clone());
    let (gpu_current, gpu_peak, gpu_evict, gpu_fail) = budget(representative.gpu_budget.clone());
    let (transient_current, transient_peak, transient_evict, transient_fail) =
        budget(representative.transient_budget.clone());
    let rss = samples.iter().filter_map(|sample| sample.rss_bytes).max();
    let fields = [
        node_count.to_string(),
        scenario.name().to_owned(),
        samples.len().to_string(),
        p(samples, |sample| sample.total, 50).to_string(),
        p(samples, |sample| sample.total, 95).to_string(),
        p(samples, |sample| sample.total, 99).to_string(),
        p(samples, |sample| sample.update, 50).to_string(),
        p(samples, |sample| sample.update, 95).to_string(),
        p(samples, |sample| sample.update, 99).to_string(),
        p(samples, |sample| sample.frame.phases.layout, 50).to_string(),
        p(samples, |sample| sample.frame.phases.layout, 95).to_string(),
        p(samples, |sample| sample.frame.phases.layout, 99).to_string(),
        p(samples, |sample| sample.frame.phases.paint, 50).to_string(),
        p(samples, |sample| sample.frame.phases.paint, 95).to_string(),
        p(samples, |sample| sample.frame.phases.paint, 99).to_string(),
        p(samples, |sample| sample.frame.phases.compile, 50).to_string(),
        p(samples, |sample| sample.frame.phases.compile, 95).to_string(),
        p(samples, |sample| sample.frame.phases.compile, 99).to_string(),
        "na".to_owned(),
        "na".to_owned(),
        "na".to_owned(),
        rss.map_or_else(|| "na".to_owned(), |bytes| bytes.to_string()),
        representative.dirty_elements.to_string(),
        representative.dirty_roots.structure.to_string(),
        representative.dirty_roots.layout.to_string(),
        representative.dirty_roots.paint.to_string(),
        representative.dirty_roots.hit_test.to_string(),
        representative.dirty_roots.semantics.to_string(),
        representative.dirty_roots.resource.to_string(),
        representative.full_rebuilds.to_string(),
        representative.incremental_rebuilds.to_string(),
        representative.allocations.arena_allocations.to_string(),
        representative.allocations.arena_releases.to_string(),
        "na".to_owned(),
        "na".to_owned(),
        representative.arena.live.to_string(),
        representative.arena.slots.to_string(),
        representative.arena.estimated_reserved_bytes.to_string(),
        representative.scene.paint_commands.to_string(),
        representative.scene.render_chunks.to_string(),
        representative.scene.batches.to_string(),
        representative.scene.passes.to_string(),
        representative.scene.chunk_rebuilds.to_string(),
        representative.scene.compiled_cache_hits.to_string(),
        representative.scene.compiled_cache_misses.to_string(),
        representative.scene.gpu_upload_bytes.to_string(),
        representative.scene.transient_vram_bytes.to_string(),
        representative.resources.hits.to_string(),
        representative.resources.misses.to_string(),
        representative.resources.evictions.to_string(),
        representative.resources.upload_bytes.to_string(),
        representative.resources.failures.to_string(),
        representative.resources.in_flight_references.to_string(),
        cpu_current.to_string(),
        cpu_peak.to_string(),
        cpu_evict.to_string(),
        cpu_fail.to_string(),
        gpu_current.to_string(),
        gpu_peak.to_string(),
        gpu_evict.to_string(),
        gpu_fail.to_string(),
        transient_current.to_string(),
        transient_peak.to_string(),
        transient_evict.to_string(),
        transient_fail.to_string(),
    ];
    println!("{}", fields.join(","));
}

fn resident_set_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        let kib = status
            .lines()
            .find_map(|line| line.strip_prefix("VmRSS:"))?
            .split_whitespace()
            .next()?
            .parse::<u64>()
            .ok()?;
        return kib.checked_mul(1_024);
    }
    #[cfg(not(target_os = "linux"))]
    {
        let output = Command::new("ps")
            .args(["-o", "rss=", "-p", &std::process::id().to_string()])
            .output()
            .ok()?;
        let kib = String::from_utf8(output.stdout)
            .ok()?
            .trim()
            .parse::<u64>()
            .ok()?;
        kib.checked_mul(1_024)
    }
}

fn command_output(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map_or_else(|| "unknown".to_owned(), |output| output.trim().to_owned())
}

fn samples() -> usize {
    env::var("TGUI_BENCH_SAMPLES")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|samples| *samples > 0)
        .unwrap_or(DEFAULT_SAMPLES)
}

fn node_counts() -> Vec<usize> {
    let Some(configured) = env::var("TGUI_BENCH_NODE_COUNTS").ok() else {
        return NODE_COUNTS.to_vec();
    };
    let parsed = configured
        .split(',')
        .filter_map(|value| value.trim().parse::<usize>().ok())
        .filter(|count| *count >= 3)
        .collect::<Vec<_>>();
    if parsed.is_empty() {
        NODE_COUNTS.to_vec()
    } else {
        parsed
    }
}

fn main() -> Result<()> {
    let sample_count = samples();
    println!("# schema=tgui-p7-matrix-v1");
    println!("# commit={}", command_output("git", &["rev-parse", "HEAD"]));
    println!("# rustc={}", command_output("rustc", &["--version"]));
    println!(
        "# platform={}-{} device={} theme=fixed-light font=system-fallback dpi=1 window=1000x800 resources=none samples={sample_count}",
        env::consts::OS,
        env::consts::ARCH,
        env::var("TGUI_BENCH_DEVICE").unwrap_or_else(|_| "headless-cpu".to_owned()),
    );
    println!(
        "# unavailable=submit_p50_ns,submit_p95_ns,submit_p99_ns,heap_allocations,allocated_bytes,gpu_time_ns,driver_vram_bytes"
    );
    println!(
        "nodes,scenario,samples,total_p50_ns,total_p95_ns,total_p99_ns,update_p50_ns,update_p95_ns,update_p99_ns,layout_p50_ns,layout_p95_ns,layout_p99_ns,paint_p50_ns,paint_p95_ns,paint_p99_ns,compile_p50_ns,compile_p95_ns,compile_p99_ns,submit_p50_ns,submit_p95_ns,submit_p99_ns,rss_peak_bytes,dirty_elements,dirty_structure_roots,dirty_layout_roots,dirty_paint_roots,dirty_hit_roots,dirty_semantics_roots,dirty_resource_roots,full_rebuilds,incremental_rebuilds,arena_allocations,arena_releases,heap_allocations,allocated_bytes,arena_live,arena_slots,arena_reserved_bytes,paint_commands,render_chunks,batches,passes,chunk_rebuilds,compiled_cache_hits,compiled_cache_misses,gpu_upload_bytes,transient_vram_bytes,resource_hits,resource_misses,resource_evictions,resource_upload_bytes,resource_failures,in_flight_references,cpu_current_bytes,cpu_peak_bytes,cpu_evictions,cpu_failures,gpu_current_bytes,gpu_peak_bytes,gpu_evictions,gpu_failures,transient_current_bytes,transient_peak_bytes,transient_evictions,transient_failures"
    );
    for node_count in node_counts() {
        for scenario in Scenario::ALL {
            let mut results = Vec::with_capacity(sample_count);
            for _ in 0..sample_count {
                results.push(sample(node_count, scenario)?);
            }
            black_box(&results);
            emit(node_count, scenario, &results);
        }
    }
    Ok(())
}
