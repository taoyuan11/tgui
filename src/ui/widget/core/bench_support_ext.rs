// bench_support 扩展 - 为基准测试提供简化的辅助函数
// 这些函数封装了 tgui 内部类型，让基准测试代码更简洁

#![allow(dead_code, unused_variables)]

use std::time::Instant;

use crate::foundation::binding::{State, Signal, ViewModelContext};
use crate::ui::layout::Axis;
use crate::ui::unit::dp;
use crate::ui::widget::{Flex, Rect, Text, WidgetTree};

#[cfg(feature = "bench-support")]
use super::bench_support::{WidgetBenchmarkContext, WidgetBenchmarkStats};

// ============================================================
// State/Signal Helpers (for benchmarks)
// ============================================================

/// 创建一个用于基准测试的 ViewModelContext
pub fn create_bench_context() -> ViewModelContext {
    ViewModelContext::for_benchmarks()
}

/// 为基准测试创建 State
pub fn create_bench_state<T>(ctx: &ViewModelContext, value: T) -> State<T> {
    ctx.state(value)
}

/// 为基准测试创建 Signal
pub fn create_bench_signal<T: Clone + Send + Sync + 'static>(state: &State<T>) -> Signal<T> {
    state.signal()
}

// ============================================================
// Text Processing Helpers
// ============================================================

pub fn shape_text(text: &str, _font_size: f32) -> usize {
    // 简化版：返回字符数作为近似
    text.chars().count()
}

pub fn layout_text(text: &str, width: f32, font_size: f32) -> Vec<String> {
    // 简化版文本布局，返回行数组
    let lines: Vec<_> = text.lines().map(|s| s.to_string()).collect();
    lines
}

pub fn measure_text(text: &str, font_size: f32) -> (f32, f32) {
    // 简化版：近似计算
    let width = text.len() as f32 * font_size * 0.6;
    let height = font_size * 1.2;
    (width, height)
}

pub fn text_hit_test(layout: &[String], pos: (f32, f32)) -> usize {
    let line_height = 16.0;
    let line_index = (pos.1 / line_height) as usize;
    line_index.min(layout.len().saturating_sub(1))
}

pub struct TextController {
    text: String,
}

pub fn create_text_controller(text: &str) -> TextController {
    TextController {
        text: text.to_string(),
    }
}

impl TextController {
    pub fn insert_at(&mut self, pos: usize, s: &str) {
        if pos <= self.text.len() {
            self.text.insert_str(pos, s);
        }
    }

    pub fn delete_range(&mut self, start: usize, end: usize) {
        if start < end && end <= self.text.len() {
            self.text.drain(start..end);
        }
    }
}

pub fn select_text_range(layout: &[String], start: usize, end: usize) -> Vec<Rect> {
    let line_height = 16.0;
    let mut rects = Vec::new();

    let mut char_index = 0;
    for (line_idx, line) in layout.iter().enumerate() {
        let line_start = char_index;
        let line_end = char_index + line.len();

        if end > line_start && start < line_end {
            let sel_start = start.max(line_start) - line_start;
            let sel_end = end.min(line_end) - line_start;

            rects.push(Rect::new(
                sel_start as f32 * 8.0,
                line_idx as f32 * line_height,
                (sel_end - sel_start) as f32 * 8.0,
                line_height,
            ));
        }

        char_index = line_end + 1;
    }

    rects
}

// ============================================================
// Widget Layout Helpers
// ============================================================

pub fn create_nested_element_tree(depth: usize) -> WidgetTree<()> {
    fn build_nested(depth: usize) -> Flex<()> {
        if depth == 0 {
            return Flex::new(Axis::Vertical)
                .width(dp(100.0))
                .height(dp(50.0))
                .child(Text::new("Leaf"));
        }

        Flex::new(Axis::Vertical)
            .width(dp(100.0))
            .child(build_nested(depth - 1))
    }

    WidgetTree::new(build_nested(depth))
}

pub fn create_flat_element_tree(count: usize) -> WidgetTree<()> {
    let mut container = Flex::new(Axis::Vertical).width(dp(800.0)).gap(dp(4.0));

    for i in 0..count {
        container = container.child(
            Flex::new(Axis::Horizontal)
                .width(dp(780.0))
                .height(dp(40.0))
                .child(Text::new(format!("Item {}", i)))
        );
    }

    WidgetTree::new(container)
}

pub fn create_flex_container(children: usize) -> WidgetTree<()> {
    let mut container = Flex::new(Axis::Horizontal)
        .width(dp(800.0))
        .gap(dp(8.0));

    for i in 0..children {
        container = container.child(
            Flex::new(Axis::Vertical)
                .width(dp(150.0))
                .height(dp(200.0))
                .child(Text::new(format!("Child {}", i)))
        );
    }

    WidgetTree::new(container)
}

pub fn create_grid_layout(rows: usize, cols: usize) -> WidgetTree<()> {
    let mut grid = Flex::new(Axis::Vertical).width(dp(800.0)).gap(dp(4.0));

    for row in 0..rows {
        let mut row_flex = Flex::new(Axis::Horizontal).width(dp(800.0)).gap(dp(4.0));

        for col in 0..cols {
            row_flex = row_flex.child(
                Flex::new(Axis::Vertical)
                    .width(dp(100.0))
                    .height(dp(80.0))
                    .child(Text::new(format!("{},{}", row, col)))
            );
        }

        grid = grid.child(row_flex);
    }

    WidgetTree::new(grid)
}

pub fn create_mixed_complex_layout() -> WidgetTree<()> {
    WidgetTree::new(
        Flex::new(Axis::Vertical)
            .width(dp(1920.0))
            .height(dp(1080.0))
            .gap(dp(8.0))
            .child(
                Flex::new(Axis::Horizontal)
                    .width(dp(1920.0))
                    .height(dp(60.0))
                    .gap(dp(16.0))
                    .child(Text::new("Header"))
            )
            .child(
                Flex::new(Axis::Horizontal)
                    .width(dp(1920.0))
                    .gap(dp(16.0))
                    .child(
                        Flex::new(Axis::Vertical)
                            .width(dp(300.0))
                            .gap(dp(8.0))
                            .child(Text::new("Sidebar 1"))
                            .child(Text::new("Sidebar 2"))
                            .child(Text::new("Sidebar 3"))
                    )
                    .child(
                        Flex::new(Axis::Vertical)
                            .width(dp(1300.0))
                            .gap(dp(8.0))
                            .child(Text::new("Main Content"))
                    )
                    .child(
                        Flex::new(Axis::Vertical)
                            .width(dp(300.0))
                            .gap(dp(8.0))
                            .child(Text::new("Right Panel"))
                    )
            )
    )
}

pub fn compute_layout(tree: &WidgetTree<()>, viewport: (f32, f32)) -> WidgetBenchmarkStats {
    let mut ctx = WidgetBenchmarkContext::new()
        .with_viewport(Rect::new(0.0, 0.0, viewport.0, viewport.1));
    ctx.run_layout(tree, Instant::now())
}

pub fn collect_scene_primitives(stats: &WidgetBenchmarkStats) -> usize {
    stats.shape_count + stats.text_count + stats.texture_count
}

pub fn perform_hit_test(_stats: &WidgetBenchmarkStats, _pos: (f32, f32)) -> bool {
    // 简化的命中测试
    true
}

pub fn invalidate_single_widget(_stats: &mut WidgetBenchmarkStats, _widget_idx: usize) {
    // 标记失效
}

pub fn recompute_layout(_stats: &WidgetBenchmarkStats, _viewport: (f32, f32)) -> WidgetBenchmarkStats {
    *_stats
}

// ============================================================
// Scene Rendering Helpers
// ============================================================

pub fn build_scene_graph(node_count: usize) -> Vec<SceneNode> {
    (0..node_count)
        .map(|i| SceneNode {
            id: i,
            z_order: i,
            rect: Rect::new(0.0, i as f32 * 10.0, 100.0, 10.0),
        })
        .collect()
}

pub struct SceneNode {
    id: usize,
    z_order: usize,
    rect: Rect,
}

pub fn create_scene_chunk(size: usize) -> Vec<SceneNode> {
    build_scene_graph(size)
}

pub fn splice_scene_chunk(scene: &mut Vec<SceneNode>, at: usize, chunk: &[SceneNode]) {
    if at < scene.len() {
        scene.splice(at..at, chunk.iter().cloned());
    }
}

pub fn invalidate_widget(_scene: &mut Vec<SceneNode>, _widget_id: usize) {
    // 标记失效
}

pub fn collect_invalidated(_scene: &[SceneNode]) -> Vec<usize> {
    vec![]
}

pub fn build_full_scene(tree: &WidgetTree<()>) -> Vec<SceneNode> {
    build_scene_graph(100)
}

pub fn update_single_widget_scene(_scene: &mut Vec<SceneNode>, _widget_id: usize) {
    // 更新单个 widget 场景
}

pub fn create_unordered_primitives(count: usize) -> Vec<SceneNode> {
    let mut nodes = build_scene_graph(count);
    // 打乱 z-order
    for i in 0..nodes.len() {
        nodes[i].z_order = (i * 7) % count;
    }
    nodes
}

pub fn sort_by_z_order(primitives: &[SceneNode]) -> Vec<SceneNode> {
    let mut sorted = primitives.to_vec();
    sorted.sort_by_key(|n| n.z_order);
    sorted
}

pub fn build_modified_scene_graph(size: usize, modifications: usize) -> Vec<SceneNode> {
    let mut nodes = build_scene_graph(size);
    for i in 0..modifications.min(size) {
        nodes[i].rect.width += 10.0;
    }
    nodes
}

pub fn compute_scene_diff(old: &[SceneNode], new: &[SceneNode]) -> Vec<usize> {
    old.iter()
        .zip(new.iter())
        .enumerate()
        .filter_map(|(i, (o, n))| {
            if o.rect.width != n.rect.width {
                Some(i)
            } else {
                None
            }
        })
        .collect()
}

pub fn create_n_rectangles(n: usize) -> Vec<SceneNode> {
    build_scene_graph(n)
}

pub fn create_n_rounded_rects(n: usize) -> Vec<SceneNode> {
    build_scene_graph(n)
}

pub fn create_n_circles(n: usize) -> Vec<SceneNode> {
    build_scene_graph(n)
}

pub fn create_n_text_primitives(n: usize) -> Vec<SceneNode> {
    build_scene_graph(n)
}

pub fn generate_vertices(primitives: &[SceneNode]) -> Vec<f32> {
    let mut vertices = Vec::new();
    for node in primitives {
        // 每个矩形 6 个顶点 (x, y) - 转换 Dp 为 f32
        let x: f32 = node.rect.x.into();
        let y: f32 = node.rect.y.into();
        let w: f32 = node.rect.width.into();
        let h: f32 = node.rect.height.into();

        vertices.extend_from_slice(&[
            x, y,
            x + w, y,
            x, y + h,
            x + w, y,
            x + w, y + h,
            x, y + h,
        ]);
    }
    vertices
}

pub fn create_rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::new(x, y, width, height)
}

pub fn apply_clipping(primitives: &[SceneNode], clip_rect: &Rect) -> Vec<SceneNode> {
    let clip_x: f32 = clip_rect.x.into();
    let clip_y: f32 = clip_rect.y.into();
    let clip_w: f32 = clip_rect.width.into();
    let clip_h: f32 = clip_rect.height.into();

    primitives
        .iter()
        .filter(|node| {
            let x: f32 = node.rect.x.into();
            let y: f32 = node.rect.y.into();
            let w: f32 = node.rect.width.into();
            let h: f32 = node.rect.height.into();

            x < clip_x + clip_w
                && x + w > clip_x
                && y < clip_y + clip_h
                && y + h > clip_y
        })
        .cloned()
        .collect()
}

impl Clone for SceneNode {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            z_order: self.z_order,
            rect: self.rect,
        }
    }
}

// ============================================================
// Event Handling Helpers
// ============================================================

pub struct RuntimeState {
    hover_id: Option<usize>,
    focus_id: Option<usize>,
}

pub fn build_hit_regions(_stats: &WidgetBenchmarkStats) -> Vec<Rect> {
    vec![]
}

pub fn hit_test(_regions: &[Rect], _pos: (f32, f32)) -> Option<usize> {
    Some(0)
}

pub fn create_runtime_state(_tree: &WidgetTree<()>) -> RuntimeState {
    RuntimeState {
        hover_id: None,
        focus_id: None,
    }
}

pub fn update_hover_state(runtime: &mut RuntimeState, _pos: (f32, f32)) {
    runtime.hover_id = Some(0);
}

pub enum FocusDirection {
    Next,
    Previous,
}

pub fn navigate_focus(runtime: &mut RuntimeState, _dir: FocusDirection) {
    runtime.focus_id = Some(runtime.focus_id.unwrap_or(0) + 1);
}

pub fn create_focusable_tree(count: usize) -> WidgetTree<()> {
    create_flat_element_tree(count)
}

pub enum MouseEventType {
    Click,
    Move,
    Down,
    Up,
}

pub struct MouseEvent {
    event_type: MouseEventType,
    pos: (f32, f32),
}

pub fn create_mouse_event(event_type: MouseEventType, pos: (f32, f32)) -> MouseEvent {
    MouseEvent { event_type, pos }
}

pub fn dispatch_mouse_event(_runtime: &mut RuntimeState, _event: MouseEvent) {
    // 派发鼠标事件
}

pub struct KeyCode;
impl KeyCode {
    pub const A: Self = KeyCode;
}

pub struct Modifiers;
impl Modifiers {
    pub const NONE: Self = Modifiers;
}

pub struct KeyboardEvent;

pub fn create_keyboard_event(_key: KeyCode, _mods: Modifiers) -> KeyboardEvent {
    KeyboardEvent
}

pub fn dispatch_keyboard_event(_runtime: &mut RuntimeState, _event: KeyboardEvent) {
    // 派发键盘事件
}

pub fn set_focus(runtime: &mut RuntimeState, id: usize) {
    runtime.focus_id = Some(id);
}

pub struct TestCommand;

pub fn create_test_command() -> TestCommand {
    TestCommand
}

pub fn dispatch_command(_runtime: &mut RuntimeState, _cmd: TestCommand) {
    // 派发命令
}

pub fn create_scrollable_tree() -> WidgetTree<()> {
    create_flat_element_tree(50)
}

pub struct ScrollEvent {
    delta: (f32, f32),
}

pub fn create_scroll_event(dx: f32, dy: f32) -> ScrollEvent {
    ScrollEvent { delta: (dx, dy) }
}

pub fn handle_scroll_event(_runtime: &mut RuntimeState, _event: ScrollEvent) {
    // 处理滚动事件
}

pub fn update_drag_state(_runtime: &mut RuntimeState, _pos: (f32, f32)) {
    // 更新拖拽状态
}

pub enum TouchPhase {
    Started,
    Moved,
    Ended,
}

pub struct TouchEvent {
    phase: TouchPhase,
    pos: (f32, f32),
}

pub fn create_touch_event(phase: TouchPhase, pos: (f32, f32)) -> TouchEvent {
    TouchEvent { phase, pos }
}

pub struct GestureRecognizer {
    state: usize,
}

pub fn create_gesture_recognizer() -> GestureRecognizer {
    GestureRecognizer { state: 0 }
}

pub fn recognize_gesture(recognizer: &mut GestureRecognizer, _event: TouchEvent) {
    recognizer.state += 1;
}

pub fn bubble_event(_runtime: &mut RuntimeState, _event: MouseEvent) {
    // 事件冒泡
}

// ============================================================
// Animation Helpers
// ============================================================

pub struct AnimationEngine {
    animations: Vec<Animation>,
}

pub struct Animation {
    id: usize,
    duration: f32,
    elapsed: f32,
}

pub fn create_animation_engine() -> AnimationEngine {
    AnimationEngine {
        animations: Vec::new(),
    }
}

pub fn create_n_animations(n: usize) -> Vec<Animation> {
    (0..n)
        .map(|i| Animation {
            id: i,
            duration: 1000.0,
            elapsed: 0.0,
        })
        .collect()
}

pub fn add_animation_to_engine(engine: &mut AnimationEngine, anim: Animation) {
    engine.animations.push(anim);
}

pub fn update_animation_engine(engine: &mut AnimationEngine, dt: f32) {
    for anim in &mut engine.animations {
        anim.elapsed += dt;
        if anim.elapsed > anim.duration {
            anim.elapsed = 0.0;
        }
    }
}

#[derive(Copy, Clone)]
pub enum InterpolationType {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    Spring,
}

pub fn interpolate_float(start: f32, end: f32, t: f32, interp: InterpolationType) -> f32 {
    let t = match interp {
        InterpolationType::Linear => t,
        InterpolationType::EaseIn => t * t,
        InterpolationType::EaseOut => t * (2.0 - t),
        InterpolationType::EaseInOut => {
            if t < 0.5 {
                2.0 * t * t
            } else {
                -1.0 + (4.0 - 2.0 * t) * t
            }
        }
        InterpolationType::Spring => {
            let damping = 0.8;
            1.0 - (1.0 - t).powf(2.0) * (1.0 + damping * (1.0 - t))
        }
    };
    start + (end - start) * t
}

pub struct Color {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

pub fn create_color(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color { r, g, b, a }
}

pub fn interpolate_color(c1: Color, c2: Color, t: f32) -> Color {
    Color {
        r: ((1.0 - t) * c1.r as f32 + t * c2.r as f32) as u8,
        g: ((1.0 - t) * c1.g as f32 + t * c2.g as f32) as u8,
        b: ((1.0 - t) * c1.b as f32 + t * c2.b as f32) as u8,
        a: ((1.0 - t) * c1.a as f32 + t * c2.a as f32) as u8,
    }
}

#[derive(Copy, Clone)]
pub struct Transform {
    x: f32,
    y: f32,
    scale: f32,
    rotation: f32,
}

pub fn create_transform(x: f32, y: f32, scale: f32, rotation: f32) -> Transform {
    Transform { x, y, scale, rotation }
}

pub fn interpolate_transform(t1: Transform, t2: Transform, t: f32) -> Transform {
    Transform {
        x: t1.x + (t2.x - t1.x) * t,
        y: t1.y + (t2.y - t1.y) * t,
        scale: t1.scale + (t2.scale - t1.scale) * t,
        rotation: t1.rotation + (t2.rotation - t1.rotation) * t,
    }
}

pub struct Timeline {
    duration: f32,
}

pub fn create_complex_timeline() -> Timeline {
    Timeline { duration: 2000.0 }
}

pub fn evaluate_timeline(_timeline: &Timeline, _time: f32) -> f32 {
    0.5
}

pub struct AnimationStateMachine {
    state: usize,
}

pub fn create_animation_state_machine() -> AnimationStateMachine {
    AnimationStateMachine { state: 0 }
}

pub fn trigger_state_transition(machine: &mut AnimationStateMachine) {
    machine.state = (machine.state + 1) % 5;
}

pub struct SpringAnimation {
    current: f32,
    target: f32,
    velocity: f32,
    stiffness: f32,
    damping: f32,
}

pub fn create_spring_animation(start: f32, target: f32, stiffness: f32, damping: f32) -> SpringAnimation {
    SpringAnimation {
        current: start,
        target,
        velocity: 0.0,
        stiffness,
        damping,
    }
}

pub fn update_spring(spring: &mut SpringAnimation, dt: f32) -> f32 {
    let force = (spring.target - spring.current) * spring.stiffness;
    let damping_force = spring.velocity * spring.damping;
    spring.velocity += (force - damping_force) * dt / 1000.0;
    spring.current += spring.velocity * dt / 1000.0;
    spring.current
}

pub struct Keyframe {
    time: f32,
    value: f32,
}

pub fn create_n_keyframes(n: usize) -> Vec<Keyframe> {
    (0..n)
        .map(|i| Keyframe {
            time: i as f32 / (n - 1) as f32,
            value: (i as f32 / (n - 1) as f32) * 100.0,
        })
        .collect()
}

pub fn evaluate_keyframes(keyframes: &[Keyframe], t: f32) -> f32 {
    if keyframes.is_empty() {
        return 0.0;
    }
    if keyframes.len() == 1 {
        return keyframes[0].value;
    }

    for i in 0..keyframes.len() - 1 {
        if t >= keyframes[i].time && t <= keyframes[i + 1].time {
            let local_t = (t - keyframes[i].time) / (keyframes[i + 1].time - keyframes[i].time);
            return keyframes[i].value + (keyframes[i + 1].value - keyframes[i].value) * local_t;
        }
    }

    keyframes.last().unwrap().value
}
