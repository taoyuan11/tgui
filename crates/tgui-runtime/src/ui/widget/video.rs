use std::time::Duration;

use crate::foundation::binding::Signal;
use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::media::ContentFit;
use crate::theme::{StyleContext, WidgetState};
use crate::ui::layout::{pct, Align, Insets, LayoutStyle, Overflow, Value};
use crate::ui::theme::{StateValue, Theme};
use crate::ui::unit::dp;
use crate::video::{VideoController, VideoPlaybackState};

use super::common::{
    CursorStyle, InteractionHandlers, LifecycleEventHandlers, MediaEventHandlers, Point,
    VisualStyle, WidgetId, WidgetKey, WidgetKind,
};
use super::container::{set_layout_inset, set_layout_length, set_layout_lengths, IntoLengthValue};
use super::core::Element;
use super::icon::{Icon, SvgIconId};
use super::p3_support::{
    impl_p3_layout_api, merge_layout, resolve_component_style_with_sheet, with_visual_identity,
};
use super::style::{
    ContainerStyle, IconStyle, ProgressBarStyle, SliderStyle, StyleResolver, StyleSheet,
    TextWidgetStyle, VideoStyle, VideoSurfaceStyle,
};
use super::{Flex, IntoTextContent, ProgressBar, Slider, Stack, Text};

#[derive(Clone)]
pub struct VideoSurface {
    pub(crate) key: Option<WidgetKey>,
    pub(crate) layout: LayoutStyle,
    pub(crate) visual: VisualStyle,
    pub(crate) controller: VideoController,
    pub(crate) background: Option<Value<Color>>,
    pub(crate) fit: ContentFit,
    pub(crate) cursor_style: Option<Value<CursorStyle>>,
    pub(crate) style: Option<StyleResolver<VideoSurfaceStyle>>,
}

macro_rules! impl_video_layout_api {
    () => {
        pub fn size(mut self, width: impl IntoLengthValue, height: impl IntoLengthValue) -> Self {
            set_layout_lengths(&mut self.layout, width, height);
            self
        }

        pub fn width(mut self, width: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.width, width);
            self
        }

        pub fn height(mut self, height: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.height, height);
            self
        }

        pub fn min_width(mut self, width: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.min_width, width);
            self
        }

        pub fn min_height(mut self, height: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.min_height, height);
            self
        }

        pub fn max_width(mut self, width: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.max_width, width);
            self
        }

        pub fn max_height(mut self, height: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.max_height, height);
            self
        }

        pub fn aspect_ratio(mut self, aspect_ratio: impl Into<Value<f32>>) -> Self {
            self.layout.aspect_ratio = Some(aspect_ratio.into());
            self
        }

        pub fn margin(mut self, insets: impl Into<Value<Insets>>) -> Self {
            self.layout.margin = insets.into();
            self
        }

        pub fn padding(mut self, insets: impl Into<Value<Insets>>) -> Self {
            self.layout.padding = Some(insets.into());
            self
        }

        pub fn grow(mut self, grow: impl Into<Value<f32>>) -> Self {
            self.layout.grow = grow.into();
            self
        }

        pub fn shrink(mut self, shrink: impl Into<Value<f32>>) -> Self {
            self.layout.shrink = shrink.into();
            self
        }

        pub fn basis(mut self, basis: impl IntoLengthValue) -> Self {
            self.layout.basis = Some(basis.into_length_value());
            self
        }

        pub fn align_self(mut self, align: Align) -> Self {
            self.layout.align_self = Some(align);
            self
        }

        pub fn justify_self(mut self, align: Align) -> Self {
            self.layout.justify_self = Some(align);
            self
        }

        pub fn column(mut self, start: usize) -> Self {
            self.layout.column_start = Some(Value::Static(start.max(1)));
            self
        }

        pub fn row(mut self, start: usize) -> Self {
            self.layout.row_start = Some(Value::Static(start.max(1)));
            self
        }

        pub fn column_span(mut self, span: usize) -> Self {
            self.layout.column_span = span.max(1);
            self
        }

        pub fn row_span(mut self, span: usize) -> Self {
            self.layout.row_span = span.max(1);
            self
        }

        pub fn position_absolute(mut self) -> Self {
            self.layout.position_type = crate::ui::layout::PositionType::Absolute;
            self
        }

        pub fn left(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.layout.left, value);
            self
        }

        pub fn top(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.layout.top, value);
            self
        }

        pub fn right(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.layout.right, value);
            self
        }

        pub fn bottom(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.layout.bottom, value);
            self
        }

        pub fn inset(mut self, value: impl IntoLengthValue + Copy) -> Self {
            set_layout_inset(&mut self.layout.left, value);
            set_layout_inset(&mut self.layout.top, value);
            set_layout_inset(&mut self.layout.right, value);
            set_layout_inset(&mut self.layout.bottom, value);
            self
        }
    };
}

impl VideoSurface {
    pub fn new(controller: VideoController) -> Self {
        Self {
            key: None,
            layout: LayoutStyle::default(),
            visual: VisualStyle::default(),
            controller,
            background: None,
            fit: ContentFit::Contain,
            cursor_style: None,
            style: None,
        }
    }

    impl_video_layout_api!();

    pub fn key(mut self, key: impl Into<WidgetKey>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut VideoSurfaceStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(super::style::StyleResolver::mutate(
            |context| VideoSurfaceStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> VideoSurfaceStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(super::style::StyleResolver::full(resolver));
        self
    }

    pub fn cursor(mut self, cursor: impl Into<Value<CursorStyle>>) -> Self {
        self.cursor_style = Some(cursor.into());
        self
    }

    pub fn on_click<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_click: Some(command),
            ..Default::default()
        })
    }

    pub fn on_double_click<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_double_click: Some(command),
            ..Default::default()
        })
    }

    pub fn on_mouse_enter<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_mouse_enter: Some(command),
            ..Default::default()
        })
    }

    pub fn on_mouse_leave<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_mouse_leave: Some(command),
            ..Default::default()
        })
    }

    pub fn on_mouse_move<VM>(self, command: ValueCommand<VM, Point>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_mouse_move: Some(command),
            ..Default::default()
        })
    }

    pub fn on_mount<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_lifecycle_events(LifecycleEventHandlers {
            on_mount: Some(command),
            ..Default::default()
        })
    }

    pub fn on_unmount<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_lifecycle_events(LifecycleEventHandlers {
            on_unmount: Some(command),
            ..Default::default()
        })
    }

    pub fn on_update<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_lifecycle_events(LifecycleEventHandlers {
            on_update: Some(command),
            ..Default::default()
        })
    }

    pub fn on_loading<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_media_events(MediaEventHandlers {
            on_loading: Some(command),
            ..Default::default()
        })
    }

    pub fn on_success<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_media_events(MediaEventHandlers {
            on_success: Some(command),
            ..Default::default()
        })
    }

    pub fn on_error<VM>(self, command: ValueCommand<VM, String>) -> Element<VM> {
        self.into_element_with_media_events(MediaEventHandlers {
            on_error: Some(command),
            ..Default::default()
        })
    }

    fn into_element_with_interactions<VM>(
        self,
        mut interactions: InteractionHandlers<VM>,
    ) -> Element<VM> {
        interactions.cursor_style = self.cursor_style.clone();
        Element {
            id: WidgetId::next(),
            key: self.key.clone(),
            layout: self.layout.clone(),
            focus: Default::default(),
            visual: self.visual.clone(),
            interactions,
            lifecycle_events: LifecycleEventHandlers::default(),
            media_events: MediaEventHandlers::default(),
            background: self.background.clone(),
            tooltip: None,
            popover: None,
            menu: None,
            context_menu: None,
            modal: None,
            drawer: None,
            tab_trigger: None,
            list_item: None,
            tree_root: None,
            tree_node: None,
            data_grid_root: None,
            data_grid_cell: None,
            data_grid_header: None,
            data_grid_resize_handle: None,
            splitter_handle: None,
            carousel_auto_play: None,
            kind: WidgetKind::VideoSurface {
                style: self.style.clone(),
                video: self,
            },
        }
    }

    fn into_element_with_lifecycle_events<VM>(
        self,
        lifecycle_events: LifecycleEventHandlers<VM>,
    ) -> Element<VM> {
        Element {
            id: WidgetId::next(),
            key: self.key.clone(),
            layout: self.layout.clone(),
            focus: Default::default(),
            visual: self.visual.clone(),
            interactions: InteractionHandlers {
                cursor_style: self.cursor_style.clone(),
                ..Default::default()
            },
            lifecycle_events,
            media_events: MediaEventHandlers::default(),
            background: self.background.clone(),
            tooltip: None,
            popover: None,
            menu: None,
            context_menu: None,
            modal: None,
            drawer: None,
            tab_trigger: None,
            list_item: None,
            tree_root: None,
            tree_node: None,
            data_grid_root: None,
            data_grid_cell: None,
            data_grid_header: None,
            data_grid_resize_handle: None,
            splitter_handle: None,
            carousel_auto_play: None,
            kind: WidgetKind::VideoSurface {
                style: self.style.clone(),
                video: self,
            },
        }
    }

    fn into_element_with_media_events<VM>(
        self,
        media_events: MediaEventHandlers<VM>,
    ) -> Element<VM> {
        Element {
            id: WidgetId::next(),
            key: self.key.clone(),
            layout: self.layout.clone(),
            focus: Default::default(),
            visual: self.visual.clone(),
            interactions: InteractionHandlers {
                cursor_style: self.cursor_style.clone(),
                ..Default::default()
            },
            lifecycle_events: LifecycleEventHandlers::default(),
            media_events,
            background: self.background.clone(),
            tooltip: None,
            popover: None,
            menu: None,
            context_menu: None,
            modal: None,
            drawer: None,
            tab_trigger: None,
            list_item: None,
            tree_root: None,
            tree_node: None,
            data_grid_root: None,
            data_grid_cell: None,
            data_grid_header: None,
            data_grid_resize_handle: None,
            splitter_handle: None,
            carousel_auto_play: None,
            kind: WidgetKind::VideoSurface {
                style: self.style.clone(),
                video: self,
            },
        }
    }
}

pub struct Video<VM> {
    controller: VideoController,
    show_controls: Value<bool>,
    show_status: Value<bool>,
    show_volume: Value<bool>,
    fit: ContentFit,
    style: Option<StyleResolver<VideoStyle>>,
    layout: LayoutStyle,
    visual: VisualStyle,
    key: Option<WidgetKey>,
    media_events: MediaEventHandlers<VM>,
}

impl<VM> Video<VM> {
    pub fn new(controller: VideoController) -> Self {
        Self {
            controller,
            show_controls: Value::Static(true),
            show_status: Value::Static(true),
            show_volume: Value::Static(true),
            fit: ContentFit::Contain,
            style: None,
            layout: LayoutStyle::default(),
            visual: VisualStyle::default(),
            key: None,
            media_events: MediaEventHandlers::default(),
        }
    }

    impl_p3_layout_api!(layout);

    pub fn show_controls(mut self, show_controls: impl Into<Value<bool>>) -> Self {
        self.show_controls = show_controls.into();
        self
    }

    pub fn show_status(mut self, show_status: impl Into<Value<bool>>) -> Self {
        self.show_status = show_status.into();
        self
    }

    pub fn show_volume(mut self, show_volume: impl Into<Value<bool>>) -> Self {
        self.show_volume = show_volume.into();
        self
    }

    pub fn fit(mut self, fit: ContentFit) -> Self {
        self.fit = fit;
        self
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut VideoStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| VideoStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> VideoStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }

    pub fn on_loading(mut self, command: Command<VM>) -> Self {
        self.media_events.on_loading = Some(command);
        self
    }

    pub fn on_success(mut self, command: Command<VM>) -> Self {
        self.media_events.on_success = Some(command);
        self
    }

    pub fn on_error(mut self, command: ValueCommand<VM, String>) -> Self {
        self.media_events.on_error = Some(command);
        self
    }
}

impl<VM: 'static> From<Video<VM>> for Element<VM> {
    fn from(video: Video<VM>) -> Self {
        let layout_style = resolve_video_style_for_layout(video.style.as_ref());
        let controller = video.controller.clone();
        let show_controls = video.show_controls.resolve();
        let show_status = video.show_status.resolve();
        let show_volume = video.show_volume.resolve();
        let fit = video.fit;

        let mut surface: Element<VM> = VideoSurface::new(controller.clone())
            .size(pct(100.0), pct(100.0))
            .position_absolute()
            .inset(dp(0.0))
            .style(move |style, _| {
                style.fit = fit;
                style.surface.background = Some(Color::hexa(0x000000FF).into());
                style.surface.border_radius = Some(Value::Static(layout_style.radius));
            })
            .into();
        surface.media_events = video.media_events.clone();

        let root_style = video.style.clone();
        let mut root = Stack::new()
            .size(
                layout_style.default_surface_width,
                layout_style.default_surface_height,
            )
            .aspect_ratio(16.0 / 9.0)
            .padding(layout_style.padding)
            .overflow(Overflow::Hidden)
            .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
                let resolved = resolve_video_style_with_sheet(
                    root_style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    state,
                );
                let mut container = ContainerStyle::default_for_theme(context.theme);
                container.surface = resolved.surface.clone();
                container.surface.background = Some(resolved.background);
                container.surface.border_color = Some(resolved.border);
                container.surface.border_width = Some(Value::Static(resolved.border_width));
                container.surface.border_radius = Some(Value::Static(resolved.radius));
                container
            })
            .child(surface);

        if show_controls {
            root = root.child(video_controls(
                controller.clone(),
                show_volume,
                video.style.clone(),
                video.visual.clone(),
            ));
        }

        if show_status {
            root = root.child(status_overlay(
                controller.clone(),
                video.style.clone(),
                video.visual.clone(),
            ));
        }

        let mut root: Element<VM> = root.into();
        root.key = video.key;
        root = with_visual_identity(root, &video.visual);
        root.layout = merge_layout(root.layout, video.layout);
        if root.layout.height != LayoutStyle::default().height {
            root.layout.aspect_ratio = None;
        }
        root
    }
}

fn video_controls<VM: 'static>(
    controller: VideoController,
    show_volume: bool,
    style: Option<StyleResolver<VideoStyle>>,
    visual: VisualStyle,
) -> Element<VM> {
    let layout_style = resolve_video_style_for_layout(style.as_ref());
    let progress = progress_signal(&controller);
    let buffered = buffered_signal(&controller);
    let seek_disabled = controller
        .duration()
        .map(|duration| duration.map(|duration| duration.is_zero()).unwrap_or(true));

    let play_controller = controller.clone();
    let play_disabled: Value<bool> = controller
        .playback_state()
        .map(playback_button_disabled)
        .into();
    let play_icons = vec![
        video_icon_with_opacity(
            SvgIconId::PlayArrow,
            style.clone(),
            visual.clone(),
            false,
            playback_icon_opacity(controller.playback_state(), SvgIconId::PlayArrow),
        ),
        video_icon_with_opacity(
            SvgIconId::Pause,
            style.clone(),
            visual.clone(),
            false,
            playback_icon_opacity(controller.playback_state(), SvgIconId::Pause),
        ),
    ];
    let play_button = icon_button(
        play_icons,
        play_disabled,
        style.clone(),
        Command::new(move |_| match play_controller.playback_state().get() {
            VideoPlaybackState::Playing => play_controller.pause(),
            VideoPlaybackState::Ended => play_controller.replay(),
            VideoPlaybackState::Loading
            | VideoPlaybackState::Buffering
            | VideoPlaybackState::Error(_) => {}
            _ => play_controller.play(),
        }),
    );

    let seek_controller = controller.clone();
    let seek_end_controller = controller.clone();
    let seek = Slider::new(progress, 0.0, 1.0)
        .step(0.001)
        .width(pct(100.0))
        .height(layout_style.progress_hit_height)
        .disable(seek_disabled)
        .style_full_with_style_sheet(video_slider_style(style.clone(), visual.clone()))
        .on_change(ValueCommand::new(move |_, fraction| {
            seek_to_fraction(&seek_controller, fraction);
        }))
        .on_change_end(ValueCommand::new(move |_, fraction| {
            seek_to_fraction(&seek_end_controller, fraction);
        }));

    let progress_track = Stack::new()
        .width(pct(100.0))
        .height(layout_style.progress_hit_height)
        .center()
        .child(
            ProgressBar::new(buffered)
                .show_label(false)
                .width(pct(100.0))
                .height(layout_style.progress_height)
                .style_full_with_style_sheet(video_progress_style(style.clone(), visual.clone())),
        )
        .child(seek);

    let mut controls = Flex::horizontal()
        .width(pct(100.0))
        .align(Align::Center)
        .gap(layout_style.controls_gap)
        .child(play_button)
        .child(time_text(controller.clone(), style.clone(), visual.clone()))
        .child(Stack::<VM>::new().grow(1.0));

    if show_volume {
        let mute_controller = controller.clone();
        let mute_icons = vec![
            video_icon_with_opacity(
                SvgIconId::VolumeMute,
                style.clone(),
                visual.clone(),
                false,
                volume_icon_opacity(
                    controller.muted(),
                    controller.volume(),
                    SvgIconId::VolumeMute,
                ),
            ),
            video_icon_with_opacity(
                SvgIconId::VolumeOff,
                style.clone(),
                visual.clone(),
                false,
                volume_icon_opacity(
                    controller.muted(),
                    controller.volume(),
                    SvgIconId::VolumeOff,
                ),
            ),
            video_icon_with_opacity(
                SvgIconId::VolumeDown,
                style.clone(),
                visual.clone(),
                false,
                volume_icon_opacity(
                    controller.muted(),
                    controller.volume(),
                    SvgIconId::VolumeDown,
                ),
            ),
            video_icon_with_opacity(
                SvgIconId::VolumeUp,
                style.clone(),
                visual.clone(),
                false,
                volume_icon_opacity(controller.muted(), controller.volume(), SvgIconId::VolumeUp),
            ),
        ];
        let mute = icon_button(
            mute_icons,
            Value::Static(false),
            style.clone(),
            Command::new(move |_| {
                let muted = mute_controller.muted().get();
                mute_controller.set_muted(!muted);
            }),
        );

        let volume_controller = controller.clone();
        let volume = Slider::new(controller.volume(), 0.0, 1.0)
            .step(0.01)
            .width(layout_style.volume_width)
            .height(layout_style.control_button_size)
            .style_full_with_style_sheet(video_slider_style(style.clone(), visual.clone()))
            .on_change(ValueCommand::new(move |_, volume| {
                volume_controller.set_volume(volume);
            }));

        controls = controls.child(mute).child(volume);
    }

    Flex::vertical()
        .width(pct(100.0))
        .position_absolute()
        .left(dp(0.0))
        .right(dp(0.0))
        .bottom(dp(0.0))
        .padding(layout_style.overlay_padding)
        .gap(layout_style.overlay_gap)
        .style_full_with_style_sheet(video_overlay_style(style.clone()))
        .child(progress_track)
        .child(controls)
        .into()
}

fn time_text<VM: 'static>(
    controller: VideoController,
    style: Option<StyleResolver<VideoStyle>>,
    visual_identity: VisualStyle,
) -> Element<VM> {
    let duration = controller.duration();
    let text = controller.position().map(move |position| {
        format!(
            "{} / {}",
            format_duration(position),
            duration
                .get()
                .map(format_duration)
                .unwrap_or_else(|| "--:--".to_string())
        )
    });
    styled_video_text(text, style, visual_identity, |style| {
        (style.time_text_style.clone(), style.time_text_color.clone())
    })
}

fn status_overlay<VM: 'static>(
    controller: VideoController,
    style: Option<StyleResolver<VideoStyle>>,
    visual_identity: VisualStyle,
) -> Element<VM> {
    let layout_style = resolve_video_style_for_layout(style.as_ref());
    let text = status_text(controller, style.clone(), visual_identity);
    Stack::new()
        .position_absolute()
        .left(dp(12.0))
        .top(dp(12.0))
        .padding(layout_style.status_padding)
        .style_full_with_style_sheet(video_status_style(style))
        .child(text)
        .into()
}

fn status_text<VM: 'static>(
    controller: VideoController,
    style: Option<StyleResolver<VideoStyle>>,
    visual_identity: VisualStyle,
) -> Element<VM> {
    let text = controller.playback_state().map(video_status_text);
    styled_video_text(text, style, visual_identity, |style| {
        (
            style.status_text_style.clone(),
            style.status_text_color.clone(),
        )
    })
}

fn styled_video_text<VM: 'static>(
    text: impl IntoTextContent,
    style: Option<StyleResolver<VideoStyle>>,
    visual_identity: VisualStyle,
    text_style: fn(&VideoStyle) -> (crate::ui::theme::TextStyle, Value<Color>),
) -> Element<VM> {
    with_visual_identity(
        Text::new(text)
            .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
                let resolved = resolve_video_style_with_sheet(
                    style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    state,
                );
                let mut text = TextWidgetStyle::default_for_theme(context.theme);
                let (typography, color) = text_style(&resolved);
                text.typography = typography;
                text.color = color;
                text
            })
            .into(),
        &visual_identity,
    )
}

fn icon_button<VM: 'static>(
    icons: Vec<Element<VM>>,
    disabled: Value<bool>,
    style: Option<StyleResolver<VideoStyle>>,
    command: Command<VM>,
) -> Element<VM> {
    let layout_style = resolve_video_style_for_layout(style.as_ref());
    Stack::new()
        .size(
            layout_style.control_button_size,
            layout_style.control_button_size,
        )
        .center()
        .style_full_with_style_sheet(video_icon_button_style(style))
        .opacity(disabled_opacity(
            disabled.clone(),
            layout_style.control_disabled_opacity,
        ))
        .cursor(disabled_cursor(disabled.clone()))
        .child(icons)
        .on_click(guard_disabled_command(disabled, command))
        .into()
}

fn video_icon_with_opacity<VM: 'static>(
    icon: SvgIconId,
    style: Option<StyleResolver<VideoStyle>>,
    visual_identity: VisualStyle,
    disabled: bool,
    opacity: Value<f32>,
) -> Element<VM> {
    let mut icon = video_icon(icon, style, visual_identity, disabled);
    icon.visual.opacity = opacity;
    icon
}

fn video_icon<VM: 'static>(
    icon: SvgIconId,
    style: Option<StyleResolver<VideoStyle>>,
    visual_identity: VisualStyle,
    disabled: bool,
) -> Element<VM> {
    let layout_style = resolve_video_style_for_layout(style.as_ref());
    with_visual_identity(
        Icon::internal(icon)
            .size(layout_style.control_icon_size)
            .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
                let resolved = resolve_video_style_with_sheet(
                    style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    state,
                );
                let mut icon = IconStyle::default_for_theme(context.theme);
                icon.size = resolved.control_icon_size;
                icon.color = if disabled {
                    resolved.control_icon_disabled_color
                } else {
                    resolved.control_icon_color
                };
                icon
            })
            .into(),
        &visual_identity,
    )
}

fn video_overlay_style(
    style: Option<StyleResolver<VideoStyle>>,
) -> impl Fn(&StyleContext<'_>, &StyleSheet, &VisualStyle, WidgetState) -> ContainerStyle
       + Send
       + Sync
       + 'static {
    move |context, style_sheet, visual, state| {
        let resolved =
            resolve_video_style_with_sheet(style.as_ref(), context, style_sheet, visual, state);
        let mut container = ContainerStyle::default_for_theme(context.theme);
        container.surface.background = Some(resolved.overlay_background);
        container
    }
}

fn video_status_style(
    style: Option<StyleResolver<VideoStyle>>,
) -> impl Fn(&StyleContext<'_>, &StyleSheet, &VisualStyle, WidgetState) -> ContainerStyle
       + Send
       + Sync
       + 'static {
    move |context, style_sheet, visual, state| {
        let resolved =
            resolve_video_style_with_sheet(style.as_ref(), context, style_sheet, visual, state);
        let mut container = ContainerStyle::default_for_theme(context.theme);
        container.surface.background = Some(resolved.status_background);
        container.surface.border_radius = Some(Value::Static(context.theme.radius.full));
        container
    }
}

fn video_icon_button_style(
    style: Option<StyleResolver<VideoStyle>>,
) -> impl Fn(&StyleContext<'_>, &StyleSheet, &VisualStyle, WidgetState) -> ContainerStyle
       + Send
       + Sync
       + 'static {
    move |context, style_sheet, visual, state| {
        let _ = resolve_video_style_with_sheet(style.as_ref(), context, style_sheet, visual, state);
        let mut container = ContainerStyle::default_for_theme(context.theme);
        if state.hovered || state.pressed {
            container.surface.background = Some(Color::hexa(0xFFFFFF24).into());
        }
        container.surface.border_radius = Some(Value::Static(context.theme.radius.full));
        container
    }
}

fn video_progress_style(
    style: Option<StyleResolver<VideoStyle>>,
    visual_identity: VisualStyle,
) -> impl Fn(&StyleContext<'_>, &StyleSheet, &VisualStyle, WidgetState) -> ProgressBarStyle
       + Send
       + Sync
       + 'static {
    move |context, style_sheet, _visual, state| {
        let resolved = resolve_video_style_with_sheet(
            style.as_ref(),
            context,
            style_sheet,
            &visual_identity,
            state,
        );
        let mut progress = ProgressBarStyle::default_for_theme(context.theme);
        progress.track_color = resolved.progress_track_color;
        progress.fill_color = resolved.progress_buffered_color;
        progress.height = resolved.progress_height;
        progress.radius = Value::Static(context.theme.radius.full);
        progress
    }
}

fn video_slider_style(
    style: Option<StyleResolver<VideoStyle>>,
    visual_identity: VisualStyle,
) -> impl Fn(&StyleContext<'_>, &StyleSheet, &VisualStyle, WidgetState) -> SliderStyle
       + Send
       + Sync
       + 'static {
    move |context, style_sheet, _visual, state| {
        let resolved = resolve_video_style_with_sheet(
            style.as_ref(),
            context,
            style_sheet,
            &visual_identity,
            state,
        );
        let mut slider = SliderStyle::default_for_theme(context.theme);
        slider.track = StateValue::new(resolved.progress_track_color.clone());
        slider.active_track = StateValue::new(resolved.progress_active_color.clone());
        slider.thumb = StateValue::new(resolved.progress_thumb_color.clone());
        slider.thumb_shadow = None;
        slider.track_height = resolved.progress_height;
        slider.thumb_size = dp(12.0);
        slider.radius = Value::Static(context.theme.radius.full);
        slider.border_width = Value::Static(dp(0.0));
        slider.min_width = dp(0.0);
        slider.min_height = resolved.progress_hit_height;
        slider
    }
}

fn guard_disabled_command<VM: 'static>(disabled: Value<bool>, command: Command<VM>) -> Command<VM> {
    Command::new_with_context(move |vm, ctx| {
        if !disabled.resolve() {
            command.execute_with_context(vm, ctx);
        }
    })
}

fn disabled_opacity(disabled: Value<bool>, opacity: f32) -> Value<f32> {
    match disabled {
        Value::Static(disabled) => Value::Static(if disabled { opacity } else { 1.0 }),
        Value::Signal(disabled) => {
            Value::Signal(disabled.map(move |disabled| if disabled { opacity } else { 1.0 }))
        }
    }
}

fn disabled_cursor(disabled: Value<bool>) -> Value<CursorStyle> {
    match disabled {
        Value::Static(disabled) => Value::Static(if disabled {
            CursorStyle::NotAllowed
        } else {
            CursorStyle::Pointer
        }),
        Value::Signal(disabled) => Value::Signal(disabled.map(|disabled| {
            if disabled {
                CursorStyle::NotAllowed
            } else {
                CursorStyle::Pointer
            }
        })),
    }
}

fn progress_signal(controller: &VideoController) -> Value<f32> {
    let duration = controller.duration();
    controller
        .position()
        .map(move |position| duration_fraction(position, duration.get()))
        .into()
}

fn buffered_signal(controller: &VideoController) -> Value<f32> {
    let duration = controller.duration();
    controller
        .buffered_position()
        .map(move |buffered| match buffered {
            Some(buffered) => duration_fraction(buffered, duration.get()),
            None => 0.0,
        })
        .into()
}

fn seek_to_fraction(controller: &VideoController, fraction: f32) {
    let Some(duration) = controller.duration().get() else {
        return;
    };
    if duration.is_zero() {
        return;
    }
    let seconds = duration.as_secs_f64() * fraction.clamp(0.0, 1.0) as f64;
    controller.seek(Duration::from_secs_f64(seconds));
}

fn duration_fraction(position: Duration, duration: Option<Duration>) -> f32 {
    let Some(duration) = duration else {
        return 0.0;
    };
    if duration.is_zero() {
        return 0.0;
    }
    (position.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0) as f32
}

fn format_duration(duration: Duration) -> String {
    let total = duration.as_secs();
    let seconds = total % 60;
    let minutes = (total / 60) % 60;
    let hours = total / 3600;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

fn playback_button_icon(state: VideoPlaybackState) -> SvgIconId {
    match state {
        VideoPlaybackState::Playing => SvgIconId::Pause,
        _ => SvgIconId::PlayArrow,
    }
}

fn playback_icon_opacity(playback: Signal<VideoPlaybackState>, icon: SvgIconId) -> Value<f32> {
    Value::Signal(playback.map_memo(move |state| {
        if playback_button_icon(state) == icon {
            1.0
        } else {
            0.0
        }
    }))
}

fn playback_button_disabled(state: VideoPlaybackState) -> bool {
    matches!(
        state,
        VideoPlaybackState::Loading | VideoPlaybackState::Buffering | VideoPlaybackState::Error(_)
    )
}

fn volume_button_icon(muted: bool, volume: f32) -> SvgIconId {
    if muted {
        SvgIconId::VolumeMute
    } else if volume <= 0.0 {
        SvgIconId::VolumeOff
    } else if volume < 0.5 {
        SvgIconId::VolumeDown
    } else {
        SvgIconId::VolumeUp
    }
}

fn volume_icon_opacity(muted: Signal<bool>, volume: Signal<f32>, icon: SvgIconId) -> Value<f32> {
    Value::Signal(muted.map_memo(move |muted| {
        if volume_button_icon(muted, volume.get_untracked()) == icon {
            1.0
        } else {
            0.0
        }
    }))
}

fn video_status_text(state: VideoPlaybackState) -> String {
    match state {
        VideoPlaybackState::Idle => "Idle".to_string(),
        VideoPlaybackState::Loading => "Loading".to_string(),
        VideoPlaybackState::Ready => "Ready".to_string(),
        VideoPlaybackState::Playing => "Playing".to_string(),
        VideoPlaybackState::Paused => "Paused".to_string(),
        VideoPlaybackState::Buffering => "Buffering".to_string(),
        VideoPlaybackState::Ended => "Ended".to_string(),
        VideoPlaybackState::Error(error) => format!("Error: {error}"),
    }
}

fn resolve_video_style(
    style: Option<&StyleResolver<VideoStyle>>,
    context: &StyleContext<'_>,
) -> VideoStyle {
    let style_sheet = StyleSheet::default();
    resolve_video_style_with_sheet(
        style,
        context,
        &style_sheet,
        &VisualStyle::default(),
        WidgetState::default(),
    )
}

fn resolve_video_style_with_sheet(
    style: Option<&StyleResolver<VideoStyle>>,
    context: &StyleContext<'_>,
    style_sheet: &StyleSheet,
    visual: &VisualStyle,
    state: WidgetState,
) -> VideoStyle {
    resolve_component_style_with_sheet(
        style,
        context,
        style_sheet,
        visual,
        state,
        VideoStyle::default_for_theme(context.theme),
        |base, context| context.theme.components.video.apply(base, context),
        |sheet, base, context, visual| sheet.apply_video(base, context, visual),
        |sheet, base, context, visual, state| sheet.apply_video_state(base, context, visual, state),
    )
}

fn resolve_video_style_for_layout(style: Option<&StyleResolver<VideoStyle>>) -> VideoStyle {
    let theme = Theme::default();
    let context = StyleContext::from_theme(&theme);
    resolve_video_style(style, &context)
}

impl<VM> From<VideoSurface> for Element<VM> {
    fn from(value: VideoSurface) -> Self {
        Element {
            id: WidgetId::next(),
            key: value.key.clone(),
            layout: value.layout.clone(),
            focus: Default::default(),
            visual: value.visual.clone(),
            interactions: InteractionHandlers {
                cursor_style: value.cursor_style.clone(),
                ..Default::default()
            },
            lifecycle_events: LifecycleEventHandlers::default(),
            media_events: MediaEventHandlers::default(),
            background: value.background.clone(),
            tooltip: None,
            popover: None,
            menu: None,
            context_menu: None,
            modal: None,
            drawer: None,
            tab_trigger: None,
            list_item: None,
            tree_root: None,
            tree_node: None,
            data_grid_root: None,
            data_grid_cell: None,
            data_grid_header: None,
            data_grid_resize_handle: None,
            splitter_handle: None,
            carousel_auto_play: None,
            kind: WidgetKind::VideoSurface {
                style: value.style.clone(),
                video: value,
            },
        }
    }
}
