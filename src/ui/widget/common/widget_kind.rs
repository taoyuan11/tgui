use super::*;
use crate::ui::widget::core::Element;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ContainerKind {
    Flow,
    Stack,
    Grid {
        columns: Vec<Track>,
        rows: Vec<Track>,
    },
    Flex {
        direction: Axis,
        wrap: Wrap,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ContainerLayout {
    pub kind: ContainerKind,
    pub padding: Option<Value<Insets>>,
    pub gap: Value<crate::ui::layout::Length>,
    pub justify: Justify,
    pub align: Align,
    pub overflow_x: Overflow,
    pub overflow_y: Overflow,
    pub scrollbar_style: ScrollbarStyle,
}

impl ContainerLayout {
    pub(crate) fn flow() -> Self {
        Self {
            kind: ContainerKind::Flow,
            padding: None,
            gap: Value::Static(crate::ui::layout::Length::Px(Dp::ZERO)),
            justify: Justify::Start,
            align: Align::Start,
            overflow_x: Overflow::Hidden,
            overflow_y: Overflow::Hidden,
            scrollbar_style: ScrollbarStyle::default(),
        }
    }
}

pub(crate) enum ChildSource<VM> {
    Static(Vec<Element<VM>>),
    Dynamic(Arc<dyn Fn() -> Vec<Element<VM>> + Send + Sync>),
}

impl<VM> ChildSource<VM> {
    pub(crate) fn resolve(&self, owner: Option<WidgetId>) -> Vec<Element<VM>> {
        match self {
            Self::Static(children) => children.clone(),
            Self::Dynamic(resolver) => {
                if let Some(owner) = owner {
                    track_dependency_scope(
                        owner.dependency_owner(DependencyPhase::Structure),
                        || resolve_dynamic_children(resolver),
                    )
                } else {
                    resolve_dynamic_children(resolver)
                }
            }
        }
    }

    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> ChildSource<RootVm>
    where
        VM: 'static,
    {
        match self {
            Self::Static(children) => ChildSource::Static(
                children
                    .into_iter()
                    .map(|child| child.scope_with_selector(selector.clone()))
                    .collect(),
            ),
            Self::Dynamic(resolver) => ChildSource::Dynamic(Arc::new(move || {
                resolver()
                    .into_iter()
                    .map(|child| child.scope_with_selector(selector.clone()))
                    .collect()
            })),
        }
    }
}

fn resolve_dynamic_children<VM>(
    resolver: &Arc<dyn Fn() -> Vec<Element<VM>> + Send + Sync>,
) -> Vec<Element<VM>> {
    #[cfg(any(
        target_os = "windows",
        target_os = "macos",
        all(target_os = "linux", not(target_env = "ohos"))
    ))]
    {
        const CHILD_RESOLVER_STACK_SIZE: usize = 8 * 1024 * 1024;
        const CHILD_RESOLVER_STACK_RED_ZONE: usize = CHILD_RESOLVER_STACK_SIZE;
        stacker::maybe_grow(
            CHILD_RESOLVER_STACK_RED_ZONE,
            CHILD_RESOLVER_STACK_SIZE,
            || resolver(),
        )
    }

    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        all(target_os = "linux", not(target_env = "ohos"))
    )))]
    {
        resolver()
    }
}

impl<VM> Clone for ChildSource<VM> {
    fn clone(&self) -> Self {
        match self {
            Self::Static(children) => Self::Static(children.clone()),
            Self::Dynamic(resolver) => Self::Dynamic(resolver.clone()),
        }
    }
}

pub(crate) enum WidgetKind<VM> {
    Container {
        layout: ContainerLayout,
        children: Vec<ChildSource<VM>>,
        style: Option<StyleResolver<ContainerStyle>>,
    },
    Text {
        text: Text,
    },
    #[cfg(feature = "audio")]
    Audio {
        audio: Audio,
    },
    Image {
        image: Image,
    },
    Canvas {
        scene: Value<CanvasScene>,
        item_interactions: CanvasItemInteractionHandlers<VM>,
        style: Option<StyleResolver<CanvasStyle>>,
    },
    #[cfg(feature = "video")]
    VideoSurface {
        video: VideoSurface,
        style: Option<StyleResolver<VideoSurfaceStyle>>,
    },
    Button {
        label: Value<String>,
        disabled: Value<bool>,
        variant: ButtonVariantKind,
        style: Option<StyleResolver<WidgetButtonStyle>>,
    },
    Checkbox {
        checked: Value<bool>,
        label: Option<Value<String>>,
        on_change: Option<ValueCommand<VM, bool>>,
        disabled: Value<bool>,
        style: Option<StyleResolver<WidgetCheckboxStyle>>,
    },
    Radio {
        checked: Value<bool>,
        label: Option<Value<String>>,
        on_change: Option<ValueCommand<VM, bool>>,
        disabled: Value<bool>,
        style: Option<StyleResolver<crate::ui::widget::RadioStyle>>,
    },
    Switch {
        checked: Value<bool>,
        on_change: Option<ValueCommand<VM, bool>>,
        active_background: Option<Value<Color>>,
        inactive_background: Option<Value<Color>>,
        active_thumb_color: Option<Value<Color>>,
        inactive_thumb_color: Option<Value<Color>>,
        disabled: Value<bool>,
        style: Option<StyleResolver<WidgetSwitchStyle>>,
    },
    Select {
        selected_label: Value<Option<String>>,
        placeholder: Value<String>,
        options: Vec<SelectOptionState<VM>>,
        open: Option<Value<bool>>,
        on_open_change: Option<ValueCommand<VM, bool>>,
        disabled: Value<bool>,
        style: Option<StyleResolver<WidgetSelectStyle>>,
    },
    Slider {
        value: Value<f32>,
        min: f32,
        max: f32,
        step: f32,
        show_ticks: bool,
        show_value_label: bool,
        tick_count: Option<usize>,
        value_formatter: Option<SliderValueFormatter>,
        on_change: Option<ValueCommand<VM, f32>>,
        disabled: Value<bool>,
        style: Option<StyleResolver<WidgetSliderStyle>>,
    },
    TextEditor {
        controller: TextController,
        placeholder: Value<String>,
        on_change: Option<Command<VM>>,
        on_change_set: Option<ValueCommand<VM, TextChangeSet>>,
        disabled: Value<bool>,
        input_style: Option<StyleResolver<WidgetInputStyle>>,
        textarea_style: Option<StyleResolver<WidgetTextareaStyle>>,
        multiline: bool,
        show_scrollbar: Value<bool>,
        auto_wrap: Value<bool>,
    },
}

pub(crate) struct SelectOptionState<VM> {
    pub label: Value<String>,
    pub selected: Value<bool>,
    pub disabled: Value<bool>,
    pub on_select: Option<Command<VM>>,
}

impl<VM> Clone for SelectOptionState<VM> {
    fn clone(&self) -> Self {
        Self {
            label: self.label.clone(),
            selected: self.selected.clone(),
            disabled: self.disabled.clone(),
            on_select: self.on_select.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonVariantKind {
    Primary,
    Secondary,
    Ghost,
    Danger,
}

impl<VM> Clone for WidgetKind<VM> {
    fn clone(&self) -> Self {
        match self {
            Self::Container {
                layout,
                children,
                style,
            } => Self::Container {
                layout: layout.clone(),
                children: children.clone(),
                style: style.clone(),
            },
            Self::Text { text } => Self::Text { text: text.clone() },
            #[cfg(feature = "audio")]
            Self::Audio { audio } => Self::Audio {
                audio: audio.clone(),
            },
            Self::Image { image } => Self::Image {
                image: image.clone(),
            },
            Self::Canvas {
                scene,
                item_interactions,
                style,
            } => Self::Canvas {
                scene: scene.clone(),
                item_interactions: item_interactions.clone(),
                style: style.clone(),
            },
            #[cfg(feature = "video")]
            Self::VideoSurface { video, style } => Self::VideoSurface {
                video: video.clone(),
                style: style.clone(),
            },
            Self::Button {
                label,
                disabled,
                variant,
                style,
            } => Self::Button {
                label: label.clone(),
                disabled: disabled.clone(),
                variant: *variant,
                style: style.clone(),
            },
            Self::Checkbox {
                checked,
                label,
                on_change,
                disabled,
                style,
            } => Self::Checkbox {
                checked: checked.clone(),
                label: label.clone(),
                on_change: on_change.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::Radio {
                checked,
                label,
                on_change,
                disabled,
                style,
            } => Self::Radio {
                checked: checked.clone(),
                label: label.clone(),
                on_change: on_change.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::Switch {
                checked,
                on_change,
                active_background,
                inactive_background,
                active_thumb_color,
                inactive_thumb_color,
                disabled,
                style,
            } => Self::Switch {
                checked: checked.clone(),
                on_change: on_change.clone(),
                active_background: active_background.clone(),
                inactive_background: inactive_background.clone(),
                active_thumb_color: active_thumb_color.clone(),
                inactive_thumb_color: inactive_thumb_color.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::Select {
                selected_label,
                placeholder,
                options,
                open,
                on_open_change,
                disabled,
                style,
            } => Self::Select {
                selected_label: selected_label.clone(),
                placeholder: placeholder.clone(),
                options: options.clone(),
                open: open.clone(),
                on_open_change: on_open_change.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::Slider {
                value,
                min,
                max,
                step,
                show_ticks,
                show_value_label,
                tick_count,
                value_formatter,
                on_change,
                disabled,
                style,
            } => Self::Slider {
                value: value.clone(),
                min: *min,
                max: *max,
                step: *step,
                show_ticks: *show_ticks,
                show_value_label: *show_value_label,
                tick_count: *tick_count,
                value_formatter: value_formatter.clone(),
                on_change: on_change.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::TextEditor {
                controller,
                placeholder,
                on_change,
                on_change_set,
                disabled,
                input_style,
                textarea_style,
                multiline,
                show_scrollbar,
                auto_wrap,
            } => Self::TextEditor {
                controller: controller.clone(),
                placeholder: placeholder.clone(),
                on_change: on_change.clone(),
                on_change_set: on_change_set.clone(),
                disabled: disabled.clone(),
                input_style: input_style.clone(),
                textarea_style: textarea_style.clone(),
                multiline: *multiline,
                show_scrollbar: show_scrollbar.clone(),
                auto_wrap: auto_wrap.clone(),
            },
        }
    }
}
