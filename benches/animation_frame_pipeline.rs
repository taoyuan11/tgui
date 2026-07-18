use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

#[cfg(feature = "bench-support")]
use std::hint::black_box;
#[cfg(feature = "bench-support")]
use std::time::{Duration, Instant};
#[cfg(feature = "bench-support")]
use tgui::animation::{PlaybackDirection, Transition};
#[cfg(feature = "bench-support")]
use tgui::canvas::{Canvas, CanvasRecorder, CanvasShadow};
#[cfg(feature = "bench-support")]
use tgui::core::{dp, Color, Point, Rect};
#[cfg(feature = "bench-support")]
use tgui::layout::{Axis, Insets, Overflow};
#[cfg(feature = "bench-support")]
use tgui::mvvm::{State, ViewModelContext};
#[cfg(feature = "bench-support")]
use tgui::widgets::bench_support_ext::BenchTransformTranslatePrepareProbe;
#[cfg(feature = "bench-support")]
use tgui::widgets::{
    BackgroundBrush, BackgroundGradientStop, BackgroundLinearGradient, BackgroundRadialGradient,
    Flex, Icon, ProgressBar, Slider, Text, WidgetBenchmarkContext, WidgetTree,
};

#[cfg(feature = "bench-support")]
const MONOCHROME_ANIMATION_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 18 18"><path fill="#000" d="M2 2h14v14H2z"/></svg>"##;

#[cfg(feature = "bench-support")]
#[derive(Clone, Copy, Debug)]
enum AnimatedProperty {
    Opacity,
    Color,
    Offset,
    Scale,
    LayoutWidth,
}

#[cfg(feature = "bench-support")]
impl AnimatedProperty {
    const ALL: [Self; 5] = [
        Self::Opacity,
        Self::Color,
        Self::Offset,
        Self::Scale,
        Self::LayoutWidth,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Opacity => "opacity",
            Self::Color => "color",
            Self::Offset => "offset",
            Self::Scale => "scale",
            Self::LayoutWidth => "layout_width_candidate",
        }
    }

    fn affects_layout(self) -> bool {
        matches!(self, Self::LayoutWidth)
    }
}

#[cfg(feature = "bench-support")]
enum AnimationHandles {
    Float(Vec<State<f32>>, f32),
    Color(Vec<State<Color>>, Color),
    Point(Vec<State<Point>>, Point),
    Dp(Vec<State<tgui::core::Dp>>, tgui::core::Dp),
}

#[cfg(feature = "bench-support")]
impl AnimationHandles {
    fn set_targets(&self) {
        match self {
            Self::Float(states, target) => {
                for state in states {
                    state.set(*target);
                }
            }
            Self::Color(states, target) => {
                for state in states {
                    state.set(*target);
                }
            }
            Self::Point(states, target) => {
                for state in states {
                    state.set(*target);
                }
            }
            Self::Dp(states, target) => {
                for state in states {
                    state.set(*target);
                }
            }
        }
    }
}

#[cfg(feature = "bench-support")]
struct AnimationFrameFixture {
    tree: Box<WidgetTree<()>>,
    context: WidgetBenchmarkContext,
    next_frame: Instant,
    frame_interval: Duration,
    active_count: usize,
    property: AnimatedProperty,
    frame_index: u64,
    full_layout_rebuild: bool,
}

#[cfg(feature = "bench-support")]
#[derive(Clone, Copy)]
struct AnimationFrameSample {
    total: Duration,
    reactive_resolve: Duration,
}

#[cfg(feature = "bench-support")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MaskTintMode {
    Retained,
    FullRecollect,
}

#[cfg(feature = "bench-support")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MaskTintResolveMode {
    Direct,
    LegacyFullVisual,
}

#[cfg(feature = "bench-support")]
struct CpuMaskTintFrameFixture {
    tree: Box<WidgetTree<()>>,
    context: WidgetBenchmarkContext,
    next_frame: Instant,
    frame_interval: Duration,
    active_count: usize,
    mode: MaskTintResolveMode,
}

#[cfg(feature = "bench-support")]
struct ShadowOpacityFrameFixture {
    tree: Box<WidgetTree<()>>,
    context: WidgetBenchmarkContext,
    next_frame: Instant,
    frame_interval: Duration,
    active_count: usize,
    legacy: bool,
    canonical_texture_ids: Vec<u64>,
}

#[cfg(feature = "bench-support")]
struct CanvasShadowOpacityFrameFixture {
    tree: Box<WidgetTree<()>>,
    context: WidgetBenchmarkContext,
    next_frame: Instant,
    frame_interval: Duration,
    active_count: usize,
    legacy: bool,
    canonical_texture_ids: Vec<u64>,
}

#[cfg(feature = "bench-support")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TextColorResolveMode {
    Direct,
    LegacyFullVisual,
}

#[cfg(feature = "bench-support")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TextOpacityResolveMode {
    Direct,
    LegacyFullVisual,
}

#[cfg(feature = "bench-support")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ContainerOpacityResolveMode {
    Direct,
    LegacyFullVisual,
}

#[cfg(feature = "bench-support")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BackgroundResolveMode {
    Direct,
    LegacyFullVisual,
}

#[cfg(feature = "bench-support")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BackgroundBlurResolveMode {
    Direct,
    LegacyFullVisual,
}

#[cfg(feature = "bench-support")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BackgroundBrushResolveMode {
    Direct,
    LegacyFullVisual,
}

#[cfg(feature = "bench-support")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OffsetResolveMode {
    Direct,
    LegacyFullVisual,
}

#[cfg(feature = "bench-support")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScaleResolveMode {
    Direct,
    LegacyFullVisual,
}

#[cfg(feature = "bench-support")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BorderColorResolveMode {
    Direct,
    LegacyFullVisual,
}

#[cfg(feature = "bench-support")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BorderRadiusResolveMode {
    Direct,
    LegacyFullVisual,
}

#[cfg(feature = "bench-support")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BorderWidthResolveMode {
    Direct,
    LegacyFullVisual,
}

#[cfg(feature = "bench-support")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProgressValueResolveMode {
    Direct,
    LegacyFullVisual,
}

#[cfg(feature = "bench-support")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SliderValueResolveMode {
    Direct,
    LegacyFullVisual,
}

#[cfg(feature = "bench-support")]
struct CpuTextColorFrameFixture {
    tree: Box<WidgetTree<()>>,
    context: WidgetBenchmarkContext,
    next_frame: Instant,
    frame_interval: Duration,
    active_count: usize,
    mode: TextColorResolveMode,
}

#[cfg(feature = "bench-support")]
struct CpuTextOpacityFrameFixture {
    tree: Box<WidgetTree<()>>,
    context: WidgetBenchmarkContext,
    next_frame: Instant,
    frame_interval: Duration,
    active_count: usize,
    mode: TextOpacityResolveMode,
    complex_surface: bool,
}

#[cfg(feature = "bench-support")]
struct CpuContainerOpacityFrameFixture {
    tree: Box<WidgetTree<()>>,
    context: WidgetBenchmarkContext,
    next_frame: Instant,
    frame_interval: Duration,
    active_count: usize,
    mode: ContainerOpacityResolveMode,
}

#[cfg(feature = "bench-support")]
struct CpuBackgroundFrameFixture {
    tree: Box<WidgetTree<()>>,
    context: WidgetBenchmarkContext,
    next_frame: Instant,
    frame_interval: Duration,
    active_count: usize,
    mode: BackgroundResolveMode,
}

#[cfg(feature = "bench-support")]
struct CpuBackgroundBlurFrameFixture {
    tree: Box<WidgetTree<()>>,
    context: WidgetBenchmarkContext,
    next_frame: Instant,
    frame_interval: Duration,
    active_count: usize,
    mode: BackgroundBlurResolveMode,
}

#[cfg(feature = "bench-support")]
struct CpuBackgroundBrushFrameFixture {
    tree: Box<WidgetTree<()>>,
    context: WidgetBenchmarkContext,
    next_frame: Instant,
    frame_interval: Duration,
    active_count: usize,
    mode: BackgroundBrushResolveMode,
}

#[cfg(feature = "bench-support")]
struct CpuOffsetFrameFixture {
    tree: Box<WidgetTree<()>>,
    context: WidgetBenchmarkContext,
    next_frame: Instant,
    frame_interval: Duration,
    active_count: usize,
    mode: OffsetResolveMode,
}

#[cfg(feature = "bench-support")]
struct CpuScaleFrameFixture {
    tree: Box<WidgetTree<()>>,
    context: WidgetBenchmarkContext,
    next_frame: Instant,
    frame_interval: Duration,
    active_count: usize,
    mode: ScaleResolveMode,
    complex_surface: bool,
}

#[cfg(feature = "bench-support")]
struct CpuBorderColorFrameFixture {
    tree: Box<WidgetTree<()>>,
    context: WidgetBenchmarkContext,
    next_frame: Instant,
    frame_interval: Duration,
    active_count: usize,
    mode: BorderColorResolveMode,
}

#[cfg(feature = "bench-support")]
struct CpuBorderRadiusFrameFixture {
    tree: Box<WidgetTree<()>>,
    context: WidgetBenchmarkContext,
    next_frame: Instant,
    frame_interval: Duration,
    active_count: usize,
    mode: BorderRadiusResolveMode,
}

#[cfg(feature = "bench-support")]
struct CpuBorderWidthFrameFixture {
    tree: Box<WidgetTree<()>>,
    context: WidgetBenchmarkContext,
    next_frame: Instant,
    frame_interval: Duration,
    active_count: usize,
    mode: BorderWidthResolveMode,
    complex_surface: bool,
}

#[cfg(feature = "bench-support")]
struct CpuProgressValueFrameFixture {
    tree: Box<WidgetTree<()>>,
    context: WidgetBenchmarkContext,
    next_frame: Instant,
    frame_interval: Duration,
    active_count: usize,
    mode: ProgressValueResolveMode,
}

#[cfg(feature = "bench-support")]
struct CpuSliderValueFrameFixture {
    tree: Box<WidgetTree<()>>,
    context: WidgetBenchmarkContext,
    next_frame: Instant,
    frame_interval: Duration,
    active_count: usize,
    mode: SliderValueResolveMode,
    complex_surface: bool,
}

#[cfg(feature = "bench-support")]
struct MaskTintFrameFixture {
    tree: Box<WidgetTree<()>>,
    context: WidgetBenchmarkContext,
    next_frame: Instant,
    frame_interval: Duration,
    active_count: usize,
    mode: MaskTintMode,
}

#[cfg(feature = "bench-support")]
impl MaskTintFrameFixture {
    fn render_frame(&mut self) -> Duration {
        let started = Instant::now();
        self.context
            .render_cached_scene_to_headless_gpu(&self.tree, self.next_frame)
            .expect("mask-tint animation GPU submit");
        let elapsed = started.elapsed();
        let activity = self.context.animation_frame_activity();
        assert!(activity.refresh_changed);
        assert_eq!(activity.scene_widget_count, self.active_count);
        assert_eq!(activity.layout_widget_count, 0);
        assert!(!activity.fell_back);
        match self.mode {
            MaskTintMode::Retained => {
                assert!(activity.scene_patch_succeeded);
                assert!(activity.reactive_slot_write_succeeded);
                assert!(!activity.full_scene_recollect);
            }
            MaskTintMode::FullRecollect => {
                assert!(activity.full_scene_recollect);
                assert!(!activity.reactive_slot_write_succeeded);
            }
        }
        self.next_frame += self.frame_interval;
        elapsed
    }
}

#[cfg(feature = "bench-support")]
impl CpuMaskTintFrameFixture {
    fn render_frame(&mut self) -> Duration {
        let started = Instant::now();
        let stats = self
            .context
            .run_layout_and_scene(&self.tree, self.next_frame);
        let elapsed = started.elapsed();
        assert_eq!(stats.texture_count, 1000);
        let activity = self.context.animation_frame_activity();
        assert!(activity.refresh_changed);
        assert_eq!(activity.scene_widget_count, self.active_count);
        assert_eq!(activity.layout_widget_count, 0);
        assert!(activity.scene_patch_succeeded);
        assert!(activity.reactive_slot_write_succeeded);
        assert!(!activity.full_scene_recollect);
        assert!(!activity.fell_back);
        self.next_frame += self.frame_interval;
        elapsed
    }
}

#[cfg(feature = "bench-support")]
impl ShadowOpacityFrameFixture {
    fn render_frame(&mut self) -> Duration {
        let started = Instant::now();
        let stats = self
            .context
            .run_layout_and_scene(&self.tree, self.next_frame);
        let elapsed = started.elapsed();
        assert_eq!(stats.texture_count, self.active_count);
        let activity = self.context.animation_frame_activity();
        assert!(activity.refresh_changed);
        assert_eq!(activity.scene_widget_count, self.active_count);
        assert!(activity.scene_patch_succeeded);
        assert!(!activity.fell_back);
        assert_eq!(activity.reactive_slot_write_succeeded, !self.legacy);
        let retained = self
            .context
            .cached_texture_retained_state()
            .expect("shadow opacity retained state");
        if !self.legacy {
            assert_eq!(retained.texture_ids, self.canonical_texture_ids);
        }
        assert_eq!(retained.opacities.len(), self.active_count);
        self.next_frame += self.frame_interval;
        elapsed
    }
}

#[cfg(feature = "bench-support")]
impl CanvasShadowOpacityFrameFixture {
    fn render_frame(&mut self) -> Duration {
        let started = Instant::now();
        let stats = self
            .context
            .run_layout_and_scene(&self.tree, self.next_frame);
        let elapsed = started.elapsed();
        assert_eq!(stats.texture_count, self.active_count);
        let activity = self.context.animation_frame_activity();
        assert!(activity.refresh_changed);
        assert_eq!(activity.scene_widget_count, self.active_count);
        assert!(activity.scene_patch_succeeded);
        assert!(!activity.fell_back);
        assert!(!activity.reactive_slot_write_succeeded);
        let retained = self
            .context
            .cached_texture_retained_state()
            .expect("canvas shadow opacity retained state");
        if !self.legacy {
            assert_eq!(retained.texture_ids, self.canonical_texture_ids);
        }
        assert_eq!(retained.opacities.len(), self.active_count);
        self.next_frame += self.frame_interval;
        elapsed
    }
}

#[cfg(feature = "bench-support")]
impl CpuTextColorFrameFixture {
    fn render_frame(&mut self) -> (Duration, Vec<Option<Color>>) {
        let started = Instant::now();
        let values = self.context.resolve_all_text_colors_for_benchmark(
            &self.tree,
            self.next_frame,
            self.mode == TextColorResolveMode::LegacyFullVisual,
        );
        let elapsed = started.elapsed();
        assert_eq!(values.len(), self.active_count);
        assert!(values.iter().all(Option::is_some));
        self.next_frame += self.frame_interval;
        (elapsed, values)
    }
}

#[cfg(feature = "bench-support")]
impl CpuTextOpacityFrameFixture {
    fn render_frame(&mut self) -> (Duration, Vec<Option<Color>>) {
        let started = Instant::now();
        let values = self.context.resolve_all_plain_text_opacities_for_benchmark(
            &self.tree,
            self.next_frame,
            self.mode == TextOpacityResolveMode::LegacyFullVisual,
        );
        let elapsed = started.elapsed();
        assert_eq!(values.len(), self.active_count);
        if self.complex_surface {
            assert!(values.iter().all(Option::is_none));
        } else {
            assert!(values.iter().all(Option::is_some));
        }
        self.next_frame += self.frame_interval;
        (elapsed, values)
    }
}

#[cfg(feature = "bench-support")]
impl CpuContainerOpacityFrameFixture {
    fn render_frame(&mut self) -> (Duration, Vec<Option<(Color, Option<Color>, Option<bool>)>>) {
        let started = Instant::now();
        let values = self
            .context
            .resolve_all_plain_container_opacities_for_benchmark(
                &self.tree,
                self.next_frame,
                self.mode == ContainerOpacityResolveMode::LegacyFullVisual,
            );
        let elapsed = started.elapsed();
        assert_eq!(values.len(), self.active_count);
        assert!(values.iter().all(Option::is_some));
        self.next_frame += self.frame_interval;
        (elapsed, values)
    }
}

#[cfg(feature = "bench-support")]
impl CpuBackgroundFrameFixture {
    fn render_frame(&mut self) -> (Duration, Vec<Option<(Rect, Color)>>) {
        let started = Instant::now();
        let values = self
            .context
            .resolve_all_plain_container_backgrounds_for_benchmark(
                &self.tree,
                self.next_frame,
                self.mode == BackgroundResolveMode::LegacyFullVisual,
            );
        let elapsed = started.elapsed();
        assert_eq!(values.len(), self.active_count);
        assert!(values.iter().all(Option::is_some));
        self.next_frame += self.frame_interval;
        (elapsed, values)
    }
}

#[cfg(feature = "bench-support")]
impl CpuBackgroundBlurFrameFixture {
    fn render_frame(&mut self) -> Duration {
        let started = Instant::now();
        let values = self
            .context
            .resolve_all_plain_container_background_blurs_for_benchmark(
                &self.tree,
                self.next_frame,
                self.mode == BackgroundBlurResolveMode::LegacyFullVisual,
            );
        let elapsed = started.elapsed();
        assert_eq!(values.len(), self.active_count);
        assert!(values.iter().all(Option::is_some));
        black_box(values);
        self.next_frame += self.frame_interval;
        elapsed
    }
}

#[cfg(feature = "bench-support")]
impl CpuBackgroundBrushFrameFixture {
    fn render_frame(&mut self) -> Duration {
        let started = Instant::now();
        let values = self
            .context
            .resolve_all_plain_container_background_brushes_for_benchmark(
                &self.tree,
                self.next_frame,
                self.mode == BackgroundBrushResolveMode::LegacyFullVisual,
            );
        let elapsed = started.elapsed();
        assert_eq!(values.len(), self.active_count);
        assert!(values.iter().all(Option::is_some));
        black_box(values);
        self.next_frame += self.frame_interval;
        elapsed
    }
}

#[cfg(feature = "bench-support")]
impl CpuOffsetFrameFixture {
    fn render_frame(&mut self) -> Duration {
        let started = Instant::now();
        let values = self
            .context
            .resolve_all_plain_container_offsets_for_benchmark(
                &self.tree,
                self.next_frame,
                self.mode == OffsetResolveMode::LegacyFullVisual,
            );
        let elapsed = started.elapsed();
        assert_eq!(values.len(), self.active_count);
        assert!(values.iter().all(Option::is_some));
        black_box(values);
        self.next_frame += self.frame_interval;
        elapsed
    }
}

#[cfg(feature = "bench-support")]
impl CpuScaleFrameFixture {
    fn render_frame(
        &mut self,
    ) -> (
        Duration,
        Vec<Option<tgui::widgets::ContainerScaleResolveSnapshot>>,
    ) {
        let started = Instant::now();
        let values = self
            .context
            .resolve_all_plain_container_scales_for_benchmark(
                &self.tree,
                self.next_frame,
                self.mode == ScaleResolveMode::LegacyFullVisual,
            );
        let elapsed = started.elapsed();
        assert_eq!(values.len(), self.active_count);
        if self.complex_surface {
            for (index, value) in values.iter().enumerate() {
                assert_eq!(
                    value.is_some(),
                    index % 2 == 1,
                    "shadow/brush fallback topology changed at target {index}"
                );
            }
        } else {
            assert!(values.iter().all(Option::is_some));
        }
        self.next_frame += self.frame_interval;
        (elapsed, values)
    }
}

#[cfg(feature = "bench-support")]
impl CpuBorderColorFrameFixture {
    fn render_frame(&mut self) -> (Duration, Vec<Option<(Rect, f32, Color)>>) {
        let started = Instant::now();
        let values = self
            .context
            .resolve_all_plain_container_border_colors_for_benchmark(
                &self.tree,
                self.next_frame,
                self.mode == BorderColorResolveMode::LegacyFullVisual,
            );
        let elapsed = started.elapsed();
        assert_eq!(values.len(), self.active_count);
        assert!(values.iter().all(Option::is_some));
        self.next_frame += self.frame_interval;
        (elapsed, values)
    }
}

#[cfg(feature = "bench-support")]
impl CpuBorderRadiusFrameFixture {
    fn render_frame(
        &mut self,
    ) -> (
        Duration,
        Vec<Option<(Option<(Rect, f32)>, Option<(Rect, f32)>)>>,
    ) {
        let started = Instant::now();
        let values = self
            .context
            .resolve_all_plain_container_border_radii_for_benchmark(
                &self.tree,
                self.next_frame,
                self.mode == BorderRadiusResolveMode::LegacyFullVisual,
            );
        let elapsed = started.elapsed();
        assert_eq!(values.len(), self.active_count);
        assert!(values.iter().all(Option::is_some));
        self.next_frame += self.frame_interval;
        (elapsed, values)
    }
}

#[cfg(feature = "bench-support")]
impl CpuBorderWidthFrameFixture {
    fn render_frame(
        &mut self,
    ) -> (
        Duration,
        Vec<Option<(Option<(Rect, f32)>, Option<(Rect, f32)>)>>,
    ) {
        let started = Instant::now();
        let values = self
            .context
            .resolve_all_plain_container_border_widths_for_benchmark(
                &self.tree,
                self.next_frame,
                self.mode == BorderWidthResolveMode::LegacyFullVisual,
            );
        let elapsed = started.elapsed();
        assert_eq!(values.len(), self.active_count);
        if self.complex_surface {
            assert!(values.iter().all(Option::is_none));
        } else {
            assert!(values.iter().all(Option::is_some));
        }
        self.next_frame += self.frame_interval;
        (elapsed, values)
    }
}

#[cfg(feature = "bench-support")]
impl CpuProgressValueFrameFixture {
    fn render_frame(
        &mut self,
    ) -> (
        Duration,
        Vec<Option<tgui::widgets::ProgressValueResolveSnapshot>>,
    ) {
        let started = Instant::now();
        let values = self.context.resolve_all_progress_values_for_benchmark(
            &self.tree,
            self.next_frame,
            self.mode == ProgressValueResolveMode::LegacyFullVisual,
        );
        let elapsed = started.elapsed();
        assert_eq!(values.len(), self.active_count);
        assert!(values.iter().all(Option::is_some));
        self.next_frame += self.frame_interval;
        (elapsed, values)
    }
}

#[cfg(feature = "bench-support")]
impl CpuSliderValueFrameFixture {
    fn render_frame(
        &mut self,
    ) -> (
        Duration,
        Vec<Option<tgui::widgets::SliderValueResolveSnapshot>>,
    ) {
        let started = Instant::now();
        let values = self.context.resolve_all_slider_values_for_benchmark(
            &self.tree,
            self.next_frame,
            self.mode == SliderValueResolveMode::LegacyFullVisual,
        );
        let elapsed = started.elapsed();
        assert_eq!(values.len(), self.active_count);
        if self.complex_surface {
            assert!(values.iter().all(Option::is_none));
        } else {
            assert!(values.iter().all(Option::is_some));
        }
        self.next_frame += self.frame_interval;
        (elapsed, values)
    }
}

#[cfg(feature = "bench-support")]
impl AnimationFrameFixture {
    fn render_frame(&mut self) -> AnimationFrameSample {
        let started = Instant::now();
        self.context
            .render_cached_scene_to_headless_gpu(&self.tree, self.next_frame)
            .expect("animation frame GPU submit");
        let elapsed = started.elapsed();
        let activity = self.context.animation_frame_activity();
        assert!(
            activity.refresh_changed,
            "animation unexpectedly settled: activity={activity:?} active_engine={}",
            self.context.has_active_property_animations(),
        );
        assert!(!activity.fell_back, "animation frame used fallback rebuild");
        if self.property.affects_layout() {
            assert_eq!(
                activity.layout_widget_count, self.active_count,
                "layout activity mismatch at frame {}",
                self.frame_index,
            );
            if self.full_layout_rebuild {
                assert!(activity.full_layout_rebuild);
                assert!(!activity.layout_patch_succeeded);
            } else {
                assert!(activity.layout_patch_succeeded);
                assert!(!activity.full_layout_rebuild);
            }
            let prepare = self
                .context
                .headless_gpu_prepare_stats()
                .expect("headless prepare stats");
            assert!(
                prepare.rebuilt_commands > 0,
                "layout animation reused every prepared draw after geometry changed"
            );
        } else {
            assert_eq!(
                activity.scene_widget_count, self.active_count,
                "scene activity mismatch at frame {}",
                self.frame_index,
            );
            assert!(activity.scene_patch_succeeded);
            assert!(
                activity.reactive_slot_write_succeeded,
                "scene animation missed retained property slot at frame {}",
                self.frame_index,
            );
        }
        self.next_frame += self.frame_interval;
        self.frame_index += 1;
        AnimationFrameSample {
            total: elapsed,
            reactive_resolve: activity.reactive_slot_resolve_duration,
        }
    }
}

#[cfg(feature = "bench-support")]
fn transition() -> Transition {
    Transition::linear(Duration::from_millis(480))
        .repeat_forever()
        .direction(PlaybackDirection::Alternate)
}

#[cfg(feature = "bench-support")]
fn build_fixture(
    property: AnimatedProperty,
    active_count: usize,
    hz: u32,
) -> Option<AnimationFrameFixture> {
    const TOTAL_CELLS: usize = 1000;
    const COLUMNS: usize = 40;

    let view_model = ViewModelContext::for_benchmarks();
    let transition = transition();
    let mut float_states = Vec::new();
    let mut color_states = Vec::new();
    let mut point_states = Vec::new();
    let mut dp_states = Vec::new();
    let base_color = Color::rgba(34, 211, 238, 230);
    let target_color = Color::rgba(139, 92, 246, 230);

    let mut body = Flex::<()>::new(Axis::Vertical)
        .width(dp(1120.0))
        .gap(dp(1.0));
    for row in 0..(TOTAL_CELLS / COLUMNS) {
        let mut line = Flex::<()>::new(Axis::Horizontal)
            .width(dp(920.0))
            .height(dp(25.0))
            .gap(dp(1.0));
        for column in 0..COLUMNS {
            let index = row * COLUMNS + column;
            let cell = Flex::<()>::new(Axis::Horizontal)
                .width(dp(22.0))
                .height(dp(24.0))
                .style(move |style, _| {
                    style.surface.background = Some(base_color.into());
                    style.surface.border_radius = Some(
                        if matches!(property, AnimatedProperty::Scale) {
                            dp(0.0)
                        } else {
                            dp(5.0)
                        }
                        .into(),
                    );
                });

            let cell = if index >= active_count {
                cell
            } else {
                match property {
                    AnimatedProperty::Opacity => {
                        let state = view_model.state(0.35_f32);
                        let signal = state.signal().animated(transition);
                        float_states.push(state);
                        cell.opacity(signal)
                    }
                    AnimatedProperty::Color => {
                        let state = view_model.state(base_color);
                        let signal = state.signal().animated(transition);
                        color_states.push(state);
                        cell.style(move |style, _| {
                            style.surface.background = Some(signal.clone().into());
                        })
                    }
                    AnimatedProperty::Offset => {
                        let state = view_model.state(Point::new(dp(-2.0), dp(0.0)));
                        let signal = state.signal().animated(transition);
                        point_states.push(state);
                        cell.offset(signal)
                    }
                    AnimatedProperty::Scale => {
                        let state = view_model.state(0.92_f32);
                        let signal = state.signal().animated(transition);
                        float_states.push(state);
                        cell.scale(signal)
                    }
                    AnimatedProperty::LayoutWidth => {
                        let state = view_model.state(dp(18.0));
                        let signal = state.signal().animated(transition);
                        dp_states.push(state);
                        cell.width(signal)
                    }
                }
            };
            line = line.child(cell);
        }
        body = body.child(line);
    }

    // Benchmark cache identity is pointer-based; box the tree so moving the fixture cannot turn
    // the first timed frame into an accidental full rebuild.
    let tree = Box::new(WidgetTree::new(
        Flex::<()>::new(Axis::Vertical)
            .size(dp(1280.0), dp(720.0))
            .padding(Insets::all(dp(12.0)))
            .child(body),
    ));
    let handles = match property {
        AnimatedProperty::Opacity => AnimationHandles::Float(float_states, 1.0),
        AnimatedProperty::Color => AnimationHandles::Color(color_states, target_color),
        AnimatedProperty::Offset => {
            AnimationHandles::Point(point_states, Point::new(dp(2.0), dp(0.0)))
        }
        AnimatedProperty::Scale => AnimationHandles::Float(float_states, 1.0),
        AnimatedProperty::LayoutWidth => AnimationHandles::Dp(dp_states, dp(22.0)),
    };

    let viewport = Rect::new(0.0, 0.0, 1280.0, 720.0);
    let mut context = WidgetBenchmarkContext::new().with_viewport(viewport);
    context.set_force_individual_reactive_resolve(
        std::env::var("TGUI_ANIMATION_REACTIVE_RESOLVE_MODE").as_deref() == Ok("individual"),
    );
    let start = Instant::now();
    let _ = context.run_layout_and_scene(&tree, start);
    match context.initialize_headless_gpu() {
        Ok(adapter) => eprintln!(
            "animation_frame_pipeline: adapter='{}' backend={} cadence={}Hz active={} property={}",
            adapter.name,
            adapter.backend,
            hz,
            active_count,
            property.name(),
        ),
        Err(error) => {
            eprintln!("Skipping animation frame benchmark: {error}");
            return None;
        }
    }
    context.set_headless_gpu_cache_liveness_legacy_dirty_gate(
        std::env::var("TGUI_ANIMATION_CACHE_LIVENESS_MODE").as_deref() == Ok("legacy"),
    );
    context
        .render_cached_scene_to_headless_gpu(&tree, start)
        .expect("stable animation fixture warmup");

    handles.set_targets();
    let animation_start = start + Duration::from_millis(1);
    // Resolve the new signal targets once outside the timed path. Keeping the same AnimationEngine
    // preserves the settled `from` slots and starts real transitions; subsequent frames use only
    // refresh + retained root patching.
    context.invalidate_all();
    let _ = context.run_layout_and_scene(&tree, animation_start);
    assert!(
        context.has_active_property_animations(),
        "fixture failed to start property animations"
    );

    let frame_interval = Duration::from_secs_f64(1.0 / hz as f64);
    // Keep the benchmark's production candidate aligned with the runtime's conservative dense
    // fallback while still allowing an explicit retained/full A/B control.
    let layout_mode = std::env::var("TGUI_ANIMATION_LAYOUT_MODE").ok();
    let layout_widget_count = context.cached_layout_widget_count();
    let dense_layout_refresh = active_count >= 512
        && active_count.saturating_mul(16) >= layout_widget_count.saturating_mul(15);
    let full_layout_rebuild = property.affects_layout()
        && (layout_mode.as_deref() == Some("full")
            || (layout_mode.as_deref() != Some("retained") && dense_layout_refresh));
    context.set_force_full_layout_animation_rebuild(full_layout_rebuild);
    assert!(context.reset_headless_gpu_cache_liveness_stats());
    Some(AnimationFrameFixture {
        tree,
        context,
        next_frame: animation_start + frame_interval,
        frame_interval,
        active_count,
        property,
        frame_index: 1,
        full_layout_rebuild,
    })
}

#[cfg(feature = "bench-support")]
fn build_mask_tint_tree(active_count: usize) -> (Box<WidgetTree<()>>, Vec<State<Color>>) {
    const TOTAL_ICONS: usize = 1000;
    const COLUMNS: usize = 40;

    let view_model = ViewModelContext::for_benchmarks();
    let transition = transition();
    let base_color = Color::rgba(14, 165, 233, 224);
    let mut states = Vec::with_capacity(active_count);
    let mut body = Flex::<()>::new(Axis::Vertical)
        .width(dp(840.0))
        .gap(dp(2.0));
    for row in 0..(TOTAL_ICONS / COLUMNS) {
        let mut line = Flex::<()>::new(Axis::Horizontal)
            .width(dp(840.0))
            .height(dp(20.0))
            .gap(dp(2.0));
        for column in 0..COLUMNS {
            let index = row * COLUMNS + column;
            let icon = Icon::<()>::monochrome_svg(MONOCHROME_ANIMATION_SVG).size(dp(18.0));
            let icon = if index < active_count {
                let state = view_model.state(base_color);
                let signal = state.signal().animated(transition);
                states.push(state);
                icon.style(move |style, _| style.color = signal.clone().into())
            } else {
                icon.style(move |style, _| style.color = base_color.into())
            };
            line = line.child(icon);
        }
        body = body.child(line);
    }

    let tree = Box::new(WidgetTree::new(
        Flex::<()>::new(Axis::Vertical)
            .size(dp(960.0), dp(600.0))
            .padding(Insets::all(dp(12.0)))
            .child(body),
    ));
    (tree, states)
}

#[cfg(feature = "bench-support")]
fn build_mask_tint_fixture(
    active_count: usize,
    hz: u32,
    mode: MaskTintMode,
) -> Option<MaskTintFrameFixture> {
    const TOTAL_ICONS: usize = 1000;

    let (tree, states) = build_mask_tint_tree(active_count);
    let viewport = Rect::new(0.0, 0.0, 960.0, 600.0);
    let mut context = WidgetBenchmarkContext::new().with_viewport(viewport);
    let start = Instant::now();
    let stats = context.run_layout_and_scene(&tree, start);
    assert_eq!(stats.texture_count, TOTAL_ICONS);
    match context.initialize_headless_gpu() {
        Ok(adapter) => eprintln!(
            "texture_mask_tint_animation: adapter='{}' backend={} cadence={}Hz active={} mode={mode:?}",
            adapter.name, adapter.backend, hz, active_count,
        ),
        Err(error) => {
            eprintln!("Skipping texture mask-tint benchmark: {error}");
            return None;
        }
    }
    context
        .render_cached_scene_to_headless_gpu(&tree, start)
        .expect("stable mask-tint fixture warmup");

    for state in &states {
        state.set(Color::rgba(244, 63, 94, 176));
    }
    let animation_start = start + Duration::from_millis(1);
    context.invalidate_all();
    let stats = context.run_layout_and_scene(&tree, animation_start);
    assert_eq!(stats.texture_count, TOTAL_ICONS);
    assert!(context.has_active_property_animations());
    context.set_force_full_scene_animation_recollect(mode == MaskTintMode::FullRecollect);

    Some(MaskTintFrameFixture {
        tree,
        context,
        next_frame: animation_start + Duration::from_secs_f64(1.0 / hz as f64),
        frame_interval: Duration::from_secs_f64(1.0 / hz as f64),
        active_count,
        mode,
    })
}

#[cfg(feature = "bench-support")]
fn build_cpu_mask_tint_fixture(
    active_count: usize,
    hz: u32,
    mode: MaskTintResolveMode,
) -> CpuMaskTintFrameFixture {
    const TOTAL_ICONS: usize = 1000;

    let (tree, states) = build_mask_tint_tree(active_count);
    let viewport = Rect::new(0.0, 0.0, 960.0, 600.0);
    let mut context = WidgetBenchmarkContext::new().with_viewport(viewport);
    context.set_force_legacy_texture_mask_tint_reactive_resolve(
        mode == MaskTintResolveMode::LegacyFullVisual,
    );
    let start = Instant::now();
    let stats = context.run_layout_and_scene(&tree, start);
    assert_eq!(stats.texture_count, TOTAL_ICONS);

    for state in &states {
        state.set(Color::rgba(244, 63, 94, 176));
    }
    let animation_start = Instant::now();
    context.invalidate_all();
    let stats = context.run_layout_and_scene(&tree, animation_start);
    assert_eq!(stats.texture_count, TOTAL_ICONS);
    assert!(context.has_active_property_animations());

    CpuMaskTintFrameFixture {
        tree,
        context,
        next_frame: animation_start + Duration::from_secs_f64(1.0 / hz as f64),
        frame_interval: Duration::from_secs_f64(1.0 / hz as f64),
        active_count,
        mode,
    }
}

#[cfg(feature = "bench-support")]
fn assert_mask_tint_equivalence(
    retained: &mut MaskTintFrameFixture,
    full: &mut MaskTintFrameFixture,
) {
    let retained_before = retained
        .context
        .cached_texture_retained_state()
        .expect("retained mask-tint baseline");
    let full_before = full
        .context
        .cached_texture_retained_state()
        .expect("full-recollect mask-tint baseline");
    retained.render_frame();
    full.render_frame();
    let retained_after = retained
        .context
        .cached_texture_retained_state()
        .expect("retained mask-tint result");
    let full_after = full
        .context
        .cached_texture_retained_state()
        .expect("full-recollect mask-tint result");

    assert_eq!(retained_after.texture_ids, retained_before.texture_ids);
    assert_eq!(
        retained_after.texture_revisions,
        retained_before.texture_revisions
    );
    assert_eq!(retained_after.frames, retained_before.frames);
    assert_eq!(
        retained_after.media_key_fingerprints,
        retained_before.media_key_fingerprints
    );
    assert_eq!(
        retained_after.media_layout_fingerprints,
        retained_before.media_layout_fingerprints
    );
    assert_eq!(
        retained_after.prepare_cache_serial,
        retained_before.prepare_cache_serial
    );
    assert!(!retained_after.cache_liveness_dirty);
    assert_ne!(retained_after.mask_tints, retained_before.mask_tints);

    assert_ne!(
        full_after.prepare_cache_serial,
        full_before.prepare_cache_serial
    );
    assert_eq!(retained_after.frames, full_after.frames);
    assert_eq!(retained_after.mask_tints, full_after.mask_tints);
    assert_eq!(
        retained_after.media_key_fingerprints,
        full_after.media_key_fingerprints
    );
    assert_eq!(
        retained_after.media_layout_fingerprints,
        full_after.media_layout_fingerprints
    );

    let retained_pixels = retained
        .context
        .headless_output_rgba()
        .expect("retained mask-tint output readback");
    let full_pixels = full
        .context
        .headless_output_rgba()
        .expect("full-recollect mask-tint output readback");
    assert_eq!(
        retained_pixels, full_pixels,
        "retained texture-mask tint must be pixel-identical to full scene recollection"
    );
}

#[cfg(feature = "bench-support")]
fn percentile(samples: &[Duration], percentile: f64) -> Duration {
    let index = ((samples.len() - 1) as f64 * percentile).round() as usize;
    samples[index]
}

#[cfg(feature = "bench-support")]
fn assert_cpu_mask_tint_equivalence(
    direct: &mut CpuMaskTintFrameFixture,
    legacy: &mut CpuMaskTintFrameFixture,
) {
    assert_eq!(direct.mode, MaskTintResolveMode::Direct);
    assert_eq!(legacy.mode, MaskTintResolveMode::LegacyFullVisual);
    direct.render_frame();
    legacy.render_frame();
    let direct_state = direct
        .context
        .cached_texture_retained_state()
        .expect("direct mask-tint CPU state");
    let legacy_state = legacy
        .context
        .cached_texture_retained_state()
        .expect("legacy mask-tint CPU state");
    assert_eq!(
        direct_state.texture_ids.len(),
        legacy_state.texture_ids.len()
    );
    assert_eq!(
        direct_state.texture_revisions,
        legacy_state.texture_revisions
    );
    assert_eq!(direct_state.frames, legacy_state.frames);
    assert_eq!(direct_state.mask_tints, legacy_state.mask_tints);
    assert_eq!(
        direct_state.media_key_fingerprints,
        legacy_state.media_key_fingerprints
    );
    assert_eq!(
        direct_state.media_layout_fingerprints,
        legacy_state.media_layout_fingerprints
    );
}

#[cfg(feature = "bench-support")]
fn build_cpu_container_opacity_fixture(
    mode: ContainerOpacityResolveMode,
    complex_surface: bool,
) -> CpuContainerOpacityFrameFixture {
    const CELL_COUNT: usize = 1000;
    const COLUMNS: usize = 40;
    let view_model = ViewModelContext::for_benchmarks();
    let transition = Transition::linear(Duration::from_millis(480));
    let mut body = Flex::<()>::new(Axis::Vertical)
        .width(dp(1000.0))
        .gap(dp(1.0))
        .opacity(0.8);
    for row_index in 0..(CELL_COUNT / COLUMNS) {
        let mut row = Flex::<()>::new(Axis::Horizontal)
            .size(dp(1000.0), dp(22.0))
            .gap(dp(1.0));
        for column in 0..COLUMNS {
            let index = row_index * COLUMNS + column;
            let state = view_model.state(if index % 2 == 0 { 0.35 } else { 0.85 });
            let signal = state.signal().animated(transition);
            row = row.child(
                Flex::<()>::new(Axis::Horizontal)
                    .size(dp(24.0), dp(22.0))
                    .opacity(signal)
                    .style(move |style, context| {
                        style.surface.background = Some(Color::rgba(15, 23, 42, 255).into());
                        style.surface.border_width = Some(dp(1.0).into());
                        style.surface.border_color = Some(Color::rgba(56, 189, 248, 224).into());
                        style.surface.border_radius = Some(dp(6.0).into());
                        if complex_surface {
                            style.surface.shadow = Some(context.theme.elevation.sm.clone().into());
                        }
                    }),
            );
        }
        body = body.child(row);
    }
    let tree = Box::new(WidgetTree::new(body));
    let viewport = Rect::new(0.0, 0.0, 1000.0, 600.0);
    let mut context = WidgetBenchmarkContext::new().with_viewport(viewport);
    context.set_force_legacy_container_opacity_reactive_resolve(
        mode == ContainerOpacityResolveMode::LegacyFullVisual,
    );
    let start = Instant::now();
    assert_eq!(
        context.run_layout_and_scene(&tree, start).shape_count,
        CELL_COUNT * 2
    );
    CpuContainerOpacityFrameFixture {
        tree,
        context,
        next_frame: start + Duration::from_millis(1),
        frame_interval: Duration::from_secs_f64(1.0 / 120.0),
        active_count: CELL_COUNT,
        mode,
    }
}

#[cfg(feature = "bench-support")]
fn build_cpu_background_blur_fixture(
    mode: BackgroundBlurResolveMode,
    complex_surface: bool,
) -> CpuBackgroundBlurFrameFixture {
    const CELL_COUNT: usize = 1000;
    const COLUMNS: usize = 40;
    let view_model = ViewModelContext::for_benchmarks();
    let transition = Transition::linear(Duration::from_millis(480));
    let mut body = Flex::<()>::new(Axis::Vertical)
        .width(dp(1000.0))
        .gap(dp(1.0))
        .opacity(0.75);
    for row_index in 0..(CELL_COUNT / COLUMNS) {
        let mut row = Flex::<()>::new(Axis::Horizontal)
            .size(dp(1000.0), dp(22.0))
            .gap(dp(1.0));
        for column in 0..COLUMNS {
            let index = row_index * COLUMNS + column;
            let state = view_model.state(dp(if index % 2 == 0 { 6.0 } else { 10.0 }));
            let signal = state.signal().animated(transition);
            row = row.child(
                Flex::<()>::new(Axis::Horizontal)
                    .size(dp(24.0), dp(22.0))
                    .style(move |style, context| {
                        style.surface.background = Some(Color::rgba(15, 23, 42, 176).into());
                        style.surface.background_blur = signal.clone().into();
                        style.surface.border_width = Some(dp(1.0).into());
                        style.surface.border_color = Some(Color::rgba(56, 189, 248, 160).into());
                        style.surface.border_radius = Some(dp(7.0).into());
                        if complex_surface {
                            style.surface.shadow = Some(context.theme.elevation.sm.clone().into());
                        }
                    }),
            );
        }
        body = body.child(row);
    }
    let tree = Box::new(WidgetTree::new(body));
    let viewport = Rect::new(0.0, 0.0, 1000.0, 600.0);
    let mut context = WidgetBenchmarkContext::new().with_viewport(viewport);
    context.set_force_legacy_background_blur_reactive_resolve(
        mode == BackgroundBlurResolveMode::LegacyFullVisual,
    );
    let start = Instant::now();
    assert_eq!(
        context.run_layout_and_scene(&tree, start).shape_count,
        CELL_COUNT * 2
    );
    CpuBackgroundBlurFrameFixture {
        tree,
        context,
        next_frame: start + Duration::from_millis(1),
        frame_interval: Duration::from_secs_f64(1.0 / 120.0),
        active_count: CELL_COUNT,
        mode,
    }
}

#[cfg(feature = "bench-support")]
fn benchmark_background_brush(index: usize, mixed: bool) -> BackgroundBrush {
    if !mixed || index % 3 == 0 {
        return BackgroundBrush::Solid(Color::rgba(32 + (index % 96) as u8, 132, 220, 216));
    }
    if index % 3 == 1 {
        return BackgroundBrush::LinearGradient(BackgroundLinearGradient::new(
            Point::new(dp(0.0), dp(0.0)),
            Point::new(dp(24.0), dp(22.0)),
            vec![
                BackgroundGradientStop::new(0.0, Color::rgba(56, 189, 248, 224)),
                BackgroundGradientStop::new(1.0, Color::rgba(249, 115, 22, 192)),
            ],
        ));
    }
    BackgroundBrush::RadialGradient(BackgroundRadialGradient::new(
        Point::new(dp(12.0), dp(11.0)),
        dp(12.0),
        vec![
            BackgroundGradientStop::new(0.0, Color::rgba(244, 63, 94, 208)),
            BackgroundGradientStop::new(1.0, Color::rgba(30, 64, 175, 144)),
        ],
    ))
}

#[cfg(feature = "bench-support")]
fn build_cpu_background_brush_fixture(
    mode: BackgroundBrushResolveMode,
    mixed: bool,
    complex_surface: bool,
) -> CpuBackgroundBrushFrameFixture {
    const CELL_COUNT: usize = 1000;
    const COLUMNS: usize = 40;
    let view_model = ViewModelContext::for_benchmarks();
    let mut body = Flex::<()>::new(Axis::Vertical)
        .width(dp(1000.0))
        .gap(dp(1.0))
        .opacity(0.75);
    for row_index in 0..(CELL_COUNT / COLUMNS) {
        let mut row = Flex::<()>::new(Axis::Horizontal)
            .size(dp(1000.0), dp(22.0))
            .gap(dp(1.0));
        for column in 0..COLUMNS {
            let index = row_index * COLUMNS + column;
            let state = view_model.state(benchmark_background_brush(index, mixed));
            let signal = state.signal();
            row = row.child(
                Flex::<()>::new(Axis::Horizontal)
                    .size(dp(24.0), dp(22.0))
                    .style(move |style, _| {
                        style.surface.background = Some(Color::rgba(15, 23, 42, 160).into());
                        style.surface.background_brush = Some(signal.clone().into());
                        style.surface.border_width = Some(dp(1.0).into());
                        style.surface.border_color = Some(Color::rgba(148, 163, 184, 176).into());
                        style.surface.border_radius = Some(dp(7.0).into());
                        if complex_surface {
                            style.surface.background_blur = dp(6.0).into();
                        }
                    }),
            );
        }
        body = body.child(row);
    }
    let tree = Box::new(WidgetTree::new(body));
    let viewport = Rect::new(0.0, 0.0, 1000.0, 600.0);
    let mut context = WidgetBenchmarkContext::new().with_viewport(viewport);
    context.set_force_legacy_background_brush_reactive_resolve(
        mode == BackgroundBrushResolveMode::LegacyFullVisual,
    );
    let start = Instant::now();
    assert_eq!(
        context.run_layout_and_scene(&tree, start).shape_count,
        CELL_COUNT
    );
    CpuBackgroundBrushFrameFixture {
        tree,
        context,
        next_frame: start + Duration::from_millis(1),
        frame_interval: Duration::from_secs_f64(1.0 / 120.0),
        active_count: CELL_COUNT,
        mode,
    }
}

#[cfg(feature = "bench-support")]
fn build_cpu_offset_fixture(
    mode: OffsetResolveMode,
    complex_surface: bool,
) -> CpuOffsetFrameFixture {
    const CELL_COUNT: usize = 1000;
    const COLUMNS: usize = 40;
    let view_model = ViewModelContext::for_benchmarks();
    let transition = Transition::linear(Duration::from_millis(480));
    let mut body = Flex::<()>::new(Axis::Vertical)
        .width(dp(1000.0))
        .gap(dp(1.0))
        .opacity(0.75);
    for row_index in 0..(CELL_COUNT / COLUMNS) {
        let mut row = Flex::<()>::new(Axis::Horizontal)
            .size(dp(1000.0), dp(22.0))
            .gap(dp(1.0));
        for column in 0..COLUMNS {
            let index = row_index * COLUMNS + column;
            let state = view_model.state(Point::new(
                dp(if index % 2 == 0 { -2.0 } else { 2.0 }),
                dp(0.0),
            ));
            let signal = state.signal().animated(transition);
            row = row.child(
                Flex::<()>::new(Axis::Horizontal)
                    .size(dp(24.0), dp(22.0))
                    .offset(signal)
                    .style(move |style, _| {
                        style.surface.background = Some(Color::rgba(34, 211, 238, 230).into());
                        style.surface.border_radius = Some(dp(5.0).into());
                        if complex_surface {
                            style.surface.border_width = Some(dp(1.0).into());
                            style.surface.border_color =
                                Some(Color::rgba(148, 163, 184, 176).into());
                        }
                    }),
            );
        }
        body = body.child(row);
    }
    let tree = Box::new(WidgetTree::new(body));
    let viewport = Rect::new(0.0, 0.0, 1000.0, 600.0);
    let mut context = WidgetBenchmarkContext::new().with_viewport(viewport);
    context.set_force_legacy_offset_reactive_resolve(mode == OffsetResolveMode::LegacyFullVisual);
    let start = Instant::now();
    assert_eq!(
        context.run_layout_and_scene(&tree, start).shape_count,
        CELL_COUNT * if complex_surface { 2 } else { 1 }
    );
    CpuOffsetFrameFixture {
        tree,
        context,
        next_frame: start + Duration::from_millis(1),
        frame_interval: Duration::from_secs_f64(1.0 / 120.0),
        active_count: CELL_COUNT,
        mode,
    }
}

#[cfg(feature = "bench-support")]
fn build_cpu_scale_fixture(mode: ScaleResolveMode, complex_surface: bool) -> CpuScaleFrameFixture {
    const ELIGIBLE_COUNT: usize = 1000;
    const FALLBACK_COUNT: usize = 256;
    const ELIGIBLE_COLUMNS: usize = 40;
    const FALLBACK_COLUMNS: usize = 32;

    let active_count = if complex_surface {
        FALLBACK_COUNT
    } else {
        ELIGIBLE_COUNT
    };
    let columns = if complex_surface {
        FALLBACK_COLUMNS
    } else {
        ELIGIBLE_COLUMNS
    };
    let view_model = ViewModelContext::for_benchmarks();
    let transition = Transition::linear(Duration::from_millis(480));
    let mut body = Flex::<()>::new(Axis::Vertical)
        .width(dp(1000.0))
        .overflow(Overflow::Hidden)
        .gap(dp(1.0))
        .opacity(0.75);
    for row_index in 0..(active_count / columns) {
        let mut row = Flex::<()>::new(Axis::Horizontal)
            .size(dp(1000.0), dp(22.0))
            .overflow(Overflow::Hidden)
            .gap(dp(1.0));
        for column in 0..columns {
            let index = row_index * columns + column;
            let state = view_model.state(1.0 + (index % 5) as f32 * 0.04);
            let signal = state.signal().animated(transition);
            row = row.child(
                Flex::<()>::new(Axis::Horizontal)
                    .size(dp(24.0), dp(22.0))
                    .overflow(Overflow::Hidden)
                    .scale(signal)
                    .style(move |style, context| {
                        style.surface.background = Some(Color::rgba(34, 211, 238, 230).into());
                        style.surface.border_radius = Some(dp(5.0).into());
                        if complex_surface {
                            if index % 2 == 0 {
                                style.surface.shadow =
                                    Some(context.theme.elevation.sm.clone().into());
                            } else {
                                style.surface.background_brush = Some(
                                    BackgroundBrush::Solid(Color::rgba(99, 102, 241, 160)).into(),
                                );
                            }
                        }
                    }),
            );
        }
        body = body.child(row);
    }
    let tree = Box::new(WidgetTree::new(body));
    let viewport = Rect::new(0.0, 0.0, 1000.0, 600.0);
    let mut context = WidgetBenchmarkContext::new().with_viewport(viewport);
    context.set_force_legacy_scale_reactive_resolve(mode == ScaleResolveMode::LegacyFullVisual);
    let start = Instant::now();
    let _ = context.run_layout_and_scene(&tree, start);
    CpuScaleFrameFixture {
        tree,
        context,
        next_frame: start + Duration::from_millis(1),
        frame_interval: Duration::from_secs_f64(1.0 / 120.0),
        active_count,
        mode,
        complex_surface,
    }
}

#[cfg(feature = "bench-support")]
fn build_cpu_text_opacity_fixture(
    mode: TextOpacityResolveMode,
    complex_surface: bool,
) -> CpuTextOpacityFrameFixture {
    const TEXT_COUNT: usize = 1000;
    const COLUMNS: usize = 40;
    let view_model = ViewModelContext::for_benchmarks();
    let transition = Transition::linear(Duration::from_millis(480));
    let mut body = Flex::<()>::new(Axis::Vertical)
        .width(dp(1000.0))
        .gap(dp(1.0));
    for row_index in 0..(TEXT_COUNT / COLUMNS) {
        let mut row = Flex::<()>::new(Axis::Horizontal)
            .size(dp(1000.0), dp(22.0))
            .gap(dp(1.0));
        for column in 0..COLUMNS {
            let index = row_index * COLUMNS + column;
            let state = view_model.state(if index % 2 == 0 { 0.35 } else { 0.85 });
            let signal = state.signal().animated(transition);
            row = row.child(
                Text::new(format!("T{index:04}"))
                    .style(move |style, context| {
                        style.color = Color::rgba(56, 189, 248, 224).into();
                        style.surface.opacity = signal.clone().into();
                        if complex_surface {
                            if index % 2 == 0 {
                                style.surface.shadow =
                                    Some(context.theme.elevation.sm.clone().into());
                            } else {
                                style.surface.background_brush = Some(
                                    BackgroundBrush::Solid(Color::rgba(99, 102, 241, 160)).into(),
                                );
                            }
                        }
                    })
                    .size(dp(24.0), dp(22.0)),
            );
        }
        body = body.child(row);
    }
    let tree = Box::new(WidgetTree::new(body));
    let viewport = Rect::new(0.0, 0.0, 1000.0, 600.0);
    let mut context = WidgetBenchmarkContext::new().with_viewport(viewport);
    context.set_force_legacy_text_opacity_reactive_resolve(
        mode == TextOpacityResolveMode::LegacyFullVisual,
    );
    let start = Instant::now();
    assert_eq!(
        context.run_layout_and_scene(&tree, start).text_count,
        TEXT_COUNT
    );
    CpuTextOpacityFrameFixture {
        tree,
        context,
        next_frame: start + Duration::from_millis(1),
        frame_interval: Duration::from_secs_f64(1.0 / 120.0),
        active_count: TEXT_COUNT,
        mode,
        complex_surface,
    }
}

#[cfg(feature = "bench-support")]
fn build_cpu_text_color_fixture(mode: TextColorResolveMode) -> CpuTextColorFrameFixture {
    const TEXT_COUNT: usize = 1000;
    const COLUMNS: usize = 40;

    let view_model = ViewModelContext::for_benchmarks();
    let transition = Transition::linear(Duration::from_millis(480));
    let mut body = Flex::<()>::new(Axis::Vertical)
        .width(dp(1000.0))
        .gap(dp(1.0));
    for row_index in 0..(TEXT_COUNT / COLUMNS) {
        let mut row = Flex::<()>::new(Axis::Horizontal)
            .size(dp(1000.0), dp(22.0))
            .gap(dp(1.0));
        for column in 0..COLUMNS {
            let index = row_index * COLUMNS + column;
            let state = view_model.state(Color::rgba(14, 165, 233, 224));
            let signal = state.signal().animated(transition);
            row = row.child(
                Text::new(format!("T{index}"))
                    .size(dp(24.0), dp(22.0))
                    .style_full(move |context| {
                        let mut style =
                            tgui::widgets::TextWidgetStyle::default_for_theme(context.theme);
                        style.color = signal.clone().into();
                        style
                    }),
            );
        }
        body = body.child(row);
    }
    let tree = Box::new(WidgetTree::new(body));
    let viewport = Rect::new(0.0, 0.0, 1000.0, 600.0);
    let mut context = WidgetBenchmarkContext::new().with_viewport(viewport);
    context.set_force_legacy_text_color_reactive_resolve(
        mode == TextColorResolveMode::LegacyFullVisual,
    );
    let start = Instant::now();
    assert_eq!(
        context.run_layout_and_scene(&tree, start).text_count,
        TEXT_COUNT
    );
    CpuTextColorFrameFixture {
        tree,
        context,
        next_frame: Instant::now(),
        frame_interval: Duration::from_secs_f64(1.0 / 120.0),
        active_count: TEXT_COUNT,
        mode,
    }
}

#[cfg(feature = "bench-support")]
fn build_cpu_background_fixture(
    mode: BackgroundResolveMode,
    complex_surface: bool,
) -> CpuBackgroundFrameFixture {
    const CELL_COUNT: usize = 1000;
    const COLUMNS: usize = 40;

    let view_model = ViewModelContext::for_benchmarks();
    let transition = Transition::linear(Duration::from_millis(480));
    let mut body = Flex::<()>::new(Axis::Vertical)
        .width(dp(1000.0))
        .gap(dp(1.0));
    for row_index in 0..(CELL_COUNT / COLUMNS) {
        let mut row = Flex::<()>::new(Axis::Horizontal)
            .size(dp(1000.0), dp(22.0))
            .gap(dp(1.0));
        for column in 0..COLUMNS {
            let index = row_index * COLUMNS + column;
            let state = view_model.state(if index % 2 == 0 {
                Color::rgba(14, 165, 233, 224)
            } else {
                Color::rgba(139, 92, 246, 224)
            });
            let signal = state.signal().animated(transition);
            row = row.child(
                Flex::<()>::new(Axis::Horizontal)
                    .size(dp(24.0), dp(22.0))
                    .style(move |style, context| {
                        style.surface.background = Some(signal.clone().into());
                        if complex_surface {
                            style.surface.shadow = Some(context.theme.elevation.sm.clone().into());
                        }
                    }),
            );
        }
        body = body.child(row);
    }
    let tree = Box::new(WidgetTree::new(body));
    let viewport = Rect::new(0.0, 0.0, 1000.0, 600.0);
    let mut context = WidgetBenchmarkContext::new().with_viewport(viewport);
    context.set_force_legacy_background_reactive_resolve(
        mode == BackgroundResolveMode::LegacyFullVisual,
    );
    let start = Instant::now();
    assert_eq!(
        context.run_layout_and_scene(&tree, start).shape_count,
        CELL_COUNT
    );
    CpuBackgroundFrameFixture {
        tree,
        context,
        next_frame: start + Duration::from_millis(1),
        frame_interval: Duration::from_secs_f64(1.0 / 120.0),
        active_count: CELL_COUNT,
        mode,
    }
}

#[cfg(feature = "bench-support")]
fn bench_background_resolution_cpu(c: &mut Criterion) {
    let mut direct = build_cpu_background_fixture(BackgroundResolveMode::Direct, false);
    let mut legacy = build_cpu_background_fixture(BackgroundResolveMode::LegacyFullVisual, false);
    let (_, direct_values) = direct.render_frame();
    let (_, legacy_values) = legacy.render_frame();
    assert_eq!(direct_values, legacy_values);

    let mut direct_probe = Vec::with_capacity(64);
    let mut legacy_probe = Vec::with_capacity(64);
    for _ in 0..64 {
        direct_probe.push(direct.render_frame().0);
        legacy_probe.push(legacy.render_frame().0);
    }
    direct_probe.sort_unstable();
    legacy_probe.sort_unstable();
    let direct_p50 = percentile(&direct_probe, 0.50);
    let direct_p95 = percentile(&direct_probe, 0.95);
    let legacy_p50 = percentile(&legacy_probe, 0.50);
    let legacy_p95 = percentile(&legacy_probe, 0.95);
    let reduction = 100.0 * (1.0 - direct_p50.as_secs_f64() / legacy_p50.as_secs_f64());
    eprintln!(
        "background_resolve_cpu_budget: active=1000 direct_p50_ms={:.3} direct_p95_ms={:.3} legacy_p50_ms={:.3} legacy_p95_ms={:.3} median_reduction_pct={:.1} value_equivalent=true fallback=false",
        direct_p50.as_secs_f64() * 1_000.0,
        direct_p95.as_secs_f64() * 1_000.0,
        legacy_p50.as_secs_f64() * 1_000.0,
        legacy_p95.as_secs_f64() * 1_000.0,
        reduction,
    );

    let mut fallback_direct = build_cpu_background_fixture(BackgroundResolveMode::Direct, true);
    let mut fallback_legacy =
        build_cpu_background_fixture(BackgroundResolveMode::LegacyFullVisual, true);
    let (_, fallback_direct_values) = fallback_direct.render_frame();
    let (_, fallback_legacy_values) = fallback_legacy.render_frame();
    assert_eq!(fallback_direct_values, fallback_legacy_values);
    let mut fallback_direct_probe = Vec::with_capacity(64);
    let mut fallback_legacy_probe = Vec::with_capacity(64);
    for _ in 0..64 {
        fallback_direct_probe.push(fallback_direct.render_frame().0);
        fallback_legacy_probe.push(fallback_legacy.render_frame().0);
    }
    fallback_direct_probe.sort_unstable();
    fallback_legacy_probe.sort_unstable();
    let fallback_direct_p50 = percentile(&fallback_direct_probe, 0.50);
    let fallback_legacy_p50 = percentile(&fallback_legacy_probe, 0.50);
    eprintln!(
        "background_fallback_cpu_budget: active=1000 direct_fallback_p50_ms={:.3} legacy_p50_ms={:.3} overhead_pct={:.1} value_equivalent=true fallback=true",
        fallback_direct_p50.as_secs_f64() * 1_000.0,
        fallback_legacy_p50.as_secs_f64() * 1_000.0,
        100.0 * (fallback_direct_p50.as_secs_f64() / fallback_legacy_p50.as_secs_f64() - 1.0),
    );

    let mut group = c.benchmark_group("background_reactive_resolve_cpu/120Hz");
    group.throughput(Throughput::Elements(1000));
    group.bench_function("direct/1000", |b| {
        b.iter(|| black_box(direct.render_frame()))
    });
    group.bench_function("legacy_full_visual/1000", |b| {
        b.iter(|| black_box(legacy.render_frame()))
    });
    group.finish();
}

#[cfg(feature = "bench-support")]
fn build_cpu_border_color_fixture(
    mode: BorderColorResolveMode,
    complex_surface: bool,
) -> CpuBorderColorFrameFixture {
    const CELL_COUNT: usize = 1000;
    const COLUMNS: usize = 40;

    let view_model = ViewModelContext::for_benchmarks();
    let transition = Transition::linear(Duration::from_millis(480));
    let mut body = Flex::<()>::new(Axis::Vertical)
        .width(dp(1000.0))
        .gap(dp(1.0));
    for row_index in 0..(CELL_COUNT / COLUMNS) {
        let mut row = Flex::<()>::new(Axis::Horizontal)
            .size(dp(1000.0), dp(22.0))
            .gap(dp(1.0));
        for column in 0..COLUMNS {
            let index = row_index * COLUMNS + column;
            let state = view_model.state(if index % 2 == 0 {
                Color::rgba(14, 165, 233, 224)
            } else {
                Color::rgba(139, 92, 246, 224)
            });
            let signal = state.signal().animated(transition);
            row = row.child(
                Flex::<()>::new(Axis::Horizontal)
                    .size(dp(24.0), dp(22.0))
                    .style(move |style, context| {
                        style.surface.background = Some(Color::rgba(15, 23, 42, 255).into());
                        style.surface.border_width = Some(dp(1.0).into());
                        style.surface.border_color = Some(signal.clone().into());
                        if complex_surface {
                            style.surface.shadow = Some(context.theme.elevation.sm.clone().into());
                        }
                    }),
            );
        }
        body = body.child(row);
    }
    let tree = Box::new(WidgetTree::new(body));
    let viewport = Rect::new(0.0, 0.0, 1000.0, 600.0);
    let mut context = WidgetBenchmarkContext::new().with_viewport(viewport);
    context.set_force_legacy_border_color_reactive_resolve(
        mode == BorderColorResolveMode::LegacyFullVisual,
    );
    let start = Instant::now();
    assert_eq!(
        context.run_layout_and_scene(&tree, start).shape_count,
        CELL_COUNT * 2
    );
    CpuBorderColorFrameFixture {
        tree,
        context,
        next_frame: start + Duration::from_millis(1),
        frame_interval: Duration::from_secs_f64(1.0 / 120.0),
        active_count: CELL_COUNT,
        mode,
    }
}

#[cfg(feature = "bench-support")]
fn bench_border_color_resolution_cpu(c: &mut Criterion) {
    let mut direct = build_cpu_border_color_fixture(BorderColorResolveMode::Direct, false);
    let mut legacy =
        build_cpu_border_color_fixture(BorderColorResolveMode::LegacyFullVisual, false);
    let mut full = build_cpu_border_color_fixture(BorderColorResolveMode::Direct, false);
    let (_, direct_values) = direct.render_frame();
    let (_, legacy_values) = legacy.render_frame();
    let full_values = full
        .context
        .full_recollect_plain_container_border_colors_for_benchmark(&full.tree, full.next_frame);
    assert_eq!(direct_values, legacy_values);
    assert_eq!(direct_values, full_values);

    let mut direct_probe = Vec::with_capacity(64);
    let mut legacy_probe = Vec::with_capacity(64);
    for _ in 0..64 {
        direct_probe.push(direct.render_frame().0);
        legacy_probe.push(legacy.render_frame().0);
    }
    direct_probe.sort_unstable();
    legacy_probe.sort_unstable();
    let direct_p50 = percentile(&direct_probe, 0.50);
    let direct_p95 = percentile(&direct_probe, 0.95);
    let legacy_p50 = percentile(&legacy_probe, 0.50);
    let legacy_p95 = percentile(&legacy_probe, 0.95);
    let reduction = 100.0 * (1.0 - direct_p50.as_secs_f64() / legacy_p50.as_secs_f64());
    eprintln!(
        "border_color_resolve_cpu_budget: active=1000 direct_p50_ms={:.3} direct_p95_ms={:.3} legacy_p50_ms={:.3} legacy_p95_ms={:.3} median_reduction_pct={:.1} direct_legacy_full_equivalent=true topology_guard=unchanged fallback=false",
        direct_p50.as_secs_f64() * 1_000.0,
        direct_p95.as_secs_f64() * 1_000.0,
        legacy_p50.as_secs_f64() * 1_000.0,
        legacy_p95.as_secs_f64() * 1_000.0,
        reduction,
    );

    let mut fallback_direct = build_cpu_border_color_fixture(BorderColorResolveMode::Direct, true);
    let mut fallback_legacy =
        build_cpu_border_color_fixture(BorderColorResolveMode::LegacyFullVisual, true);
    let (_, fallback_direct_values) = fallback_direct.render_frame();
    let (_, fallback_legacy_values) = fallback_legacy.render_frame();
    assert_eq!(fallback_direct_values, fallback_legacy_values);
    let mut fallback_direct_probe = Vec::with_capacity(64);
    let mut fallback_legacy_probe = Vec::with_capacity(64);
    for _ in 0..64 {
        fallback_direct_probe.push(fallback_direct.render_frame().0);
        fallback_legacy_probe.push(fallback_legacy.render_frame().0);
    }
    fallback_direct_probe.sort_unstable();
    fallback_legacy_probe.sort_unstable();
    let fallback_direct_p50 = percentile(&fallback_direct_probe, 0.50);
    let fallback_legacy_p50 = percentile(&fallback_legacy_probe, 0.50);
    eprintln!(
        "border_color_fallback_cpu_budget: active=1000 direct_fallback_p50_ms={:.3} legacy_p50_ms={:.3} overhead_pct={:.1} value_equivalent=true fallback=true",
        fallback_direct_p50.as_secs_f64() * 1_000.0,
        fallback_legacy_p50.as_secs_f64() * 1_000.0,
        100.0 * (fallback_direct_p50.as_secs_f64() / fallback_legacy_p50.as_secs_f64() - 1.0),
    );

    let mut group = c.benchmark_group("border_color_reactive_resolve_cpu/120Hz");
    group.throughput(Throughput::Elements(1000));
    group.bench_function("direct/1000", |b| {
        b.iter(|| black_box(direct.render_frame()))
    });
    group.bench_function("legacy_full_visual/1000", |b| {
        b.iter(|| black_box(legacy.render_frame()))
    });
    group.finish();
}

#[cfg(feature = "bench-support")]
fn build_cpu_border_radius_fixture(mode: BorderRadiusResolveMode) -> CpuBorderRadiusFrameFixture {
    const CELL_COUNT: usize = 1000;
    const COLUMNS: usize = 40;

    let view_model = ViewModelContext::for_benchmarks();
    let transition = Transition::linear(Duration::from_millis(480));
    let mut body = Flex::<()>::new(Axis::Vertical)
        .width(dp(1000.0))
        .gap(dp(1.0));
    for row_index in 0..(CELL_COUNT / COLUMNS) {
        let mut row = Flex::<()>::new(Axis::Horizontal)
            .size(dp(1000.0), dp(22.0))
            .gap(dp(1.0));
        for column in 0..COLUMNS {
            let index = row_index * COLUMNS + column;
            let state = view_model.state(dp(if index % 2 == 0 { 4.0 } else { 9.0 }));
            let signal = state.signal().animated(transition);
            row = row.child(
                Flex::<()>::new(Axis::Horizontal)
                    .size(dp(24.0), dp(22.0))
                    .style(move |style, _| {
                        style.surface.background = Some(Color::rgba(15, 23, 42, 255).into());
                        style.surface.border_width = Some(dp(1.0).into());
                        style.surface.border_color = Some(Color::rgba(56, 189, 248, 224).into());
                        style.surface.border_radius = Some(signal.clone().into());
                    }),
            );
        }
        body = body.child(row);
    }
    let tree = Box::new(WidgetTree::new(body));
    let viewport = Rect::new(0.0, 0.0, 1000.0, 600.0);
    let mut context = WidgetBenchmarkContext::new().with_viewport(viewport);
    context.set_force_legacy_border_radius_reactive_resolve(
        mode == BorderRadiusResolveMode::LegacyFullVisual,
    );
    let start = Instant::now();
    assert_eq!(
        context.run_layout_and_scene(&tree, start).shape_count,
        CELL_COUNT * 2
    );
    CpuBorderRadiusFrameFixture {
        tree,
        context,
        next_frame: start + Duration::from_millis(1),
        frame_interval: Duration::from_secs_f64(1.0 / 120.0),
        active_count: CELL_COUNT,
        mode,
    }
}

#[cfg(feature = "bench-support")]
fn bench_border_radius_resolution_cpu(c: &mut Criterion) {
    let mut direct = build_cpu_border_radius_fixture(BorderRadiusResolveMode::Direct);
    let mut legacy = build_cpu_border_radius_fixture(BorderRadiusResolveMode::LegacyFullVisual);
    let (_, direct_values) = direct.render_frame();
    let (_, legacy_values) = legacy.render_frame();
    assert_eq!(direct_values, legacy_values);

    let mut direct_probe = Vec::with_capacity(64);
    let mut legacy_probe = Vec::with_capacity(64);
    for _ in 0..64 {
        direct_probe.push(direct.render_frame().0);
        legacy_probe.push(legacy.render_frame().0);
    }
    direct_probe.sort_unstable();
    legacy_probe.sort_unstable();
    let direct_p50 = percentile(&direct_probe, 0.50);
    let direct_p95 = percentile(&direct_probe, 0.95);
    let legacy_p50 = percentile(&legacy_probe, 0.50);
    let legacy_p95 = percentile(&legacy_probe, 0.95);
    let reduction = 100.0 * (1.0 - direct_p50.as_secs_f64() / legacy_p50.as_secs_f64());
    eprintln!(
        "border_radius_resolve_cpu_budget: active=1000 direct_p50_ms={:.3} direct_p95_ms={:.3} legacy_p50_ms={:.3} legacy_p95_ms={:.3} median_reduction_pct={:.1} value_equivalent=true fallback=false",
        direct_p50.as_secs_f64() * 1_000.0,
        direct_p95.as_secs_f64() * 1_000.0,
        legacy_p50.as_secs_f64() * 1_000.0,
        legacy_p95.as_secs_f64() * 1_000.0,
        reduction,
    );

    let mut group = c.benchmark_group("border_radius_reactive_resolve_cpu/120Hz");
    group.throughput(Throughput::Elements(1000));
    group.bench_function("direct/1000", |b| {
        b.iter(|| black_box(direct.render_frame()))
    });
    group.bench_function("legacy_full_visual/1000", |b| {
        b.iter(|| black_box(legacy.render_frame()))
    });
    group.finish();
}

#[cfg(feature = "bench-support")]
fn build_cpu_border_width_fixture(
    mode: BorderWidthResolveMode,
    complex_surface: bool,
) -> CpuBorderWidthFrameFixture {
    const CELL_COUNT: usize = 1000;
    const COLUMNS: usize = 40;
    let view_model = ViewModelContext::for_benchmarks();
    let transition = Transition::linear(Duration::from_millis(480));
    let mut body = Flex::<()>::new(Axis::Vertical)
        .width(dp(1000.0))
        .gap(dp(1.0));
    for row_index in 0..(CELL_COUNT / COLUMNS) {
        let mut row = Flex::<()>::new(Axis::Horizontal)
            .size(dp(1000.0), dp(22.0))
            .gap(dp(1.0));
        for column in 0..COLUMNS {
            let index = row_index * COLUMNS + column;
            let state = view_model.state(dp(if index % 2 == 0 { 1.0 } else { 3.0 }));
            let signal = state.signal().animated(transition);
            row = row.child(
                Flex::<()>::new(Axis::Horizontal)
                    .size(dp(24.0), dp(22.0))
                    .style(move |style, context| {
                        style.surface.background = Some(Color::rgba(15, 23, 42, 255).into());
                        style.surface.border_width = Some(signal.clone().into());
                        style.surface.border_color = Some(Color::rgba(56, 189, 248, 224).into());
                        style.surface.border_radius = Some(dp(8.0).into());
                        if complex_surface {
                            if index % 2 == 0 {
                                style.surface.shadow =
                                    Some(context.theme.elevation.sm.clone().into());
                            } else {
                                style.surface.background_brush = Some(
                                    BackgroundBrush::Solid(Color::rgba(99, 102, 241, 160)).into(),
                                );
                            }
                        }
                    }),
            );
        }
        body = body.child(row);
    }
    let tree = Box::new(WidgetTree::new(body));
    let viewport = Rect::new(0.0, 0.0, 1000.0, 600.0);
    let mut context = WidgetBenchmarkContext::new().with_viewport(viewport);
    context.set_force_legacy_border_width_reactive_resolve(
        mode == BorderWidthResolveMode::LegacyFullVisual,
    );
    let start = Instant::now();
    assert_eq!(
        context.run_layout_and_scene(&tree, start).shape_count,
        if complex_surface {
            CELL_COUNT + CELL_COUNT / 2
        } else {
            CELL_COUNT * 2
        }
    );
    CpuBorderWidthFrameFixture {
        tree,
        context,
        next_frame: start + Duration::from_millis(1),
        frame_interval: Duration::from_secs_f64(1.0 / 120.0),
        active_count: CELL_COUNT,
        mode,
        complex_surface,
    }
}

#[cfg(feature = "bench-support")]
fn bench_border_width_resolution_cpu(c: &mut Criterion) {
    fn probe_pair(
        direct: &mut CpuBorderWidthFrameFixture,
        legacy: &mut CpuBorderWidthFrameFixture,
    ) -> (Duration, Duration, Duration, Duration) {
        const SAMPLE_COUNT: usize = 256;
        let mut direct_samples = Vec::with_capacity(SAMPLE_COUNT);
        let mut legacy_samples = Vec::with_capacity(SAMPLE_COUNT);
        for sample in 0..SAMPLE_COUNT {
            if sample % 2 == 0 {
                direct_samples.push(direct.render_frame().0);
                legacy_samples.push(legacy.render_frame().0);
            } else {
                legacy_samples.push(legacy.render_frame().0);
                direct_samples.push(direct.render_frame().0);
            }
        }
        direct_samples.sort_unstable();
        legacy_samples.sort_unstable();
        (
            percentile(&direct_samples, 0.50),
            percentile(&direct_samples, 0.95),
            percentile(&legacy_samples, 0.50),
            percentile(&legacy_samples, 0.95),
        )
    }

    let mut direct = build_cpu_border_width_fixture(BorderWidthResolveMode::Direct, false);
    let mut legacy =
        build_cpu_border_width_fixture(BorderWidthResolveMode::LegacyFullVisual, false);
    let (_, direct_values) = direct.render_frame();
    let (_, legacy_values) = legacy.render_frame();
    assert_eq!(direct_values, legacy_values);
    let (direct_p50, direct_p95, legacy_p50, legacy_p95) = probe_pair(&mut direct, &mut legacy);
    let reduction = 100.0 * (1.0 - direct_p50.as_secs_f64() / legacy_p50.as_secs_f64());
    eprintln!(
        "border_width_resolve_cpu_budget: active=1000 direct_p50_ms={:.3} direct_p95_ms={:.3} legacy_p50_ms={:.3} legacy_p95_ms={:.3} median_reduction_pct={:.1} value_equivalent=true fallback=false",
        direct_p50.as_secs_f64() * 1_000.0,
        direct_p95.as_secs_f64() * 1_000.0,
        legacy_p50.as_secs_f64() * 1_000.0,
        legacy_p95.as_secs_f64() * 1_000.0,
        reduction,
    );

    let mut fallback_direct = build_cpu_border_width_fixture(BorderWidthResolveMode::Direct, true);
    let mut fallback_legacy =
        build_cpu_border_width_fixture(BorderWidthResolveMode::LegacyFullVisual, true);
    let (_, fallback_direct_values) = fallback_direct.render_frame();
    let (_, fallback_legacy_values) = fallback_legacy.render_frame();
    assert_eq!(fallback_direct_values, fallback_legacy_values);
    let (fallback_direct_p50, fallback_direct_p95, fallback_legacy_p50, fallback_legacy_p95) =
        probe_pair(&mut fallback_direct, &mut fallback_legacy);
    let fallback_overhead =
        100.0 * (fallback_direct_p50.as_secs_f64() / fallback_legacy_p50.as_secs_f64() - 1.0);
    eprintln!(
        "border_width_fallback_cpu_budget: active=1000 direct_fallback_p50_ms={:.3} direct_fallback_p95_ms={:.3} legacy_p50_ms={:.3} legacy_p95_ms={:.3} overhead_pct={:.1} value_equivalent=true complex_surface=shadow+brush sticky_negative=true fallback=true",
        fallback_direct_p50.as_secs_f64() * 1_000.0,
        fallback_direct_p95.as_secs_f64() * 1_000.0,
        fallback_legacy_p50.as_secs_f64() * 1_000.0,
        fallback_legacy_p95.as_secs_f64() * 1_000.0,
        fallback_overhead,
    );

    let mut group = c.benchmark_group("border_width_reactive_resolve_cpu/120Hz");
    group.throughput(Throughput::Elements(1000));
    group.bench_function("direct/1000", |b| {
        b.iter(|| black_box(direct.render_frame()))
    });
    group.bench_function("legacy_full_visual/1000", |b| {
        b.iter(|| black_box(legacy.render_frame()))
    });
    group.finish();
}

#[cfg(feature = "bench-support")]
fn build_cpu_progress_value_fixture(
    mode: ProgressValueResolveMode,
) -> CpuProgressValueFrameFixture {
    const PROGRESS_COUNT: usize = 1000;
    let mut body = Flex::<()>::new(Axis::Vertical)
        .width(dp(1000.0))
        .gap(dp(1.0));
    for index in 0..PROGRESS_COUNT {
        body = body.child(ProgressBar::new((index % 101) as f32 / 100.0).size(dp(320.0), dp(12.0)));
    }
    let tree = Box::new(WidgetTree::new(body));
    let viewport = Rect::new(0.0, 0.0, 1000.0, 14_000.0);
    let mut context = WidgetBenchmarkContext::new().with_viewport(viewport);
    context.set_force_legacy_progress_value_reactive_resolve(
        mode == ProgressValueResolveMode::LegacyFullVisual,
    );
    let start = Instant::now();
    assert_eq!(
        context.run_layout_and_scene(&tree, start).shape_count,
        PROGRESS_COUNT * 2
    );
    CpuProgressValueFrameFixture {
        tree,
        context,
        next_frame: start + Duration::from_millis(1),
        frame_interval: Duration::from_secs_f64(1.0 / 120.0),
        active_count: PROGRESS_COUNT,
        mode,
    }
}

#[cfg(feature = "bench-support")]
fn bench_progress_value_resolution_cpu(c: &mut Criterion) {
    let mut direct = build_cpu_progress_value_fixture(ProgressValueResolveMode::Direct);
    let mut legacy = build_cpu_progress_value_fixture(ProgressValueResolveMode::LegacyFullVisual);
    let (_, direct_values) = direct.render_frame();
    let (_, legacy_values) = legacy.render_frame();
    assert_eq!(direct_values, legacy_values);

    let mut direct_probe = Vec::with_capacity(64);
    let mut legacy_probe = Vec::with_capacity(64);
    for _ in 0..64 {
        direct_probe.push(direct.render_frame().0);
        legacy_probe.push(legacy.render_frame().0);
    }
    direct_probe.sort_unstable();
    legacy_probe.sort_unstable();
    let direct_p50 = percentile(&direct_probe, 0.50);
    let direct_p95 = percentile(&direct_probe, 0.95);
    let legacy_p50 = percentile(&legacy_probe, 0.50);
    let legacy_p95 = percentile(&legacy_probe, 0.95);
    let reduction = 100.0 * (1.0 - direct_p50.as_secs_f64() / legacy_p50.as_secs_f64());
    eprintln!(
        "progress_value_resolve_cpu_budget: active=1000 direct_p50_ms={:.3} direct_p95_ms={:.3} legacy_p50_ms={:.3} legacy_p95_ms={:.3} median_reduction_pct={:.1} value_equivalent=true fallback=false",
        direct_p50.as_secs_f64() * 1_000.0,
        direct_p95.as_secs_f64() * 1_000.0,
        legacy_p50.as_secs_f64() * 1_000.0,
        legacy_p95.as_secs_f64() * 1_000.0,
        reduction,
    );

    let mut group = c.benchmark_group("progress_value_reactive_resolve_cpu/120Hz");
    group.throughput(Throughput::Elements(1000));
    group.bench_function("direct/1000", |b| {
        b.iter(|| black_box(direct.render_frame()))
    });
    group.bench_function("legacy_full_visual/1000", |b| {
        b.iter(|| black_box(legacy.render_frame()))
    });
    group.finish();
}

#[cfg(feature = "bench-support")]
fn build_cpu_slider_value_fixture(
    mode: SliderValueResolveMode,
    complex_surface: bool,
) -> CpuSliderValueFrameFixture {
    const ELIGIBLE_COUNT: usize = 1000;
    const FALLBACK_COUNT: usize = 256;
    const COLUMNS: usize = 4;

    let slider_count = if complex_surface {
        FALLBACK_COUNT
    } else {
        ELIGIBLE_COUNT
    };
    let view_model = ViewModelContext::for_benchmarks();
    let value = view_model.state(0.37_f32).signal();
    let mut body = Flex::<()>::new(Axis::Vertical)
        .width(dp(1000.0))
        .gap(dp(1.0));
    for row_start in (0..slider_count).step_by(COLUMNS) {
        let mut row = Flex::<()>::new(Axis::Horizontal)
            .size(dp(1000.0), dp(32.0))
            .gap(dp(8.0));
        for _ in row_start..(row_start + COLUMNS).min(slider_count) {
            let mut slider = Slider::<()>::new(value.clone(), 0.0, 1.0)
                .step(0.01)
                .size(dp(240.0), dp(32.0));
            if complex_surface {
                slider = slider.style(|style, context| {
                    style.thumb_shadow = Some(context.theme.elevation.sm.clone());
                });
            }
            row = row.child(slider);
        }
        body = body.child(row);
    }
    let tree = Box::new(WidgetTree::new(body));
    let viewport = Rect::new(
        0.0,
        0.0,
        1000.0,
        (slider_count.div_ceil(COLUMNS) * 33) as f32,
    );
    let mut context = WidgetBenchmarkContext::new().with_viewport(viewport);
    let start = Instant::now();
    let initial = context.run_layout_and_scene(&tree, start);
    assert_eq!(initial.shape_count, slider_count * 3);
    assert_eq!(
        initial.texture_count,
        usize::from(complex_surface) * slider_count
    );
    CpuSliderValueFrameFixture {
        tree,
        context,
        next_frame: start + Duration::from_millis(1),
        frame_interval: Duration::from_secs_f64(1.0 / 120.0),
        active_count: slider_count,
        mode,
        complex_surface,
    }
}

#[cfg(feature = "bench-support")]
fn bench_slider_value_resolution_cpu(c: &mut Criterion) {
    const PROBE_ROUNDS: usize = 3;

    fn probe_pair(
        direct: &mut CpuSliderValueFrameFixture,
        legacy: &mut CpuSliderValueFrameFixture,
    ) -> (Duration, Duration, Duration, Duration) {
        const SAMPLE_COUNT: usize = 192;
        let mut direct_samples = Vec::with_capacity(SAMPLE_COUNT);
        let mut legacy_samples = Vec::with_capacity(SAMPLE_COUNT);
        for sample in 0..SAMPLE_COUNT {
            if sample % 2 == 0 {
                direct_samples.push(direct.render_frame().0);
                legacy_samples.push(legacy.render_frame().0);
            } else {
                legacy_samples.push(legacy.render_frame().0);
                direct_samples.push(direct.render_frame().0);
            }
        }
        direct_samples.sort_unstable();
        legacy_samples.sort_unstable();
        (
            percentile(&direct_samples, 0.50),
            percentile(&direct_samples, 0.95),
            percentile(&legacy_samples, 0.50),
            percentile(&legacy_samples, 0.95),
        )
    }

    let mut direct = build_cpu_slider_value_fixture(SliderValueResolveMode::Direct, false);
    let mut legacy =
        build_cpu_slider_value_fixture(SliderValueResolveMode::LegacyFullVisual, false);
    let (_, direct_values) = direct.render_frame();
    let (_, legacy_values) = legacy.render_frame();
    assert_eq!(direct_values, legacy_values);
    for round in 1..=PROBE_ROUNDS {
        let (direct_p50, direct_p95, legacy_p50, legacy_p95) = probe_pair(&mut direct, &mut legacy);
        let reduction = 100.0 * (1.0 - direct_p50.as_secs_f64() / legacy_p50.as_secs_f64());
        eprintln!(
            "slider_value_resolve_cpu_budget: round={round}/{PROBE_ROUNDS} active=1000 direct_p50_ms={:.3} direct_p95_ms={:.3} legacy_p50_ms={:.3} legacy_p95_ms={:.3} median_reduction_pct={:.1} value_equivalent=true plain_slider=true fallback=false",
            direct_p50.as_secs_f64() * 1_000.0,
            direct_p95.as_secs_f64() * 1_000.0,
            legacy_p50.as_secs_f64() * 1_000.0,
            legacy_p95.as_secs_f64() * 1_000.0,
            reduction,
        );
        assert!(
            reduction >= 5.0,
            "SliderValue direct resolver missed its 5% median reduction gate in round {round}/{PROBE_ROUNDS}: {reduction:.2}%"
        );
    }

    let mut fallback_direct = build_cpu_slider_value_fixture(SliderValueResolveMode::Direct, true);
    let mut fallback_legacy =
        build_cpu_slider_value_fixture(SliderValueResolveMode::LegacyFullVisual, true);
    let (_, fallback_direct_values) = fallback_direct.render_frame();
    let (_, fallback_legacy_values) = fallback_legacy.render_frame();
    assert_eq!(fallback_direct_values, fallback_legacy_values);
    for round in 1..=PROBE_ROUNDS {
        let (fallback_direct_p50, fallback_direct_p95, fallback_legacy_p50, fallback_legacy_p95) =
            probe_pair(&mut fallback_direct, &mut fallback_legacy);
        let fallback_overhead =
            100.0 * (fallback_direct_p50.as_secs_f64() / fallback_legacy_p50.as_secs_f64() - 1.0);
        eprintln!(
            "slider_value_fallback_cpu_budget: round={round}/{PROBE_ROUNDS} active=256 direct_fallback_p50_ms={:.3} direct_fallback_p95_ms={:.3} legacy_p50_ms={:.3} legacy_p95_ms={:.3} overhead_pct={:.1} value_equivalent=true thumb_shadow=true sticky_negative=true fallback=true",
            fallback_direct_p50.as_secs_f64() * 1_000.0,
            fallback_direct_p95.as_secs_f64() * 1_000.0,
            fallback_legacy_p50.as_secs_f64() * 1_000.0,
            fallback_legacy_p95.as_secs_f64() * 1_000.0,
            fallback_overhead,
        );
        assert!(
            fallback_overhead <= 5.0,
            "SliderValue sticky fallback exceeded its 5% median overhead gate in round {round}/{PROBE_ROUNDS}: {fallback_overhead:.2}%"
        );
    }

    let mut eligible_group = c.benchmark_group("slider_value_reactive_resolve_cpu/120Hz");
    eligible_group.throughput(Throughput::Elements(1000));
    eligible_group.bench_function("direct/1000", |b| {
        b.iter(|| black_box(direct.render_frame()))
    });
    eligible_group.bench_function("legacy_full_visual/1000", |b| {
        b.iter(|| black_box(legacy.render_frame()))
    });
    eligible_group.finish();

    let mut fallback_group = c.benchmark_group("slider_value_reactive_fallback_cpu/120Hz");
    fallback_group.throughput(Throughput::Elements(256));
    fallback_group.bench_function("direct_sticky/256", |b| {
        b.iter(|| black_box(fallback_direct.render_frame()))
    });
    fallback_group.bench_function("legacy_full_visual/256", |b| {
        b.iter(|| black_box(fallback_legacy.render_frame()))
    });
    fallback_group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_offset_resolution_cpu(c: &mut Criterion) {
    let mut direct = build_cpu_offset_fixture(OffsetResolveMode::Direct, false);
    let mut legacy = build_cpu_offset_fixture(OffsetResolveMode::LegacyFullVisual, false);
    let verification_time = Instant::now();
    let direct_values = direct
        .context
        .resolve_all_plain_container_offsets_for_benchmark(&direct.tree, verification_time, false);
    let legacy_values = legacy
        .context
        .resolve_all_plain_container_offsets_for_benchmark(&legacy.tree, verification_time, true);
    assert_eq!(direct_values.len(), legacy_values.len());
    for (direct_value, legacy_value) in direct_values.iter().zip(&legacy_values) {
        let (Some(direct_value), Some(legacy_value)) = (direct_value, legacy_value) else {
            panic!("eligible Offset fixtures must resolve every target");
        };
        assert_eq!(direct_value.background, legacy_value.background);
        assert_eq!(direct_value.border, legacy_value.border);
        assert_eq!(direct_value.backdrop_blur, legacy_value.backdrop_blur);
        assert_eq!(direct_value.brush, legacy_value.brush);
        assert_eq!(direct_value.has_texture, legacy_value.has_texture);
        assert_eq!(
            direct_value
                .container_occluder
                .as_ref()
                .map(|(_, rect, clip_rect)| (*rect, *clip_rect)),
            legacy_value
                .container_occluder
                .as_ref()
                .map(|(_, rect, clip_rect)| (*rect, *clip_rect)),
        );
    }

    let mut direct_probe = Vec::with_capacity(64);
    let mut legacy_probe = Vec::with_capacity(64);
    for _ in 0..64 {
        direct_probe.push(direct.render_frame());
        legacy_probe.push(legacy.render_frame());
    }
    direct_probe.sort_unstable();
    legacy_probe.sort_unstable();
    let direct_p50 = percentile(&direct_probe, 0.50);
    let direct_p95 = percentile(&direct_probe, 0.95);
    let legacy_p50 = percentile(&legacy_probe, 0.50);
    let legacy_p95 = percentile(&legacy_probe, 0.95);
    eprintln!(
        "offset_resolve_cpu_budget: active=1000 direct_p50_ms={:.3} direct_p95_ms={:.3} legacy_p50_ms={:.3} legacy_p95_ms={:.3} median_reduction_pct={:.1} value_equivalent=true transform_record=false fallback=false",
        direct_p50.as_secs_f64() * 1_000.0,
        direct_p95.as_secs_f64() * 1_000.0,
        legacy_p50.as_secs_f64() * 1_000.0,
        legacy_p95.as_secs_f64() * 1_000.0,
        100.0 * (1.0 - direct_p50.as_secs_f64() / legacy_p50.as_secs_f64()),
    );

    let mut fallback_direct = build_cpu_offset_fixture(OffsetResolveMode::Direct, true);
    let mut fallback_legacy = build_cpu_offset_fixture(OffsetResolveMode::LegacyFullVisual, true);
    let fallback_time = Instant::now();
    let fallback_direct_values = fallback_direct
        .context
        .resolve_all_plain_container_offsets_for_benchmark(
            &fallback_direct.tree,
            fallback_time,
            false,
        );
    let fallback_legacy_values = fallback_legacy
        .context
        .resolve_all_plain_container_offsets_for_benchmark(
            &fallback_legacy.tree,
            fallback_time,
            true,
        );
    assert_eq!(fallback_direct_values.len(), fallback_legacy_values.len());
    for (direct_value, legacy_value) in fallback_direct_values.iter().zip(&fallback_legacy_values) {
        let (Some(direct_value), Some(legacy_value)) = (direct_value, legacy_value) else {
            panic!("fallback Offset fixtures must resolve every target");
        };
        assert_eq!(direct_value.background, legacy_value.background);
        assert_eq!(direct_value.border, legacy_value.border);
        assert_eq!(direct_value.backdrop_blur, legacy_value.backdrop_blur);
        assert_eq!(direct_value.brush, legacy_value.brush);
        assert_eq!(direct_value.has_texture, legacy_value.has_texture);
        assert_eq!(
            direct_value
                .container_occluder
                .as_ref()
                .map(|(_, rect, clip_rect)| (*rect, *clip_rect)),
            legacy_value
                .container_occluder
                .as_ref()
                .map(|(_, rect, clip_rect)| (*rect, *clip_rect)),
        );
    }
    let mut fallback_direct_probe = Vec::with_capacity(64);
    let mut fallback_legacy_probe = Vec::with_capacity(64);
    for _ in 0..64 {
        fallback_direct_probe.push(fallback_direct.render_frame());
        fallback_legacy_probe.push(fallback_legacy.render_frame());
    }
    fallback_direct_probe.sort_unstable();
    fallback_legacy_probe.sort_unstable();
    let fallback_direct_p50 = percentile(&fallback_direct_probe, 0.50);
    let fallback_legacy_p50 = percentile(&fallback_legacy_probe, 0.50);
    eprintln!(
        "offset_fallback_cpu_budget: active=1000 direct_fallback_p50_ms={:.3} legacy_p50_ms={:.3} overhead_pct={:.1} value_equivalent=true fallback=true",
        fallback_direct_p50.as_secs_f64() * 1_000.0,
        fallback_legacy_p50.as_secs_f64() * 1_000.0,
        100.0 * (fallback_direct_p50.as_secs_f64() / fallback_legacy_p50.as_secs_f64() - 1.0),
    );

    let mut group = c.benchmark_group("offset_reactive_resolve_cpu/120Hz");
    group.throughput(Throughput::Elements(1000));
    group.bench_function("direct/1000", |b| {
        b.iter(|| black_box(direct.render_frame()))
    });
    group.bench_function("legacy_full_visual/1000", |b| {
        b.iter(|| black_box(legacy.render_frame()))
    });
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_scale_resolution_cpu(c: &mut Criterion) {
    fn assert_equivalent(
        direct: &[Option<tgui::widgets::ContainerScaleResolveSnapshot>],
        legacy: &[Option<tgui::widgets::ContainerScaleResolveSnapshot>],
    ) {
        assert_eq!(direct.len(), legacy.len());
        for (direct_value, legacy_value) in direct.iter().zip(legacy) {
            match (direct_value, legacy_value) {
                (None, None) => {}
                (Some(direct_value), Some(legacy_value)) => {
                    assert_eq!(direct_value.background, legacy_value.background);
                    assert_eq!(direct_value.border, legacy_value.border);
                    assert_eq!(direct_value.backdrop_blur, legacy_value.backdrop_blur);
                    assert_eq!(direct_value.brush, legacy_value.brush);
                    assert_eq!(direct_value.has_texture, legacy_value.has_texture);
                    assert_eq!(
                        direct_value
                            .container_occluder
                            .as_ref()
                            .map(|(_, rect, clip)| (*rect, *clip)),
                        legacy_value
                            .container_occluder
                            .as_ref()
                            .map(|(_, rect, clip)| (*rect, *clip)),
                    );
                }
                _ => panic!("Scale direct/legacy fallback topology differs"),
            }
        }
    }

    fn probe_pair(
        direct: &mut CpuScaleFrameFixture,
        legacy: &mut CpuScaleFrameFixture,
    ) -> (Duration, Duration, Duration, Duration) {
        const SAMPLE_COUNT: usize = 256;
        let mut direct_samples = Vec::with_capacity(SAMPLE_COUNT);
        let mut legacy_samples = Vec::with_capacity(SAMPLE_COUNT);
        for sample in 0..SAMPLE_COUNT {
            if sample % 2 == 0 {
                let (direct_elapsed, direct_values) = direct.render_frame();
                let (legacy_elapsed, legacy_values) = legacy.render_frame();
                black_box(direct_values);
                black_box(legacy_values);
                direct_samples.push(direct_elapsed);
                legacy_samples.push(legacy_elapsed);
            } else {
                let (legacy_elapsed, legacy_values) = legacy.render_frame();
                let (direct_elapsed, direct_values) = direct.render_frame();
                black_box(legacy_values);
                black_box(direct_values);
                legacy_samples.push(legacy_elapsed);
                direct_samples.push(direct_elapsed);
            }
        }
        direct_samples.sort_unstable();
        legacy_samples.sort_unstable();
        (
            percentile(&direct_samples, 0.50),
            percentile(&direct_samples, 0.95),
            percentile(&legacy_samples, 0.50),
            percentile(&legacy_samples, 0.95),
        )
    }

    let mut direct = build_cpu_scale_fixture(ScaleResolveMode::Direct, false);
    let mut legacy = build_cpu_scale_fixture(ScaleResolveMode::LegacyFullVisual, false);
    let (_, direct_values) = direct.render_frame();
    let (_, legacy_values) = legacy.render_frame();
    assert_equivalent(&direct_values, &legacy_values);
    let (direct_p50, direct_p95, legacy_p50, legacy_p95) = probe_pair(&mut direct, &mut legacy);
    let reduction = 100.0 * (1.0 - direct_p50.as_secs_f64() / legacy_p50.as_secs_f64());
    eprintln!(
        "scale_resolve_cpu_budget: active=1000 direct_p50_ms={:.3} direct_p95_ms={:.3} legacy_p50_ms={:.3} legacy_p95_ms={:.3} median_reduction_pct={:.1} value_equivalent=true fixed_empty_hidden_solid=true fallback=false",
        direct_p50.as_secs_f64() * 1_000.0,
        direct_p95.as_secs_f64() * 1_000.0,
        legacy_p50.as_secs_f64() * 1_000.0,
        legacy_p95.as_secs_f64() * 1_000.0,
        reduction,
    );
    assert!(
        reduction >= 5.0,
        "Scale direct resolver missed its 5% median reduction gate: {reduction:.2}%"
    );

    let mut fallback_direct = build_cpu_scale_fixture(ScaleResolveMode::Direct, true);
    let mut fallback_legacy = build_cpu_scale_fixture(ScaleResolveMode::LegacyFullVisual, true);
    let (_, fallback_direct_values) = fallback_direct.render_frame();
    let (_, fallback_legacy_values) = fallback_legacy.render_frame();
    assert_equivalent(&fallback_direct_values, &fallback_legacy_values);
    let (fallback_direct_p50, fallback_direct_p95, fallback_legacy_p50, fallback_legacy_p95) =
        probe_pair(&mut fallback_direct, &mut fallback_legacy);
    let fallback_overhead =
        100.0 * (fallback_direct_p50.as_secs_f64() / fallback_legacy_p50.as_secs_f64() - 1.0);
    eprintln!(
        "scale_fallback_cpu_budget: active=256 direct_fallback_p50_ms={:.3} direct_fallback_p95_ms={:.3} legacy_p50_ms={:.3} legacy_p95_ms={:.3} overhead_pct={:.1} value_equivalent=true complex_surface=shadow+brush_interleaved sticky_negative=true fallback=true",
        fallback_direct_p50.as_secs_f64() * 1_000.0,
        fallback_direct_p95.as_secs_f64() * 1_000.0,
        fallback_legacy_p50.as_secs_f64() * 1_000.0,
        fallback_legacy_p95.as_secs_f64() * 1_000.0,
        fallback_overhead,
    );
    assert!(
        fallback_overhead <= 5.0,
        "Scale sticky fallback exceeded its 5% median overhead gate: {fallback_overhead:.2}%"
    );

    let mut eligible_group = c.benchmark_group("scale_reactive_resolve_cpu/120Hz");
    eligible_group.throughput(Throughput::Elements(1000));
    eligible_group.bench_function("direct/1000", |b| {
        b.iter(|| black_box(direct.render_frame()))
    });
    eligible_group.bench_function("legacy_full_visual/1000", |b| {
        b.iter(|| black_box(legacy.render_frame()))
    });
    eligible_group.finish();

    let mut fallback_group = c.benchmark_group("scale_reactive_fallback_cpu/120Hz");
    fallback_group.throughput(Throughput::Elements(256));
    fallback_group.bench_function("direct_sticky/256", |b| {
        b.iter(|| black_box(fallback_direct.render_frame()))
    });
    fallback_group.bench_function("legacy_full_visual/256", |b| {
        b.iter(|| black_box(fallback_legacy.render_frame()))
    });
    fallback_group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_background_brush_resolution_cpu(c: &mut Criterion) {
    fn verify_equivalent(
        direct: &mut CpuBackgroundBrushFrameFixture,
        legacy: &mut CpuBackgroundBrushFrameFixture,
    ) {
        let now = Instant::now();
        let direct_values = direct
            .context
            .resolve_all_plain_container_background_brushes_for_benchmark(&direct.tree, now, false);
        let legacy_values = legacy
            .context
            .resolve_all_plain_container_background_brushes_for_benchmark(&legacy.tree, now, true);
        assert_eq!(direct_values, legacy_values);
    }

    fn probe_pair(
        direct: &mut CpuBackgroundBrushFrameFixture,
        legacy: &mut CpuBackgroundBrushFrameFixture,
    ) -> (Duration, Duration, Duration, Duration) {
        let mut direct_samples = Vec::with_capacity(64);
        let mut legacy_samples = Vec::with_capacity(64);
        for _ in 0..64 {
            direct_samples.push(direct.render_frame());
            legacy_samples.push(legacy.render_frame());
        }
        direct_samples.sort_unstable();
        legacy_samples.sort_unstable();
        (
            percentile(&direct_samples, 0.50),
            percentile(&direct_samples, 0.95),
            percentile(&legacy_samples, 0.50),
            percentile(&legacy_samples, 0.95),
        )
    }

    let mut solid_direct =
        build_cpu_background_brush_fixture(BackgroundBrushResolveMode::Direct, false, false);
    let mut solid_legacy = build_cpu_background_brush_fixture(
        BackgroundBrushResolveMode::LegacyFullVisual,
        false,
        false,
    );
    verify_equivalent(&mut solid_direct, &mut solid_legacy);
    let (solid_direct_p50, solid_direct_p95, solid_legacy_p50, solid_legacy_p95) =
        probe_pair(&mut solid_direct, &mut solid_legacy);

    let mut mixed_direct =
        build_cpu_background_brush_fixture(BackgroundBrushResolveMode::Direct, true, false);
    let mut mixed_legacy = build_cpu_background_brush_fixture(
        BackgroundBrushResolveMode::LegacyFullVisual,
        true,
        false,
    );
    verify_equivalent(&mut mixed_direct, &mut mixed_legacy);
    let (mixed_direct_p50, mixed_direct_p95, mixed_legacy_p50, mixed_legacy_p95) =
        probe_pair(&mut mixed_direct, &mut mixed_legacy);

    eprintln!(
        "background_brush_resolve_cpu_budget: active=1000 solid_direct_p50_ms={:.3} solid_direct_p95_ms={:.3} solid_legacy_p50_ms={:.3} solid_legacy_p95_ms={:.3} solid_reduction_pct={:.1} mixed_direct_p50_ms={:.3} mixed_direct_p95_ms={:.3} mixed_legacy_p50_ms={:.3} mixed_legacy_p95_ms={:.3} mixed_reduction_pct={:.1} primitive_equivalent=true fallback=false",
        solid_direct_p50.as_secs_f64() * 1_000.0,
        solid_direct_p95.as_secs_f64() * 1_000.0,
        solid_legacy_p50.as_secs_f64() * 1_000.0,
        solid_legacy_p95.as_secs_f64() * 1_000.0,
        100.0 * (1.0 - solid_direct_p50.as_secs_f64() / solid_legacy_p50.as_secs_f64()),
        mixed_direct_p50.as_secs_f64() * 1_000.0,
        mixed_direct_p95.as_secs_f64() * 1_000.0,
        mixed_legacy_p50.as_secs_f64() * 1_000.0,
        mixed_legacy_p95.as_secs_f64() * 1_000.0,
        100.0 * (1.0 - mixed_direct_p50.as_secs_f64() / mixed_legacy_p50.as_secs_f64()),
    );

    let mut fallback_direct =
        build_cpu_background_brush_fixture(BackgroundBrushResolveMode::Direct, true, true);
    let mut fallback_legacy = build_cpu_background_brush_fixture(
        BackgroundBrushResolveMode::LegacyFullVisual,
        true,
        true,
    );
    verify_equivalent(&mut fallback_direct, &mut fallback_legacy);
    let (fallback_direct_p50, _, fallback_legacy_p50, _) =
        probe_pair(&mut fallback_direct, &mut fallback_legacy);
    eprintln!(
        "background_brush_fallback_cpu_budget: active=1000 direct_fallback_p50_ms={:.3} legacy_p50_ms={:.3} overhead_pct={:.1} value_equivalent=true fallback=true",
        fallback_direct_p50.as_secs_f64() * 1_000.0,
        fallback_legacy_p50.as_secs_f64() * 1_000.0,
        100.0 * (fallback_direct_p50.as_secs_f64() / fallback_legacy_p50.as_secs_f64() - 1.0),
    );

    let mut group = c.benchmark_group("background_brush_reactive_resolve_cpu/120Hz");
    group.throughput(Throughput::Elements(1000));
    group.bench_function("solid_direct/1000", |b| {
        b.iter(|| black_box(solid_direct.render_frame()))
    });
    group.bench_function("solid_legacy_full_visual/1000", |b| {
        b.iter(|| black_box(solid_legacy.render_frame()))
    });
    group.bench_function("mixed_direct/1000", |b| {
        b.iter(|| black_box(mixed_direct.render_frame()))
    });
    group.bench_function("mixed_legacy_full_visual/1000", |b| {
        b.iter(|| black_box(mixed_legacy.render_frame()))
    });
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_background_blur_resolution_cpu(c: &mut Criterion) {
    let mut direct = build_cpu_background_blur_fixture(BackgroundBlurResolveMode::Direct, false);
    let mut legacy =
        build_cpu_background_blur_fixture(BackgroundBlurResolveMode::LegacyFullVisual, false);
    let verification_time = Instant::now();
    let direct_values = direct
        .context
        .resolve_all_plain_container_background_blurs_for_benchmark(
            &direct.tree,
            verification_time,
            false,
        );
    let legacy_values = legacy
        .context
        .resolve_all_plain_container_background_blurs_for_benchmark(
            &legacy.tree,
            verification_time,
            true,
        );
    assert_eq!(direct_values, legacy_values);

    let mut direct_probe = Vec::with_capacity(64);
    let mut legacy_probe = Vec::with_capacity(64);
    for _ in 0..64 {
        direct_probe.push(direct.render_frame());
        legacy_probe.push(legacy.render_frame());
    }
    direct_probe.sort_unstable();
    legacy_probe.sort_unstable();
    let direct_p50 = percentile(&direct_probe, 0.50);
    let direct_p95 = percentile(&direct_probe, 0.95);
    let legacy_p50 = percentile(&legacy_probe, 0.50);
    let legacy_p95 = percentile(&legacy_probe, 0.95);
    let reduction = 100.0 * (1.0 - direct_p50.as_secs_f64() / legacy_p50.as_secs_f64());
    eprintln!(
        "background_blur_resolve_cpu_budget: active=1000 direct_p50_ms={:.3} direct_p95_ms={:.3} legacy_p50_ms={:.3} legacy_p95_ms={:.3} median_reduction_pct={:.1} primitive_equivalent=true topology_guard=unchanged fallback=false",
        direct_p50.as_secs_f64() * 1_000.0,
        direct_p95.as_secs_f64() * 1_000.0,
        legacy_p50.as_secs_f64() * 1_000.0,
        legacy_p95.as_secs_f64() * 1_000.0,
        reduction,
    );

    let mut fallback_direct =
        build_cpu_background_blur_fixture(BackgroundBlurResolveMode::Direct, true);
    let mut fallback_legacy =
        build_cpu_background_blur_fixture(BackgroundBlurResolveMode::LegacyFullVisual, true);
    let fallback_verification_time = Instant::now();
    let fallback_direct_values = fallback_direct
        .context
        .resolve_all_plain_container_background_blurs_for_benchmark(
            &fallback_direct.tree,
            fallback_verification_time,
            false,
        );
    let fallback_legacy_values = fallback_legacy
        .context
        .resolve_all_plain_container_background_blurs_for_benchmark(
            &fallback_legacy.tree,
            fallback_verification_time,
            true,
        );
    assert_eq!(fallback_direct_values, fallback_legacy_values);
    let mut fallback_direct_probe = Vec::with_capacity(64);
    let mut fallback_legacy_probe = Vec::with_capacity(64);
    for _ in 0..64 {
        fallback_direct_probe.push(fallback_direct.render_frame());
        fallback_legacy_probe.push(fallback_legacy.render_frame());
    }
    fallback_direct_probe.sort_unstable();
    fallback_legacy_probe.sort_unstable();
    let fallback_direct_p50 = percentile(&fallback_direct_probe, 0.50);
    let fallback_legacy_p50 = percentile(&fallback_legacy_probe, 0.50);
    let fallback_overhead =
        100.0 * (fallback_direct_p50.as_secs_f64() / fallback_legacy_p50.as_secs_f64() - 1.0);
    eprintln!(
        "background_blur_fallback_cpu_budget: active=1000 direct_fallback_p50_ms={:.3} legacy_p50_ms={:.3} overhead_pct={:.1} value_equivalent=true fallback=true",
        fallback_direct_p50.as_secs_f64() * 1_000.0,
        fallback_legacy_p50.as_secs_f64() * 1_000.0,
        fallback_overhead,
    );

    let mut group = c.benchmark_group("background_blur_reactive_resolve_cpu/120Hz");
    group.throughput(Throughput::Elements(1000));
    group.bench_function("direct/1000", |b| {
        b.iter(|| black_box(direct.render_frame()))
    });
    group.bench_function("legacy_full_visual/1000", |b| {
        b.iter(|| black_box(legacy.render_frame()))
    });
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_container_opacity_resolution_cpu(c: &mut Criterion) {
    let mut direct =
        build_cpu_container_opacity_fixture(ContainerOpacityResolveMode::Direct, false);
    let mut legacy =
        build_cpu_container_opacity_fixture(ContainerOpacityResolveMode::LegacyFullVisual, false);
    let (_, direct_values) = direct.render_frame();
    let (_, legacy_values) = legacy.render_frame();
    assert_eq!(direct_values, legacy_values);

    let mut direct_probe = Vec::with_capacity(64);
    let mut legacy_probe = Vec::with_capacity(64);
    for _ in 0..64 {
        direct_probe.push(direct.render_frame().0);
        legacy_probe.push(legacy.render_frame().0);
    }
    direct_probe.sort_unstable();
    legacy_probe.sort_unstable();
    let direct_p50 = percentile(&direct_probe, 0.50);
    let direct_p95 = percentile(&direct_probe, 0.95);
    let legacy_p50 = percentile(&legacy_probe, 0.50);
    let legacy_p95 = percentile(&legacy_probe, 0.95);
    let reduction = 100.0 * (1.0 - direct_p50.as_secs_f64() / legacy_p50.as_secs_f64());
    eprintln!(
        "container_opacity_resolve_cpu_budget: active=1000 direct_p50_ms={:.3} direct_p95_ms={:.3} legacy_p50_ms={:.3} legacy_p95_ms={:.3} median_reduction_pct={:.1} value_equivalent=true topology_guard=unchanged fallback=false",
        direct_p50.as_secs_f64() * 1_000.0,
        direct_p95.as_secs_f64() * 1_000.0,
        legacy_p50.as_secs_f64() * 1_000.0,
        legacy_p95.as_secs_f64() * 1_000.0,
        reduction,
    );

    let mut fallback_direct =
        build_cpu_container_opacity_fixture(ContainerOpacityResolveMode::Direct, true);
    let mut fallback_legacy =
        build_cpu_container_opacity_fixture(ContainerOpacityResolveMode::LegacyFullVisual, true);
    let (_, fallback_direct_values) = fallback_direct.render_frame();
    let (_, fallback_legacy_values) = fallback_legacy.render_frame();
    assert_eq!(fallback_direct_values, fallback_legacy_values);
    let mut fallback_direct_probe = Vec::with_capacity(64);
    let mut fallback_legacy_probe = Vec::with_capacity(64);
    for _ in 0..64 {
        fallback_direct_probe.push(fallback_direct.render_frame().0);
        fallback_legacy_probe.push(fallback_legacy.render_frame().0);
    }
    fallback_direct_probe.sort_unstable();
    fallback_legacy_probe.sort_unstable();
    let fallback_direct_p50 = percentile(&fallback_direct_probe, 0.50);
    let fallback_legacy_p50 = percentile(&fallback_legacy_probe, 0.50);
    eprintln!(
        "container_opacity_fallback_cpu_budget: active=1000 direct_fallback_p50_ms={:.3} legacy_p50_ms={:.3} overhead_pct={:.1} value_equivalent=true fallback=true",
        fallback_direct_p50.as_secs_f64() * 1_000.0,
        fallback_legacy_p50.as_secs_f64() * 1_000.0,
        100.0 * (fallback_direct_p50.as_secs_f64() / fallback_legacy_p50.as_secs_f64() - 1.0),
    );

    let mut group = c.benchmark_group("container_opacity_reactive_resolve_cpu/120Hz");
    group.throughput(Throughput::Elements(1000));
    group.bench_function("direct/1000", |b| {
        b.iter(|| black_box(direct.render_frame()))
    });
    group.bench_function("legacy_full_visual/1000", |b| {
        b.iter(|| black_box(legacy.render_frame()))
    });
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_text_opacity_resolution_cpu(c: &mut Criterion) {
    fn probe_pair(
        direct: &mut CpuTextOpacityFrameFixture,
        legacy: &mut CpuTextOpacityFrameFixture,
    ) -> (Duration, Duration, Duration, Duration) {
        const SAMPLE_COUNT: usize = 256;
        let mut direct_samples = Vec::with_capacity(SAMPLE_COUNT);
        let mut legacy_samples = Vec::with_capacity(SAMPLE_COUNT);
        for sample in 0..SAMPLE_COUNT {
            if sample % 2 == 0 {
                direct_samples.push(direct.render_frame().0);
                legacy_samples.push(legacy.render_frame().0);
            } else {
                legacy_samples.push(legacy.render_frame().0);
                direct_samples.push(direct.render_frame().0);
            }
        }
        direct_samples.sort_unstable();
        legacy_samples.sort_unstable();
        (
            percentile(&direct_samples, 0.50),
            percentile(&direct_samples, 0.95),
            percentile(&legacy_samples, 0.50),
            percentile(&legacy_samples, 0.95),
        )
    }

    let mut direct = build_cpu_text_opacity_fixture(TextOpacityResolveMode::Direct, false);
    let mut legacy =
        build_cpu_text_opacity_fixture(TextOpacityResolveMode::LegacyFullVisual, false);
    let (_, direct_values) = direct.render_frame();
    let (_, legacy_values) = legacy.render_frame();
    assert_eq!(direct_values, legacy_values);
    let (direct_p50, direct_p95, legacy_p50, legacy_p95) = probe_pair(&mut direct, &mut legacy);
    let reduction = 100.0 * (1.0 - direct_p50.as_secs_f64() / legacy_p50.as_secs_f64());
    eprintln!(
        "text_opacity_resolve_cpu_budget: active=1000 direct_p50_ms={:.3} direct_p95_ms={:.3} legacy_p50_ms={:.3} legacy_p95_ms={:.3} median_reduction_pct={:.1} value_equivalent=true fallback=false",
        direct_p50.as_secs_f64() * 1_000.0,
        direct_p95.as_secs_f64() * 1_000.0,
        legacy_p50.as_secs_f64() * 1_000.0,
        legacy_p95.as_secs_f64() * 1_000.0,
        reduction,
    );

    let mut fallback_direct = build_cpu_text_opacity_fixture(TextOpacityResolveMode::Direct, true);
    let mut fallback_legacy =
        build_cpu_text_opacity_fixture(TextOpacityResolveMode::LegacyFullVisual, true);
    let (_, fallback_direct_values) = fallback_direct.render_frame();
    let (_, fallback_legacy_values) = fallback_legacy.render_frame();
    assert_eq!(fallback_direct_values, fallback_legacy_values);
    let (fallback_direct_p50, fallback_direct_p95, fallback_legacy_p50, fallback_legacy_p95) =
        probe_pair(&mut fallback_direct, &mut fallback_legacy);
    let fallback_overhead =
        100.0 * (fallback_direct_p50.as_secs_f64() / fallback_legacy_p50.as_secs_f64() - 1.0);
    eprintln!(
        "text_opacity_fallback_cpu_budget: active=1000 direct_fallback_p50_ms={:.3} direct_fallback_p95_ms={:.3} legacy_p50_ms={:.3} legacy_p95_ms={:.3} overhead_pct={:.1} value_equivalent=true complex_surface=shadow+brush sticky_negative=true fallback=true",
        fallback_direct_p50.as_secs_f64() * 1_000.0,
        fallback_direct_p95.as_secs_f64() * 1_000.0,
        fallback_legacy_p50.as_secs_f64() * 1_000.0,
        fallback_legacy_p95.as_secs_f64() * 1_000.0,
        fallback_overhead,
    );

    let mut group = c.benchmark_group("text_opacity_reactive_resolve_cpu/120Hz");
    group.throughput(Throughput::Elements(1000));
    group.bench_function("direct/1000", |b| {
        b.iter(|| black_box(direct.render_frame()))
    });
    group.bench_function("legacy_full_visual/1000", |b| {
        b.iter(|| black_box(legacy.render_frame()))
    });
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_text_color_resolution_cpu(c: &mut Criterion) {
    let mut direct = build_cpu_text_color_fixture(TextColorResolveMode::Direct);
    let mut legacy = build_cpu_text_color_fixture(TextColorResolveMode::LegacyFullVisual);
    let (_, direct_values) = direct.render_frame();
    let (_, legacy_values) = legacy.render_frame();
    assert_eq!(direct_values, legacy_values);

    let mut direct_probe = Vec::with_capacity(64);
    let mut legacy_probe = Vec::with_capacity(64);
    for _ in 0..64 {
        direct_probe.push(direct.render_frame().0);
        legacy_probe.push(legacy.render_frame().0);
    }
    direct_probe.sort_unstable();
    legacy_probe.sort_unstable();
    let direct_p50 = percentile(&direct_probe, 0.50);
    let direct_p95 = percentile(&direct_probe, 0.95);
    let legacy_p50 = percentile(&legacy_probe, 0.50);
    let legacy_p95 = percentile(&legacy_probe, 0.95);
    let reduction = 100.0 * (1.0 - direct_p50.as_secs_f64() / legacy_p50.as_secs_f64());
    eprintln!(
        "text_color_resolve_cpu_budget: active=1000 direct_p50_ms={:.3} direct_p95_ms={:.3} legacy_p50_ms={:.3} legacy_p95_ms={:.3} median_reduction_pct={:.1} value_equivalent=true fallback=false",
        direct_p50.as_secs_f64() * 1_000.0,
        direct_p95.as_secs_f64() * 1_000.0,
        legacy_p50.as_secs_f64() * 1_000.0,
        legacy_p95.as_secs_f64() * 1_000.0,
        reduction,
    );

    let mut group = c.benchmark_group("text_color_reactive_resolve_cpu/120Hz");
    group.throughput(Throughput::Elements(1000));
    group.bench_function("direct/1000", |b| {
        b.iter(|| black_box(direct.render_frame()))
    });
    group.bench_function("legacy_full_visual/1000", |b| {
        b.iter(|| black_box(legacy.render_frame()))
    });
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_texture_mask_tint_resolution_cpu(c: &mut Criterion) {
    let active_filter = std::env::var("TGUI_ANIMATION_ACTIVE")
        .ok()
        .and_then(|value| value.parse::<usize>().ok());
    let active_counts = active_filter
        .map(|active_count| vec![active_count])
        .unwrap_or_else(|| vec![100_usize, 1000]);
    for active_count in active_counts {
        if active_count == 0 || active_count > 1000 {
            continue;
        }
        let mut direct =
            build_cpu_mask_tint_fixture(active_count, 120, MaskTintResolveMode::Direct);
        let mut legacy =
            build_cpu_mask_tint_fixture(active_count, 120, MaskTintResolveMode::LegacyFullVisual);
        assert_cpu_mask_tint_equivalence(&mut direct, &mut legacy);

        let mut direct_probe = Vec::with_capacity(64);
        let mut legacy_probe = Vec::with_capacity(64);
        for _ in 0..64 {
            direct_probe.push(direct.render_frame());
            legacy_probe.push(legacy.render_frame());
        }
        direct_probe.sort_unstable();
        legacy_probe.sort_unstable();
        let direct_p50 = percentile(&direct_probe, 0.50);
        let direct_p95 = percentile(&direct_probe, 0.95);
        let legacy_p50 = percentile(&legacy_probe, 0.50);
        let legacy_p95 = percentile(&legacy_probe, 0.95);
        let reduction = 100.0 * (1.0 - direct_p50.as_secs_f64() / legacy_p50.as_secs_f64());
        eprintln!(
            "texture_mask_tint_resolve_cpu_budget: cadence=120Hz active={} direct_p50_ms={:.3} direct_p95_ms={:.3} legacy_p50_ms={:.3} legacy_p95_ms={:.3} median_reduction_pct={:.1} value_equivalent=true fallback=false",
            active_count,
            direct_p50.as_secs_f64() * 1_000.0,
            direct_p95.as_secs_f64() * 1_000.0,
            legacy_p50.as_secs_f64() * 1_000.0,
            legacy_p95.as_secs_f64() * 1_000.0,
            reduction,
        );

        let mut group = c.benchmark_group("texture_mask_tint_reactive_resolve_cpu/120Hz");
        group.throughput(Throughput::Elements(active_count as u64));
        group.bench_with_input(
            BenchmarkId::new("direct", active_count),
            &active_count,
            |b, _| b.iter(|| black_box(direct.render_frame())),
        );
        group.bench_with_input(
            BenchmarkId::new("legacy_full_visual", active_count),
            &active_count,
            |b, _| b.iter(|| black_box(legacy.render_frame())),
        );
        group.finish();
    }
}

#[cfg(feature = "bench-support")]
fn bench_texture_mask_tint_animation(c: &mut Criterion) {
    let probe_only = std::env::var_os("TGUI_ANIMATION_PROBE_ONLY").is_some();
    let active_filter = std::env::var("TGUI_ANIMATION_ACTIVE")
        .ok()
        .and_then(|value| value.parse::<usize>().ok());
    let active_counts = active_filter
        .map(|active_count| vec![active_count])
        .unwrap_or_else(|| vec![1_usize, 100, 1000]);
    for active_count in active_counts {
        if active_count > 1000 {
            continue;
        }
        let Some(mut retained) = build_mask_tint_fixture(active_count, 120, MaskTintMode::Retained)
        else {
            return;
        };
        let Some(mut full) =
            build_mask_tint_fixture(active_count, 120, MaskTintMode::FullRecollect)
        else {
            return;
        };
        assert_mask_tint_equivalence(&mut retained, &mut full);

        let mut retained_probe = Vec::with_capacity(32);
        let mut full_probe = Vec::with_capacity(32);
        for _ in 0..32 {
            retained_probe.push(retained.render_frame());
            full_probe.push(full.render_frame());
        }
        retained_probe.sort_unstable();
        full_probe.sort_unstable();
        let retained_p50 = percentile(&retained_probe, 0.50);
        let retained_p95 = percentile(&retained_probe, 0.95);
        let full_p50 = percentile(&full_probe, 0.50);
        let full_p95 = percentile(&full_probe, 0.95);
        let reduction = 100.0 * (1.0 - retained_p50.as_secs_f64() / full_p50.as_secs_f64());
        let prepare = retained
            .context
            .headless_gpu_prepare_stats()
            .expect("retained mask-tint prepare stats");
        eprintln!(
            "texture_mask_tint_animation_budget: path=retained_single_draw_patch_vs_full_scene_recollect cadence=120Hz active={} retained_p50_ms={:.3} retained_p95_ms={:.3} full_p50_ms={:.3} full_p95_ms={:.3} median_reduction_pct={:.1} prepare_total={} prepare_rebuild={} prepare_reuse={} pixel_equivalent=true identity_frame_media_serial_stable=true fallback=false",
            active_count,
            retained_p50.as_secs_f64() * 1_000.0,
            retained_p95.as_secs_f64() * 1_000.0,
            full_p50.as_secs_f64() * 1_000.0,
            full_p95.as_secs_f64() * 1_000.0,
            reduction,
            prepare.total_commands,
            prepare.rebuilt_commands,
            prepare.reused_commands,
        );

        if probe_only {
            continue;
        }
        let mut group = c.benchmark_group("texture_mask_tint_animation_gpu_submit/120Hz");
        group.throughput(Throughput::Elements(active_count as u64));
        group.bench_with_input(
            BenchmarkId::new("retained_single_draw_patch", active_count),
            &active_count,
            |b, _| b.iter(|| black_box(retained.render_frame())),
        );
        group.bench_with_input(
            BenchmarkId::new("forced_full_scene_recollect", active_count),
            &active_count,
            |b, _| b.iter(|| black_box(full.render_frame())),
        );
        group.finish();
    }
}

#[cfg(feature = "bench-support")]
fn build_shadow_opacity_fixture(active_count: usize, legacy: bool) -> ShadowOpacityFrameFixture {
    const COLUMNS: usize = 10;
    let view_model = ViewModelContext::for_benchmarks();
    let mut states = Vec::with_capacity(active_count);
    let mut body = Flex::<()>::new(Axis::Vertical).gap(dp(8.0));
    for row_start in (0..active_count).step_by(COLUMNS) {
        let mut row = Flex::<()>::new(Axis::Horizontal)
            .height(dp(52.0))
            .gap(dp(8.0));
        for _ in row_start..(row_start + COLUMNS).min(active_count) {
            let state = view_model.state(0.35_f32);
            let signal = state.signal().animated(transition());
            states.push(state);
            row = row.child(
                Flex::<()>::new(Axis::Horizontal)
                    .size(dp(72.0), dp(44.0))
                    .opacity(signal)
                    .style(|style, context| {
                        style.surface.background = Some(context.theme.colors.surface.into());
                        style.surface.border_radius = Some(context.theme.radius.lg.into());
                        style.surface.shadow = Some(context.theme.elevation.md.clone().into());
                    }),
            );
        }
        body = body.child(row);
    }
    let rows = active_count.div_ceil(COLUMNS);
    let tree = Box::new(WidgetTree::new(body));
    let mut context = WidgetBenchmarkContext::new().with_viewport(Rect::new(
        0.0,
        0.0,
        840.0,
        rows as f32 * 60.0 + 8.0,
    ));
    context.set_force_legacy_widget_shadow_opacity(legacy);
    let start = Instant::now();
    let stats = context.run_layout_and_scene(&tree, start);
    assert_eq!(stats.texture_count, active_count);
    let canonical_texture_ids = context
        .cached_texture_retained_state()
        .expect("initial shadow opacity state")
        .texture_ids;
    for state in states {
        state.set(1.0);
    }
    let animation_start = start + Duration::from_millis(1);
    context.invalidate_all();
    let stats = context.run_layout_and_scene(&tree, animation_start);
    assert_eq!(stats.texture_count, active_count);
    assert!(context.has_active_property_animations());
    ShadowOpacityFrameFixture {
        tree,
        context,
        next_frame: animation_start + Duration::from_secs_f64(1.0 / 120.0),
        frame_interval: Duration::from_secs_f64(1.0 / 120.0),
        active_count,
        legacy,
        canonical_texture_ids,
    }
}

#[cfg(feature = "bench-support")]
fn build_canvas_shadow_opacity_fixture(
    active_count: usize,
    legacy: bool,
) -> CanvasShadowOpacityFrameFixture {
    const COLUMNS: usize = 10;
    let view_model = ViewModelContext::for_benchmarks();
    let mut states = Vec::with_capacity(active_count);
    let mut body = Flex::<()>::new(Axis::Vertical).gap(dp(8.0));
    for row_start in (0..active_count).step_by(COLUMNS) {
        let mut row = Flex::<()>::new(Axis::Horizontal)
            .height(dp(52.0))
            .gap(dp(8.0));
        for index in row_start..(row_start + COLUMNS).min(active_count) {
            let state = view_model.state(0.35_f32);
            let signal = state.signal().animated(transition());
            states.push(state);
            let scene = CanvasRecorder::build(move |canvas| {
                let bend = 22.0 + (index % 5) as f32;
                canvas
                    .set_fill(Color::rgba(248, 250, 252, 255))
                    .set_shadow(CanvasShadow::new(
                        Color::rgba(15, 23, 42, 176),
                        Point::new(dp(0.0), dp(4.0)),
                        dp(8.0),
                    ))
                    .begin_path()
                    .move_to(5.0, 8.0)
                    .quad_to(bend, 0.0, 52.0, 8.0)
                    .line_to(58.0, 34.0)
                    .quad_to(30.0, 44.0, 4.0, 34.0)
                    .close_path()
                    .fill();
            });
            row = row.child(
                Canvas::new(scene)
                    .size(dp(64.0), dp(44.0))
                    .style(move |style, _| style.surface.opacity = signal.clone().into()),
            );
        }
        body = body.child(row);
    }
    let rows = active_count.div_ceil(COLUMNS);
    let tree = Box::new(WidgetTree::new(body));
    let mut context = WidgetBenchmarkContext::new().with_viewport(Rect::new(
        0.0,
        0.0,
        760.0,
        rows as f32 * 60.0 + 8.0,
    ));
    context.set_force_legacy_canvas_shadow_opacity(legacy);
    let start = Instant::now();
    let stats = context.run_layout_and_scene(&tree, start);
    assert_eq!(stats.texture_count, active_count);
    let canonical_texture_ids = context
        .cached_texture_retained_state()
        .expect("initial canvas shadow opacity state")
        .texture_ids;
    for state in states {
        state.set(1.0);
    }
    let animation_start = start + Duration::from_millis(1);
    context.invalidate_all();
    let stats = context.run_layout_and_scene(&tree, animation_start);
    assert_eq!(stats.texture_count, active_count);
    assert!(context.has_active_property_animations());
    CanvasShadowOpacityFrameFixture {
        tree,
        context,
        next_frame: animation_start + Duration::from_secs_f64(1.0 / 120.0),
        frame_interval: Duration::from_secs_f64(1.0 / 120.0),
        active_count,
        legacy,
        canonical_texture_ids,
    }
}

#[cfg(feature = "bench-support")]
fn bench_canvas_shadow_opacity_cpu(c: &mut Criterion) {
    let active_count = std::env::var("TGUI_ANIMATION_ACTIVE")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(100)
        .min(1000);
    let mut canonical = build_canvas_shadow_opacity_fixture(active_count, false);
    let mut legacy = build_canvas_shadow_opacity_fixture(active_count, true);
    let mut canonical_probe = Vec::with_capacity(80);
    let mut legacy_probe = Vec::with_capacity(80);
    for _ in 0..80 {
        canonical_probe.push(canonical.render_frame());
        legacy_probe.push(legacy.render_frame());
    }
    canonical_probe.sort_unstable();
    legacy_probe.sort_unstable();
    let canonical_p50 = percentile(&canonical_probe, 0.50);
    let canonical_p95 = percentile(&canonical_probe, 0.95);
    let legacy_p50 = percentile(&legacy_probe, 0.50);
    let legacy_p95 = percentile(&legacy_probe, 0.95);
    eprintln!(
        "canvas_shadow_opacity_cpu: active={} canonical_p50_ms={:.4} canonical_p95_ms={:.4} legacy_p50_ms={:.4} legacy_p95_ms={:.4} p50_reduction_pct={:.2} p95_reduction_pct={:.2} canonical_texture_identity_stable=true scale_semantics_unchanged=true",
        active_count,
        canonical_p50.as_secs_f64() * 1_000.0,
        canonical_p95.as_secs_f64() * 1_000.0,
        legacy_p50.as_secs_f64() * 1_000.0,
        legacy_p95.as_secs_f64() * 1_000.0,
        100.0 * (1.0 - canonical_p50.as_secs_f64() / legacy_p50.as_secs_f64()),
        100.0 * (1.0 - canonical_p95.as_secs_f64() / legacy_p95.as_secs_f64()),
    );
    if std::env::var_os("TGUI_ANIMATION_PROBE_ONLY").is_some() {
        return;
    }
    let mut group = c.benchmark_group("canvas_shadow_opacity_cpu/120Hz");
    group.throughput(Throughput::Elements(active_count as u64));
    group.bench_function("canonical_texture_plus_primitive_opacity", |b| {
        b.iter(|| black_box(canonical.render_frame()))
    });
    group.bench_function("legacy_baked_opacity", |b| {
        b.iter(|| black_box(legacy.render_frame()))
    });
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_shadow_opacity_cpu(c: &mut Criterion) {
    let active_count = std::env::var("TGUI_ANIMATION_ACTIVE")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(100)
        .min(1000);
    let mut canonical = build_shadow_opacity_fixture(active_count, false);
    let mut legacy = build_shadow_opacity_fixture(active_count, true);
    let mut canonical_probe = Vec::with_capacity(80);
    let mut legacy_probe = Vec::with_capacity(80);
    for _ in 0..80 {
        canonical_probe.push(canonical.render_frame());
        legacy_probe.push(legacy.render_frame());
    }
    canonical_probe.sort_unstable();
    legacy_probe.sort_unstable();
    let canonical_p50 = percentile(&canonical_probe, 0.50);
    let canonical_p95 = percentile(&canonical_probe, 0.95);
    let legacy_p50 = percentile(&legacy_probe, 0.50);
    let legacy_p95 = percentile(&legacy_probe, 0.95);
    eprintln!(
        "widget_shadow_opacity_cpu: active={} canonical_p50_ms={:.4} canonical_p95_ms={:.4} legacy_p50_ms={:.4} legacy_p95_ms={:.4} p50_reduction_pct={:.2} p95_reduction_pct={:.2} canonical_texture_identity_stable=true scale_semantics_unchanged=true",
        active_count,
        canonical_p50.as_secs_f64() * 1_000.0,
        canonical_p95.as_secs_f64() * 1_000.0,
        legacy_p50.as_secs_f64() * 1_000.0,
        legacy_p95.as_secs_f64() * 1_000.0,
        100.0 * (1.0 - canonical_p50.as_secs_f64() / legacy_p50.as_secs_f64()),
        100.0 * (1.0 - canonical_p95.as_secs_f64() / legacy_p95.as_secs_f64()),
    );
    if std::env::var_os("TGUI_ANIMATION_PROBE_ONLY").is_some() {
        return;
    }
    let mut group = c.benchmark_group("widget_shadow_opacity_cpu/120Hz");
    group.throughput(Throughput::Elements(active_count as u64));
    group.bench_function("canonical_texture_plus_primitive_opacity", |b| {
        b.iter(|| black_box(canonical.render_frame()))
    });
    group.bench_function("legacy_baked_opacity", |b| {
        b.iter(|| black_box(legacy.render_frame()))
    });
    group.finish();
}

#[cfg(feature = "bench-support")]
fn bench_transform_translate_prepare_cpu(c: &mut Criterion) {
    fn sample(
        probe: &mut BenchTransformTranslatePrepareProbe,
        cached: bool,
        samples: usize,
    ) -> Vec<Duration> {
        let mut durations = Vec::with_capacity(samples);
        for _ in 0..samples {
            let started = Instant::now();
            let checksum = if cached {
                probe.run_cached()
            } else {
                probe.run_direct()
            };
            black_box(checksum);
            durations.push(started.elapsed());
        }
        durations.sort_unstable();
        durations
    }

    for (draw_count, chain_depth, distinct_chains) in [
        (1_000_usize, 1_usize, 1_usize),
        (10_000, 2, 1),
        (10_000, 2, 4),
    ] {
        let mut cached_probe =
            BenchTransformTranslatePrepareProbe::new(draw_count, chain_depth, distinct_chains);
        let mut direct_probe =
            BenchTransformTranslatePrepareProbe::new(draw_count, chain_depth, distinct_chains);
        assert_eq!(cached_probe.run_cached(), direct_probe.run_direct());
        for _ in 0..16 {
            black_box(cached_probe.run_cached());
            black_box(direct_probe.run_direct());
        }
        let cached_samples = sample(&mut cached_probe, true, 96);
        let direct_samples = sample(&mut direct_probe, false, 96);
        let cached_p50 = percentile(&cached_samples, 0.50);
        let cached_p95 = percentile(&cached_samples, 0.95);
        let direct_p50 = percentile(&direct_samples, 0.50);
        let direct_p95 = percentile(&direct_samples, 0.95);
        eprintln!(
            "transform_translate_prepare_cpu: draws={} depth={} distinct={} cached_p50_ms={:.4} cached_p95_ms={:.4} direct_p50_ms={:.4} direct_p95_ms={:.4} p50_reduction_pct={:.1} p95_reduction_pct={:.1}",
            draw_count,
            chain_depth,
            distinct_chains,
            cached_p50.as_secs_f64() * 1_000.0,
            cached_p95.as_secs_f64() * 1_000.0,
            direct_p50.as_secs_f64() * 1_000.0,
            direct_p95.as_secs_f64() * 1_000.0,
            100.0 * (direct_p50.as_secs_f64() - cached_p50.as_secs_f64())
                / direct_p50.as_secs_f64(),
            100.0 * (direct_p95.as_secs_f64() - cached_p95.as_secs_f64())
                / direct_p95.as_secs_f64(),
        );

        let mut cached =
            BenchTransformTranslatePrepareProbe::new(draw_count, chain_depth, distinct_chains);
        let direct =
            BenchTransformTranslatePrepareProbe::new(draw_count, chain_depth, distinct_chains);
        let mut group = c.benchmark_group("transform_translate_prepare_cpu/120Hz");
        group.throughput(Throughput::Elements(draw_count as u64));
        let parameter = format!("draws_{draw_count}_depth_{chain_depth}_chains_{distinct_chains}");
        group.bench_with_input(
            BenchmarkId::new("cached", &parameter),
            &draw_count,
            |b, _| b.iter(|| black_box(cached.run_cached())),
        );
        group.bench_with_input(
            BenchmarkId::new("direct_control", &parameter),
            &draw_count,
            |b, _| b.iter(|| black_box(direct.run_direct())),
        );
        group.finish();
    }
}

#[cfg(feature = "bench-support")]
fn bench_animation_frame_pipeline(c: &mut Criterion) {
    macro_rules! run_only {
        ($environment:literal, $bench:ident) => {
            if std::env::var_os($environment).is_some() {
                $bench(c);
                return;
            }
        };
    }
    run_only!(
        "TGUI_ANIMATION_OFFSET_CPU_ONLY",
        bench_offset_resolution_cpu
    );
    run_only!("TGUI_ANIMATION_SCALE_CPU_ONLY", bench_scale_resolution_cpu);
    run_only!(
        "TGUI_ANIMATION_BACKGROUND_BRUSH_CPU_ONLY",
        bench_background_brush_resolution_cpu
    );
    run_only!(
        "TGUI_ANIMATION_BACKGROUND_BLUR_CPU_ONLY",
        bench_background_blur_resolution_cpu
    );
    run_only!(
        "TGUI_ANIMATION_CONTAINER_OPACITY_CPU_ONLY",
        bench_container_opacity_resolution_cpu
    );
    run_only!(
        "TGUI_ANIMATION_TEXT_OPACITY_CPU_ONLY",
        bench_text_opacity_resolution_cpu
    );
    run_only!(
        "TGUI_ANIMATION_BORDER_WIDTH_CPU_ONLY",
        bench_border_width_resolution_cpu
    );
    run_only!(
        "TGUI_ANIMATION_BORDER_RADIUS_CPU_ONLY",
        bench_border_radius_resolution_cpu
    );
    run_only!(
        "TGUI_ANIMATION_BORDER_COLOR_CPU_ONLY",
        bench_border_color_resolution_cpu
    );
    run_only!(
        "TGUI_ANIMATION_BACKGROUND_CPU_ONLY",
        bench_background_resolution_cpu
    );
    run_only!(
        "TGUI_ANIMATION_PROGRESS_VALUE_CPU_ONLY",
        bench_progress_value_resolution_cpu
    );
    run_only!(
        "TGUI_ANIMATION_SLIDER_VALUE_CPU_ONLY",
        bench_slider_value_resolution_cpu
    );
    run_only!(
        "TGUI_ANIMATION_CANVAS_SHADOW_OPACITY_ONLY",
        bench_canvas_shadow_opacity_cpu
    );
    run_only!(
        "TGUI_ANIMATION_SHADOW_OPACITY_ONLY",
        bench_shadow_opacity_cpu
    );
    run_only!(
        "TGUI_ANIMATION_TEXT_COLOR_CPU_ONLY",
        bench_text_color_resolution_cpu
    );
    run_only!(
        "TGUI_ANIMATION_MASK_TINT_CPU_ONLY",
        bench_texture_mask_tint_resolution_cpu
    );
    run_only!(
        "TGUI_ANIMATION_MASK_TINT_ONLY",
        bench_texture_mask_tint_animation
    );

    bench_offset_resolution_cpu(c);
    bench_scale_resolution_cpu(c);
    bench_background_brush_resolution_cpu(c);
    bench_background_blur_resolution_cpu(c);
    bench_container_opacity_resolution_cpu(c);
    bench_text_opacity_resolution_cpu(c);
    bench_border_width_resolution_cpu(c);
    bench_border_radius_resolution_cpu(c);
    bench_border_color_resolution_cpu(c);
    bench_background_resolution_cpu(c);
    bench_progress_value_resolution_cpu(c);
    bench_slider_value_resolution_cpu(c);
    bench_transform_translate_prepare_cpu(c);
    bench_canvas_shadow_opacity_cpu(c);
    bench_shadow_opacity_cpu(c);
    bench_text_color_resolution_cpu(c);
    bench_texture_mask_tint_resolution_cpu(c);
    bench_texture_mask_tint_animation(c);
    let probe_only = std::env::var_os("TGUI_ANIMATION_PROBE_ONLY").is_some();
    let scene_only = std::env::var_os("TGUI_ANIMATION_SCENE_ONLY").is_some();
    let property_filter = std::env::var("TGUI_ANIMATION_PROPERTY").ok();
    let hz_filter = std::env::var("TGUI_ANIMATION_HZ")
        .ok()
        .and_then(|value| value.parse::<u32>().ok());
    let active_filter = std::env::var("TGUI_ANIMATION_ACTIVE")
        .ok()
        .and_then(|value| value.parse::<usize>().ok());
    let active_counts = active_filter
        .map(|active_count| vec![active_count])
        .unwrap_or_else(|| vec![1_usize, 100, 1000]);
    for property in AnimatedProperty::ALL {
        if scene_only && property.affects_layout() {
            continue;
        }
        if property_filter
            .as_deref()
            .is_some_and(|filter| filter != property.name())
        {
            continue;
        }
        for hz in [60_u32, 120] {
            if hz_filter.is_some_and(|filter| filter != hz) {
                continue;
            }
            let budget = Duration::from_secs_f64(1.0 / hz as f64);
            for active_count in active_counts.iter().copied() {
                if active_count > 1000 {
                    continue;
                }
                let Some(mut fixture) = build_fixture(property, active_count, hz) else {
                    return;
                };

                let mut probe = Vec::with_capacity(32);
                let mut resolve_probe = Vec::with_capacity(32);
                for _ in 0..32 {
                    let sample = fixture.render_frame();
                    probe.push(sample.total);
                    resolve_probe.push(sample.reactive_resolve);
                }
                probe.sort_unstable();
                resolve_probe.sort_unstable();
                let p50 = percentile(&probe, 0.50);
                let p95 = percentile(&probe, 0.95);
                let max = *probe.last().expect("animation probe samples");
                let resolve_p50 = percentile(&resolve_probe, 0.50);
                let resolve_p95 = percentile(&resolve_probe, 0.95);
                let prepare = fixture
                    .context
                    .headless_gpu_prepare_stats()
                    .expect("headless prepare stats");
                let liveness = fixture
                    .context
                    .headless_gpu_cache_liveness_stats()
                    .expect("headless cache-liveness stats");
                eprintln!(
                    "animation_frame_budget: property={} path={} resolve_mode={} cache_liveness_mode={} cadence={}Hz active={} budget_ms={:.3} p50_ms={:.3} p95_ms={:.3} max_ms={:.3} resolve_p50_ms={:.3} resolve_p95_ms={:.3} p95_budget_pct={:.1}% max_budget_pct={:.1}% prepare_total={} prepare_rebuild={} prepare_reuse={} liveness_scans={} liveness_paint_skips={} fallback=false",
                    property.name(),
                    if fixture.full_layout_rebuild { "production_full_rebuild" } else if property.affects_layout() { "retained_layout_patch_candidate" } else { "reactive_property_slot" },
                    if std::env::var("TGUI_ANIMATION_REACTIVE_RESOLVE_MODE").as_deref() == Ok("individual") { "individual_control" } else { "batch" },
                    if std::env::var("TGUI_ANIMATION_CACHE_LIVENESS_MODE").as_deref() == Ok("legacy") { "legacy_dirty_draw" } else { "key_dirty" },
                    hz,
                    active_count,
                    budget.as_secs_f64() * 1_000.0,
                    p50.as_secs_f64() * 1_000.0,
                    p95.as_secs_f64() * 1_000.0,
                    max.as_secs_f64() * 1_000.0,
                    resolve_p50.as_secs_f64() * 1_000.0,
                    resolve_p95.as_secs_f64() * 1_000.0,
                    100.0 * p95.as_secs_f64() / budget.as_secs_f64(),
                    100.0 * max.as_secs_f64() / budget.as_secs_f64(),
                    prepare.total_commands,
                    prepare.rebuilt_commands,
                    prepare.reused_commands,
                    liveness.scans,
                    liveness.paint_only_skips,
                );

                if probe_only {
                    continue;
                }

                let mut group = c.benchmark_group(format!(
                    "animation_frame_gpu_submit/{}Hz/{}",
                    hz,
                    property.name()
                ));
                group.throughput(Throughput::Elements(active_count as u64));
                group.bench_with_input(
                    BenchmarkId::from_parameter(active_count),
                    &active_count,
                    |b, _| {
                        b.iter(|| {
                            black_box(fixture.render_frame());
                        });
                    },
                );
                group.finish();
            }
        }
    }
}

#[cfg(not(feature = "bench-support"))]
fn bench_animation_frame_pipeline(_c: &mut Criterion) {
    eprintln!("Skipping animation frame pipeline benchmarks: bench-support feature not enabled");
}

criterion_group!(benches, bench_animation_frame_pipeline);
criterion_main!(benches);
