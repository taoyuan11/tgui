use crate::foundation::binding::{InvalidationSignal, State};
use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::{StyleContext, WidgetState};
use crate::ui::layout::{pct, Axis, LayoutStyle, Value};

use super::common::VisualStyle;
use super::core::Element;
use super::p3_support::{
    impl_p3_layout_api, merge_layout, resolve_component_style_with_sheet, with_visual_identity,
};
use super::style::{ContainerStyle, SplitterStyle, StyleResolver, StyleSheet};
use super::{CursorStyle, Flex, SplitterHandleState, Stack, WidgetKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitterAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SplitterResize {
    pub index: usize,
    pub sizes: Vec<f32>,
}

#[derive(Clone)]
pub struct Pane<VM> {
    content: Element<VM>,
    min: f32,
    max: f32,
}

impl<VM> Pane<VM> {
    pub fn new(content: impl Into<Element<VM>>) -> Self {
        Self {
            content: content.into(),
            min: 0.05,
            max: 1.0,
        }
    }

    pub fn min(mut self, min: f32) -> Self {
        if min.is_finite() {
            self.min = min.clamp(0.0, 1.0);
        }
        self
    }

    pub fn max(mut self, max: f32) -> Self {
        if max.is_finite() {
            self.max = max.clamp(0.0, 1.0);
        }
        self
    }
}

pub struct ResizablePanels<VM> {
    panes: Vec<Pane<VM>>,
    sizes: Value<Vec<f32>>,
    axis: SplitterAxis,
    step: f32,
    on_resize: Option<ValueCommand<VM, SplitterResize>>,
    style: Option<StyleResolver<SplitterStyle>>,
    layout: LayoutStyle,
    visual: VisualStyle,
    key: Option<WidgetKey>,
}

pub type Splitter<VM> = ResizablePanels<VM>;

impl<VM> ResizablePanels<VM> {
    pub fn new(panes: Vec<Pane<VM>>, sizes: impl Into<Value<Vec<f32>>>) -> Self {
        Self {
            panes,
            sizes: sizes.into(),
            axis: SplitterAxis::Horizontal,
            step: 0.05,
            on_resize: None,
            style: None,
            layout: LayoutStyle::default(),
            visual: VisualStyle::default(),
            key: None,
        }
    }

    pub fn axis(mut self, axis: SplitterAxis) -> Self {
        self.axis = axis;
        self
    }

    pub fn step(mut self, step: f32) -> Self {
        self.step = if step.is_finite() {
            step.abs().clamp(0.001, 1.0)
        } else {
            0.05
        };
        self
    }

    pub fn on_resize(mut self, command: ValueCommand<VM, SplitterResize>) -> Self {
        self.on_resize = Some(command);
        self
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut SplitterStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| SplitterStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> SplitterStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }

    impl_p3_layout_api!(layout);
}

impl<VM: 'static> From<ResizablePanels<VM>> for Element<VM> {
    fn from(splitter: ResizablePanels<VM>) -> Self {
        let constraints = normalize_constraints(
            splitter
                .panes
                .iter()
                .map(|pane| (pane.min, pane.max))
                .collect::<Vec<_>>(),
        );
        let pane_count = splitter.panes.len();
        let (sizes, local_sizes) = splitter_sizes_value(splitter.sizes.clone(), pane_count);
        let on_resize = match local_sizes {
            Some(local_sizes) => {
                let external = splitter.on_resize.clone();
                Some(ValueCommand::new_with_context(
                    move |view_model, resize: SplitterResize, context| {
                        local_sizes.set(normalize_sizes(resize.sizes.clone(), pane_count));
                        if let Some(command) = external.as_ref() {
                            command.execute_with_context(view_model, resize, context);
                        }
                    },
                ))
            }
            None => splitter.on_resize.clone(),
        };
        let mut children = Vec::new();
        for (index, pane) in splitter.panes.into_iter().enumerate() {
            let mut content = pane.content;
            content.layout.grow = splitter_pane_grow(&sizes, pane_count, index);
            children.push(content);
            if index + 1 < pane_count {
                let handle_on_resize = on_resize.clone();
                let reset_sizes = splitter_reset_sizes(pane_count);
                let style = splitter.style.clone();
                let axis = splitter.axis;
                let handle_identity = splitter.visual.clone();
                let handle_layout_style = splitter.style.clone();
                let line_layout_style = splitter.style.clone();
                let mut handle: Element<VM> = with_visual_identity(
                    Stack::new()
                        .runtime_layout(move |layout, _container, context, style_sheet, visual| {
                            let resolved = resolve_splitter_style_with_sheet(
                                handle_layout_style.as_ref(),
                                context,
                                style_sheet,
                                visual,
                                WidgetState::default(),
                            );
                            if axis == SplitterAxis::Horizontal {
                                layout.width = Some(Value::Static(crate::ui::layout::Length::Px(
                                    resolved.hit_extent,
                                )));
                                layout.height = Some(Value::Static(pct(100.0)));
                            } else {
                                layout.width = Some(Value::Static(pct(100.0)));
                                layout.height = Some(Value::Static(crate::ui::layout::Length::Px(
                                    resolved.hit_extent,
                                )));
                            }
                        })
                        .center()
                        .cursor(if axis == SplitterAxis::Horizontal {
                            CursorStyle::EwResize
                        } else {
                            CursorStyle::NsResize
                        })
                        .on_click(Command::new_with_context({
                            let on_resize = handle_on_resize.clone();
                            let click_sizes = sizes.clone();
                            let constraints = constraints.clone();
                            let step = splitter.step;
                            move |vm, context| {
                                if let Some(command) = on_resize.as_ref() {
                                    let next_sizes = splitter_adjusted_sizes(
                                        &click_sizes.resolve(),
                                        &constraints,
                                        index,
                                        step,
                                    );
                                    command.execute_with_context(
                                        vm,
                                        SplitterResize {
                                            index,
                                            sizes: next_sizes,
                                        },
                                        context,
                                    );
                                }
                            }
                        }))
                        .on_double_click(Command::new_with_context(move |vm, context| {
                            if let Some(command) = handle_on_resize.as_ref() {
                                command.execute_with_context(
                                    vm,
                                    SplitterResize {
                                        index,
                                        sizes: reset_sizes.clone(),
                                    },
                                    context,
                                );
                            }
                        }))
                        .focusable(true)
                        .tab_index(0)
                        .child(with_visual_identity(
                            Stack::<VM>::new()
                                .runtime_layout(
                                    move |layout, _container, context, style_sheet, visual| {
                                        let resolved = resolve_splitter_style_with_sheet(
                                            line_layout_style.as_ref(),
                                            context,
                                            style_sheet,
                                            visual,
                                            WidgetState::default(),
                                        );
                                        if axis == SplitterAxis::Horizontal {
                                            layout.width =
                                                Some(Value::Static(crate::ui::layout::Length::Px(
                                                    resolved.handle_thickness,
                                                )));
                                            layout.height = Some(Value::Static(pct(100.0)));
                                        } else {
                                            layout.width = Some(Value::Static(pct(100.0)));
                                            layout.height =
                                                Some(Value::Static(crate::ui::layout::Length::Px(
                                                    resolved.handle_thickness,
                                                )));
                                        }
                                    },
                                )
                                .style_full_with_style_sheet(
                                    move |context, style_sheet, visual, state| {
                                        let resolved = resolve_splitter_style_with_sheet(
                                            style.as_ref(),
                                            context,
                                            style_sheet,
                                            visual,
                                            state,
                                        );
                                        let mut container =
                                            ContainerStyle::default_for_theme(context.theme);
                                        container.surface.background =
                                            Some(resolved.handle_color.resolve(state).clone());
                                        container
                                    },
                                )
                                .into(),
                            &handle_identity,
                        ))
                        .into(),
                    &handle_identity,
                );
                if on_resize.is_some() {
                    handle.splitter_handle = Some(SplitterHandleState {
                        axis: match axis {
                            SplitterAxis::Horizontal => Axis::Horizontal,
                            SplitterAxis::Vertical => Axis::Vertical,
                        },
                        index,
                        sizes: sizes.clone(),
                        constraints: constraints.clone(),
                        step: splitter.step,
                        on_resize: on_resize.clone(),
                    });
                } else {
                    handle.interactions = Default::default();
                    handle.focus = Default::default();
                }
                children.push(handle);
            }
        }
        let runtime_style = splitter.style.clone();
        let mut root: Element<VM> = Flex::new(match splitter.axis {
            SplitterAxis::Horizontal => Axis::Horizontal,
            SplitterAxis::Vertical => Axis::Vertical,
        })
        .gap(crate::ui::unit::dp(0.0))
        .runtime_layout(move |_layout, container, context, style_sheet, visual| {
            let resolved = resolve_splitter_style_with_sheet(
                runtime_style.as_ref(),
                context,
                style_sheet,
                visual,
                WidgetState::default(),
            );
            container.gap = Value::Static(crate::ui::layout::Length::Px(resolved.gap));
        })
        .child(children)
        .into();
        root.key = splitter.key;
        root = with_visual_identity(root, &splitter.visual);
        root.layout = merge_layout(root.layout, splitter.layout);
        root
    }
}

fn normalize_sizes(mut sizes: Vec<f32>, count: usize) -> Vec<f32> {
    if count == 0 {
        return Vec::new();
    }
    if sizes.len() != count
        || sizes
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
    {
        sizes = vec![1.0 / count as f32; count];
    }
    let total = sizes.iter().map(|value| f64::from(*value)).sum::<f64>();
    if !total.is_finite() || total <= f64::from(f32::EPSILON) {
        vec![1.0 / count as f32; count]
    } else {
        sizes
            .into_iter()
            .map(|value| (f64::from(value) / total) as f32)
            .collect()
    }
}

fn normalize_constraints(mut constraints: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    for (min, max) in &mut constraints {
        *min = if min.is_finite() {
            min.clamp(0.0, 1.0)
        } else {
            0.0
        };
        *max = if max.is_finite() {
            max.clamp(0.0, 1.0)
        } else {
            1.0
        };
        if *min > *max {
            std::mem::swap(min, max);
        }
    }
    constraints
}

fn splitter_sizes_value(
    sizes: Value<Vec<f32>>,
    count: usize,
) -> (Value<Vec<f32>>, Option<State<Vec<f32>>>) {
    match sizes {
        Value::Static(sizes) => {
            let local = State::new(normalize_sizes(sizes, count), InvalidationSignal::new());
            (Value::Signal(local.signal()), Some(local))
        }
        Value::Signal(signal) => (
            Value::Signal(
                signal
                    .project(move |sizes| normalize_sizes(sizes.clone(), count))
                    .without_transition(),
            ),
            None,
        ),
    }
}

fn splitter_pane_grow(sizes: &Value<Vec<f32>>, count: usize, index: usize) -> Value<f32> {
    match sizes {
        Value::Static(sizes) => Value::Static(sizes[index].max(0.001)),
        Value::Signal(signal) => {
            Value::Signal(signal.project(move |sizes| sizes[index.min(count - 1)].max(0.001)))
        }
    }
}

pub(crate) fn splitter_adjusted_sizes(
    sizes: &[f32],
    constraints: &[(f32, f32)],
    index: usize,
    delta: f32,
) -> Vec<f32> {
    let mut next = sizes.to_vec();
    if index + 1 >= next.len() {
        return next;
    }
    if !delta.is_finite() || delta.abs() <= f32::EPSILON {
        return normalize_sizes(next, sizes.len());
    }
    let left_min = constraints.get(index).map(|(min, _)| *min).unwrap_or(0.0);
    let left_max = constraints.get(index).map(|(_, max)| *max).unwrap_or(1.0);
    let right_min = constraints
        .get(index + 1)
        .map(|(min, _)| *min)
        .unwrap_or(0.0);
    let right_max = constraints
        .get(index + 1)
        .map(|(_, max)| *max)
        .unwrap_or(1.0);
    if delta > 0.0 {
        let applied = delta
            .min(left_max - next[index])
            .min(next[index + 1] - right_min);
        if applied > 0.0 {
            next[index] += applied;
            next[index + 1] -= applied;
        }
    } else {
        let applied = (-delta)
            .min(next[index] - left_min)
            .min(right_max - next[index + 1]);
        if applied > 0.0 {
            next[index] -= applied;
            next[index + 1] += applied;
        }
    }
    normalize_sizes(next, sizes.len())
}

pub(crate) fn splitter_reset_sizes(count: usize) -> Vec<f32> {
    normalize_sizes(Vec::new(), count)
}

fn resolve_splitter_style_with_sheet(
    style: Option<&StyleResolver<SplitterStyle>>,
    context: &StyleContext<'_>,
    style_sheet: &StyleSheet,
    visual: &VisualStyle,
    state: WidgetState,
) -> SplitterStyle {
    resolve_component_style_with_sheet(
        style,
        context,
        style_sheet,
        visual,
        state,
        SplitterStyle::default_for_theme(context.theme),
        |base, context| context.theme.components.splitter.apply(base, context),
        |sheet, base, context, visual| sheet.apply_splitter(base, context, visual),
        |sheet, base, context, visual, state| {
            sheet.apply_splitter_state(base, context, visual, state)
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_sizes_close(actual: Vec<f32>, expected: &[f32]) {
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert!((actual - expected).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn splitter_adjusted_sizes_supports_positive_and_negative_delta() {
        let sizes = vec![0.5, 0.5];
        let constraints = vec![(0.2, 0.8), (0.2, 0.8)];

        assert_eq!(
            splitter_adjusted_sizes(&sizes, &constraints, 0, 0.1),
            vec![0.6, 0.4]
        );
        assert_eq!(
            splitter_adjusted_sizes(&sizes, &constraints, 0, -0.1),
            vec![0.4, 0.6]
        );
    }

    #[test]
    fn splitter_adjusted_sizes_respects_adjacent_min_max() {
        let sizes = vec![0.75, 0.25];
        let constraints = vec![(0.2, 0.8), (0.2, 0.8)];

        assert_sizes_close(
            splitter_adjusted_sizes(&sizes, &constraints, 0, 0.5),
            &[0.8, 0.2],
        );
        assert_sizes_close(
            splitter_adjusted_sizes(&sizes, &constraints, 0, -0.8),
            &[0.2, 0.8],
        );
    }

    #[test]
    fn splitter_normalizes_non_finite_sizes_without_overflow() {
        assert_sizes_close(normalize_sizes(vec![f32::MAX, f32::MAX], 2), &[0.5, 0.5]);
        assert_sizes_close(normalize_sizes(vec![f32::NAN, 1.0], 2), &[0.5, 0.5]);
    }

    #[test]
    fn splitter_normalizes_non_finite_and_reversed_constraints() {
        assert_eq!(
            normalize_constraints(vec![(f32::NAN, f32::INFINITY), (0.8, 0.2)]),
            vec![(0.0, 1.0), (0.2, 0.8)]
        );
    }
}
