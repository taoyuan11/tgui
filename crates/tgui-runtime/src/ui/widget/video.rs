use std::time::Duration;

use crate::foundation::binding::Signal;
use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::media::ContentFit;
use crate::theme::{StyleContext, WidgetState};
use crate::ui::layout::{pct, Align, Insets, Justify, LayoutStyle, Overflow, Value};
use crate::ui::theme::StateValue;
use crate::ui::unit::{dp, sp, Dp};
use crate::video::{
    VideoAudioTrack, VideoAudioTrackSelection, VideoController, VideoPlaybackState,
    VideoSubtitleCuePlacement, VideoSubtitleCueStyle, VideoSubtitleHorizontalAlign,
    VideoSubtitleTrack, VideoSubtitleTrackSelection, VideoSubtitleVerticalAlign,
};

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
    ContainerStyle, IconStyle, ProgressBarStyle, SelectStyle, SliderStyle, StyleResolver,
    StyleSheet, TextWidgetStyle, VideoStyle, VideoSurfaceStyle,
};
use super::{Flex, IntoTextContent, ProgressBar, Select, SelectOption, Slider, Stack, Text};

const PLAYBACK_RATE_OPTIONS: &[(i32, &str)] = &[
    (25, "Speed: 0.25x"),
    (50, "Speed: 0.5x"),
    (75, "Speed: 0.75x"),
    (100, "Speed: 1x"),
    (125, "Speed: 1.25x"),
    (150, "Speed: 1.5x"),
    (175, "Speed: 1.75x"),
    (200, "Speed: 2x"),
    (400, "Speed: 4x"),
];
const DEFAULT_ASS_SUBTITLE_EFFECT_CENTI_PX: u16 = 100;

#[derive(Clone, Copy)]
enum SubtitleTextLayer {
    Foreground,
    Outline(Color),
    Shadow(Color),
}

/// Displays decoded video frames from a [`VideoController`].
///
/// `VideoSurface` renders only the media surface and placeholder states. Use
/// [`Video`] when you want the built-in play/pause, seek, time, and volume
/// controls.
#[derive(Clone)]
pub struct VideoSurface {
    pub(crate) key: Option<WidgetKey>,
    pub(crate) layout: LayoutStyle,
    pub(crate) visual: VisualStyle,
    pub(crate) controller: VideoController,
    pub(crate) background: Option<Value<Color>>,
    pub(crate) fit: ContentFit,
    pub(crate) fit_overridden: bool,
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
    /// Creates a surface bound to a video controller.
    pub fn new(controller: VideoController) -> Self {
        Self {
            key: None,
            layout: LayoutStyle::default(),
            visual: VisualStyle::default(),
            controller,
            background: None,
            fit: ContentFit::Contain,
            fit_overridden: false,
            cursor_style: None,
            style: None,
        }
    }

    impl_video_layout_api!();

    /// Assigns a stable widget key.
    pub fn key(mut self, key: impl Into<WidgetKey>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// Mutates the resolved video surface style.
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

    /// Replaces the complete video surface style resolver.
    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> VideoSurfaceStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(super::style::StyleResolver::full(resolver));
        self
    }

    /// Sets the cursor used when hovering the surface.
    pub fn cursor(mut self, cursor: impl Into<Value<CursorStyle>>) -> Self {
        self.cursor_style = Some(cursor.into());
        self
    }

    /// Sets how decoded frames fit inside the surface bounds.
    pub fn fit(mut self, fit: ContentFit) -> Self {
        self.fit = fit;
        self.fit_overridden = true;
        self
    }

    /// Registers a click handler for the surface.
    pub fn on_click<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_click: Some(command),
            ..Default::default()
        })
    }

    /// Registers a double-click handler for the surface.
    pub fn on_double_click<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_double_click: Some(command),
            ..Default::default()
        })
    }

    /// Registers a mouse-enter handler for the surface.
    pub fn on_mouse_enter<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_mouse_enter: Some(command),
            ..Default::default()
        })
    }

    /// Registers a mouse-leave handler for the surface.
    pub fn on_mouse_leave<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_mouse_leave: Some(command),
            ..Default::default()
        })
    }

    /// Registers a mouse-move handler for the surface.
    pub fn on_mouse_move<VM>(self, command: ValueCommand<VM, Point>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_mouse_move: Some(command),
            ..Default::default()
        })
    }

    /// Registers a mount lifecycle handler.
    pub fn on_mount<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_lifecycle_events(LifecycleEventHandlers {
            on_mount: Some(command),
            ..Default::default()
        })
    }

    /// Registers an unmount lifecycle handler.
    pub fn on_unmount<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_lifecycle_events(LifecycleEventHandlers {
            on_unmount: Some(command),
            ..Default::default()
        })
    }

    /// Registers an update lifecycle handler.
    pub fn on_update<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_lifecycle_events(LifecycleEventHandlers {
            on_update: Some(command),
            ..Default::default()
        })
    }

    /// Registers a media loading handler.
    pub fn on_loading<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_media_events(MediaEventHandlers {
            on_loading: Some(command),
            ..Default::default()
        })
    }

    /// Registers a media success handler fired when loading completes.
    pub fn on_success<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_media_events(MediaEventHandlers {
            on_success: Some(command),
            ..Default::default()
        })
    }

    /// Registers a media error handler.
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

/// A video player widget with built-in controls.
///
/// `Video` composes a [`VideoSurface`] with optional playback controls, status
/// text, seek progress, and volume controls. Playback behavior is driven by the
/// supplied [`VideoController`].
pub struct Video<VM> {
    controller: VideoController,
    show_controls: Value<bool>,
    show_status: Value<bool>,
    show_volume: Value<bool>,
    show_looping: Value<bool>,
    show_playback_rate: Value<bool>,
    show_audio_tracks: Value<bool>,
    show_subtitle_tracks: Value<bool>,
    show_subtitles: Value<bool>,
    fit: ContentFit,
    style: Option<StyleResolver<VideoStyle>>,
    layout: LayoutStyle,
    visual: VisualStyle,
    key: Option<WidgetKey>,
    media_events: MediaEventHandlers<VM>,
}

impl<VM> Video<VM> {
    /// Creates a video player bound to a controller.
    pub fn new(controller: VideoController) -> Self {
        Self {
            controller,
            show_controls: Value::Static(true),
            show_status: Value::Static(true),
            show_volume: Value::Static(true),
            show_looping: Value::Static(false),
            show_playback_rate: Value::Static(false),
            show_audio_tracks: Value::Static(true),
            show_subtitle_tracks: Value::Static(true),
            show_subtitles: Value::Static(true),
            fit: ContentFit::Contain,
            style: None,
            layout: LayoutStyle::default(),
            visual: VisualStyle::default(),
            key: None,
            media_events: MediaEventHandlers::default(),
        }
    }

    impl_p3_layout_api!(layout);

    /// Shows or hides the built-in playback controls.
    pub fn show_controls(mut self, show_controls: impl Into<Value<bool>>) -> Self {
        self.show_controls = show_controls.into();
        self
    }

    /// Shows or hides the status text overlay.
    pub fn show_status(mut self, show_status: impl Into<Value<bool>>) -> Self {
        self.show_status = show_status.into();
        self
    }

    /// Shows or hides the mute and volume controls.
    pub fn show_volume(mut self, show_volume: impl Into<Value<bool>>) -> Self {
        self.show_volume = show_volume.into();
        self
    }

    /// Shows or hides the built-in looping toggle.
    ///
    /// The toggle forwards changes to [`VideoController::set_looping`]. It is
    /// hidden by default so existing player layouts keep their current density.
    pub fn show_looping(mut self, show_looping: impl Into<Value<bool>>) -> Self {
        self.show_looping = show_looping.into();
        self
    }

    /// Shows or hides the built-in playback rate selector.
    ///
    /// The selector offers common media-player speed presets and forwards
    /// changes to [`VideoController::set_playback_rate`].
    pub fn show_playback_rate(mut self, show_playback_rate: impl Into<Value<bool>>) -> Self {
        self.show_playback_rate = show_playback_rate.into();
        self
    }

    /// Shows or hides the built-in audio track selector.
    ///
    /// The selector is only rendered after the backend discovers at least one
    /// audio track in the loaded source.
    pub fn show_audio_tracks(mut self, show_audio_tracks: impl Into<Value<bool>>) -> Self {
        self.show_audio_tracks = show_audio_tracks.into();
        self
    }

    /// Shows or hides the built-in subtitle track selector.
    ///
    /// The selector is only rendered after the backend discovers at least one
    /// subtitle track in the loaded source.
    pub fn show_subtitle_tracks(mut self, show_subtitle_tracks: impl Into<Value<bool>>) -> Self {
        self.show_subtitle_tracks = show_subtitle_tracks.into();
        self
    }

    /// Shows or hides the built-in subtitle text overlay.
    ///
    /// The overlay is only rendered while a selected text subtitle cue is
    /// active. Bitmap subtitle tracks are ignored by the text overlay.
    pub fn show_subtitles(mut self, show_subtitles: impl Into<Value<bool>>) -> Self {
        self.show_subtitles = show_subtitles.into();
        self
    }

    /// Sets how decoded frames fit inside the surface bounds.
    pub fn fit(mut self, fit: ContentFit) -> Self {
        self.fit = fit;
        self
    }

    /// Mutates the resolved video player style.
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

    /// Replaces the complete video player style resolver.
    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> VideoStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }

    /// Registers a media loading handler on the inner surface.
    pub fn on_loading(mut self, command: Command<VM>) -> Self {
        self.media_events.on_loading = Some(command);
        self
    }

    /// Registers a media success handler on the inner surface.
    pub fn on_success(mut self, command: Command<VM>) -> Self {
        self.media_events.on_success = Some(command);
        self
    }

    /// Registers a media error handler on the inner surface.
    pub fn on_error(mut self, command: ValueCommand<VM, String>) -> Self {
        self.media_events.on_error = Some(command);
        self
    }
}

impl<VM: 'static> From<Video<VM>> for Element<VM> {
    fn from(video: Video<VM>) -> Self {
        let controller = video.controller.clone();
        let show_controls = video.show_controls.resolve();
        let show_status = video.show_status.resolve();
        let show_volume = video.show_volume.resolve();
        let show_looping = video.show_looping.resolve();
        let show_playback_rate = video.show_playback_rate.resolve();
        let show_audio_tracks = video.show_audio_tracks.resolve();
        let show_subtitle_tracks = video.show_subtitle_tracks.resolve();
        let show_subtitles = video.show_subtitles.resolve();
        let fit = video.fit;

        let surface_style = video.style.clone();
        let mut surface: Element<VM> = VideoSurface::new(controller.clone())
            .size(pct(100.0), pct(100.0))
            .position_absolute()
            .inset(dp(0.0))
            .style(move |style, context| {
                style.fit = fit;
                style.surface.background = Some(Color::hexa(0x000000FF).into());
                let resolved = resolve_video_style(surface_style.as_ref(), context);
                style.surface.border_radius = Some(Value::Static(resolved.radius));
            })
            .into();
        surface.media_events = video.media_events.clone();

        let root_style = video.style.clone();
        let root_layout_style = video.style.clone();
        let mut root = Stack::new()
            .aspect_ratio(16.0 / 9.0)
            .runtime_layout(move |layout, _container, context, style_sheet, visual| {
                let resolved = resolve_video_style_with_sheet(
                    root_layout_style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    WidgetState::default(),
                );
                if layout.width.is_none() {
                    layout.width = Some(Value::Static(crate::ui::layout::Length::Px(
                        resolved.default_surface_width,
                    )));
                }
                if layout.height.is_none() {
                    layout.height = Some(Value::Static(crate::ui::layout::Length::Px(
                        resolved.default_surface_height,
                    )));
                }
                if layout.padding.is_none() {
                    layout.padding = Some(Value::Static(resolved.padding));
                }
            })
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

        if let Some(subtitles) = maybe_subtitle_overlay(
            &controller,
            show_subtitles,
            show_controls,
            video.style.clone(),
            video.visual.clone(),
        ) {
            root = root.child(subtitles);
        }

        if show_controls {
            root = root.child(video_controls(
                controller.clone(),
                show_volume,
                show_looping,
                show_playback_rate,
                show_audio_tracks,
                show_subtitle_tracks,
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

#[inline(never)]
fn maybe_subtitle_overlay<VM: 'static>(
    controller: &VideoController,
    show_subtitles: bool,
    controls_visible: bool,
    style: Option<StyleResolver<VideoStyle>>,
    visual: VisualStyle,
) -> Option<Element<VM>> {
    if !show_subtitles {
        return None;
    }

    let subtitle = controller
        .current_subtitle()
        .get()
        .filter(|cue| !cue.text.trim().is_empty())?;
    let placement = controller
        .current_subtitle_placement()
        .get()
        .unwrap_or_default();
    let subtitle_style = controller
        .current_subtitle_style()
        .get()
        .unwrap_or_default();
    Some(subtitle_overlay(
        subtitle.text,
        placement,
        subtitle_style,
        controls_visible,
        style,
        visual,
    ))
}

fn video_controls<VM: 'static>(
    controller: VideoController,
    show_volume: bool,
    show_looping: bool,
    show_playback_rate: bool,
    show_audio_tracks: bool,
    show_subtitle_tracks: bool,
    style: Option<StyleResolver<VideoStyle>>,
    visual: VisualStyle,
) -> Element<VM> {
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
        .runtime_layout({
            let style = style.clone();
            move |layout, context, style_sheet, visual| {
                let resolved = resolve_video_style_with_sheet(
                    style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    WidgetState::default(),
                );
                if layout.height.is_none() {
                    layout.height = Some(Value::Static(crate::ui::layout::Length::Px(
                        resolved.progress_hit_height,
                    )));
                }
            }
        })
        .disable(seek_disabled)
        .style_full_with_style_sheet(video_slider_style(style.clone(), visual.clone()))
        .on_change(ValueCommand::new(move |_, fraction| {
            seek_to_fraction(&seek_controller, fraction);
        }))
        .on_change_end(ValueCommand::new(move |_, fraction| {
            seek_to_fraction(&seek_end_controller, fraction);
        }));

    let progress_layout_style = style.clone();
    let progress_track = Stack::new()
        .width(pct(100.0))
        .runtime_layout(move |layout, _container, context, style_sheet, visual| {
            let resolved = resolve_video_style_with_sheet(
                progress_layout_style.as_ref(),
                context,
                style_sheet,
                visual,
                WidgetState::default(),
            );
            layout.height = Some(Value::Static(crate::ui::layout::Length::Px(
                resolved.progress_hit_height,
            )));
        })
        .center()
        .child(
            ProgressBar::new(buffered)
                .show_label(false)
                .width(pct(100.0))
                .style_full_with_style_sheet(video_progress_style(style.clone(), visual.clone())),
        )
        .child(seek);

    let controls_style = style.clone();
    let mut controls = Flex::horizontal()
        .width(pct(100.0))
        .align(Align::Center)
        .gap(crate::ui::unit::dp(0.0))
        .runtime_layout(move |_layout, container, context, style_sheet, visual| {
            let resolved = resolve_video_style_with_sheet(
                controls_style.as_ref(),
                context,
                style_sheet,
                visual,
                WidgetState::default(),
            );
            container.gap = Value::Static(crate::ui::layout::Length::Px(resolved.controls_gap));
        })
        .child(play_button);

    if show_looping {
        controls = controls.child(looping_button(
            controller.clone(),
            style.clone(),
            visual.clone(),
        ));
    }

    controls = controls
        .child(time_text(controller.clone(), style.clone(), visual.clone()))
        .child(Stack::<VM>::new().grow(1.0));

    if show_playback_rate {
        controls = controls.child(playback_rate_selector(
            controller.clone(),
            style.clone(),
            visual.clone(),
        ));
    }

    if show_audio_tracks {
        if let Some(selector) =
            audio_track_selector(controller.clone(), style.clone(), visual.clone())
        {
            controls = controls.child(selector);
        }
    }

    if show_subtitle_tracks {
        if let Some(selector) =
            subtitle_track_selector(controller.clone(), style.clone(), visual.clone())
        {
            controls = controls.child(selector);
        }
    }

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
        let volume_layout_style = style.clone();
        let volume = Slider::new(controller.volume(), 0.0, 1.0)
            .step(0.01)
            .runtime_layout(move |layout, context, style_sheet, visual| {
                let resolved = resolve_video_style_with_sheet(
                    volume_layout_style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    WidgetState::default(),
                );
                if layout.width.is_none() {
                    layout.width = Some(Value::Static(crate::ui::layout::Length::Px(
                        resolved.volume_width,
                    )));
                }
                if layout.height.is_none() {
                    layout.height = Some(Value::Static(crate::ui::layout::Length::Px(
                        resolved.control_button_size,
                    )));
                }
            })
            .style_full_with_style_sheet(video_slider_style(style.clone(), visual.clone()))
            .on_change(ValueCommand::new(move |_, volume| {
                volume_controller.set_volume(volume);
            }));

        controls = controls.child(mute).child(volume);
    }

    let overlay_style = style.clone();
    Flex::vertical()
        .width(pct(100.0))
        .position_absolute()
        .left(dp(0.0))
        .right(dp(0.0))
        .bottom(dp(0.0))
        .padding(crate::ui::layout::Insets::ZERO)
        .gap(crate::ui::unit::dp(0.0))
        .runtime_layout(move |layout, container, context, style_sheet, visual| {
            let resolved = resolve_video_style_with_sheet(
                overlay_style.as_ref(),
                context,
                style_sheet,
                visual,
                WidgetState::default(),
            );
            layout.height = Some(Value::Static(crate::ui::layout::Length::Px(
                resolved.control_button_size
                    + resolved.progress_hit_height
                    + resolved.overlay_gap
                    + resolved.overlay_padding.top
                    + resolved.overlay_padding.bottom,
            )));
            layout.padding = Some(Value::Static(resolved.overlay_padding));
            container.gap = Value::Static(crate::ui::layout::Length::Px(resolved.overlay_gap));
        })
        .style_full_with_style_sheet(video_overlay_style(style.clone()))
        .child(progress_track)
        .child(controls)
        .into()
}

#[inline(never)]
fn subtitle_overlay<VM: 'static>(
    text: String,
    placement: VideoSubtitleCuePlacement,
    subtitle_style: VideoSubtitleCueStyle,
    controls_visible: bool,
    style: Option<StyleResolver<VideoStyle>>,
    visual_identity: VisualStyle,
) -> Element<VM> {
    let overlay_layout_style = style.clone();
    let vertical = placement.vertical;
    let overlay = Flex::vertical()
        .width(pct(100.0))
        .position_absolute()
        .left(dp(0.0))
        .right(dp(0.0))
        .align(subtitle_horizontal_align(placement.horizontal))
        .runtime_layout(move |layout, container, context, style_sheet, visual| {
            let resolved = resolve_video_style_with_sheet(
                overlay_layout_style.as_ref(),
                context,
                style_sheet,
                visual,
                WidgetState::default(),
            );
            layout.top = None;
            layout.bottom = None;
            container.justify = Justify::Start;
            match vertical {
                VideoSubtitleVerticalAlign::Top => {
                    layout.top = Some(Value::Static(crate::ui::layout::Length::Px(
                        resolved.subtitle_bottom_offset,
                    )));
                }
                VideoSubtitleVerticalAlign::Middle => {
                    layout.top = Some(Value::Static(crate::ui::layout::Length::Px(Dp::ZERO)));
                    layout.bottom = Some(Value::Static(crate::ui::layout::Length::Px(Dp::ZERO)));
                    container.justify = Justify::Center;
                }
                VideoSubtitleVerticalAlign::Bottom => {
                    let controls_height = resolved.control_button_size
                        + resolved.progress_hit_height
                        + resolved.overlay_gap
                        + resolved.overlay_padding.top
                        + resolved.overlay_padding.bottom;
                    let bottom = if controls_visible {
                        controls_height + resolved.subtitle_bottom_offset
                    } else {
                        resolved.subtitle_bottom_offset
                    };
                    layout.bottom = Some(Value::Static(crate::ui::layout::Length::Px(bottom)));
                }
            }
        });
    let subtitle_box_layout_style = style.clone();
    let mut subtitle_box = Stack::new()
        .max_width(pct(100.0))
        .padding(Insets::ZERO)
        .runtime_layout(move |layout, _container, context, style_sheet, visual| {
            let resolved = resolve_video_style_with_sheet(
                subtitle_box_layout_style.as_ref(),
                context,
                style_sheet,
                visual,
                WidgetState::default(),
            );
            layout.padding = Some(Value::Static(resolved.subtitle_padding));
        })
        .style_full_with_style_sheet(video_subtitle_surface_style(style.clone()));

    if let Some((color, offset)) = subtitle_shadow_effect(subtitle_style) {
        subtitle_box = subtitle_box.child(positioned_subtitle_text_layer(
            text.clone(),
            style.clone(),
            subtitle_style,
            SubtitleTextLayer::Shadow(color),
            offset,
        ));
    }

    if let Some((color, width)) = subtitle_outline_effect(subtitle_style) {
        for (x, y) in subtitle_outline_offsets(width) {
            subtitle_box = subtitle_box.child(positioned_subtitle_text_layer(
                text.clone(),
                style.clone(),
                subtitle_style,
                SubtitleTextLayer::Outline(color),
                (x, y),
            ));
        }
    }

    subtitle_box = subtitle_box.child(subtitle_text_layer(
        text,
        style,
        subtitle_style,
        SubtitleTextLayer::Foreground,
    ));

    overlay
        .child(with_visual_identity(
            Element::from(subtitle_box),
            &visual_identity,
        ))
        .into()
}

fn subtitle_text_layer(
    text: String,
    style: Option<StyleResolver<VideoStyle>>,
    subtitle_style: VideoSubtitleCueStyle,
    layer: SubtitleTextLayer,
) -> Text {
    Text::new(text)
        .max_width(pct(100.0))
        .style_full_with_style_sheet(video_subtitle_text_style(style, subtitle_style, layer))
}

fn positioned_subtitle_text_layer<VM: 'static>(
    text: String,
    style: Option<StyleResolver<VideoStyle>>,
    subtitle_style: VideoSubtitleCueStyle,
    layer: SubtitleTextLayer,
    offset: (Dp, Dp),
) -> Element<VM> {
    let layout_style = style.clone();
    Stack::new()
        .position_absolute()
        .runtime_layout(move |layout, _container, context, style_sheet, visual| {
            let resolved = resolve_video_style_with_sheet(
                layout_style.as_ref(),
                context,
                style_sheet,
                visual,
                WidgetState::default(),
            );
            layout.left = Some(Value::Static(crate::ui::layout::Length::Px(
                resolved.subtitle_padding.left + offset.0,
            )));
            layout.top = Some(Value::Static(crate::ui::layout::Length::Px(
                resolved.subtitle_padding.top + offset.1,
            )));
        })
        .child(subtitle_text_layer(text, style, subtitle_style, layer))
        .into()
}

fn subtitle_outline_effect(subtitle_style: VideoSubtitleCueStyle) -> Option<(Color, Dp)> {
    let requested =
        subtitle_style.outline_color.is_some() || subtitle_style.outline_width_centi_px.is_some();
    let width = subtitle_effect_dp(
        subtitle_style.outline_width_centi_px,
        DEFAULT_ASS_SUBTITLE_EFFECT_CENTI_PX,
    );
    (requested && width > Dp::ZERO)
        .then_some((subtitle_style.outline_color.unwrap_or(Color::BLACK), width))
}

fn subtitle_shadow_effect(subtitle_style: VideoSubtitleCueStyle) -> Option<(Color, (Dp, Dp))> {
    let requested =
        subtitle_style.shadow_color.is_some() || subtitle_style.shadow_depth_centi_px.is_some();
    let depth = subtitle_effect_dp(
        subtitle_style.shadow_depth_centi_px,
        DEFAULT_ASS_SUBTITLE_EFFECT_CENTI_PX,
    );
    (requested && depth > Dp::ZERO).then_some((
        subtitle_style.shadow_color.unwrap_or(Color::BLACK),
        (depth, depth),
    ))
}

fn subtitle_outline_offsets(width: Dp) -> [(Dp, Dp); 8] {
    [
        (-width, Dp::ZERO),
        (width, Dp::ZERO),
        (Dp::ZERO, -width),
        (Dp::ZERO, width),
        (-width, -width),
        (width, -width),
        (-width, width),
        (width, width),
    ]
}

fn subtitle_effect_dp(value: Option<u16>, fallback: u16) -> Dp {
    dp(value.unwrap_or(fallback) as f32 / 100.0)
}

fn subtitle_horizontal_align(alignment: VideoSubtitleHorizontalAlign) -> Align {
    match alignment {
        VideoSubtitleHorizontalAlign::Left => Align::Start,
        VideoSubtitleHorizontalAlign::Center => Align::Center,
        VideoSubtitleHorizontalAlign::Right => Align::End,
    }
}

fn looping_button<VM: 'static>(
    controller: VideoController,
    style: Option<StyleResolver<VideoStyle>>,
    visual_identity: VisualStyle,
) -> Element<VM> {
    let loop_controller = controller.clone();
    let looping = controller.looping();
    let initial_inactive_opacity =
        VideoStyle::default_for_theme(&crate::ui::theme::Theme::default()).control_disabled_opacity;
    let runtime_looping = looping.clone();
    let runtime_style = style.clone();
    let button_style = style.clone();
    Stack::new()
        .runtime_layout(move |layout, _container, context, style_sheet, visual| {
            let resolved = resolve_video_style_with_sheet(
                runtime_style.as_ref(),
                context,
                style_sheet,
                visual,
                WidgetState::default(),
            );
            layout.width = Some(Value::Static(crate::ui::layout::Length::Px(
                resolved.control_button_size,
            )));
            layout.height = Some(Value::Static(crate::ui::layout::Length::Px(
                resolved.control_button_size,
            )));
            visual.opacity =
                looping_icon_opacity(runtime_looping.clone(), resolved.control_disabled_opacity);
        })
        .center()
        .style_full_with_style_sheet(video_icon_button_style(button_style))
        .opacity(looping_icon_opacity(
            looping.clone(),
            initial_inactive_opacity,
        ))
        .cursor(disabled_cursor(Value::Static(false)))
        .child(video_icon(SvgIconId::Repeat, style, visual_identity, false))
        .on_click(Command::new(move |_| {
            let looping = loop_controller.looping().get();
            loop_controller.set_looping(!looping);
        }))
        .into()
}

fn looping_icon_opacity(looping: Signal<bool>, inactive_opacity: f32) -> Value<f32> {
    Value::Signal(looping.map_memo(move |looping| if looping { 1.0 } else { inactive_opacity }))
}

fn playback_rate_selector<VM: 'static>(
    controller: VideoController,
    style: Option<StyleResolver<VideoStyle>>,
    visual_identity: VisualStyle,
) -> Element<VM> {
    let mut options = PLAYBACK_RATE_OPTIONS
        .iter()
        .map(|(key, label)| SelectOption::new(*key, (*label).to_string()))
        .collect::<Vec<_>>();
    let current_key = playback_rate_key(controller.playback_rate().get());
    if !PLAYBACK_RATE_OPTIONS
        .iter()
        .any(|(key, _)| *key == current_key)
    {
        options.push(SelectOption::new(
            current_key,
            playback_rate_label(current_key),
        ));
    }

    let selected = controller
        .playback_rate()
        .map(|rate| Some(playback_rate_key(rate)));
    let rate_controller = controller.clone();
    let disabled = controller.playback_state().map(|state| {
        matches!(
            state,
            VideoPlaybackState::Loading | VideoPlaybackState::Error(_)
        )
    });

    let layout_style = style.clone();
    Stack::new()
        .runtime_layout(move |layout, _container, context, style_sheet, visual| {
            let resolved = resolve_video_style_with_sheet(
                layout_style.as_ref(),
                context,
                style_sheet,
                visual,
                WidgetState::default(),
            );
            layout.width = Some(Value::Static(crate::ui::layout::Length::Px(
                resolved.playback_rate_width,
            )));
            layout.height = Some(Value::Static(crate::ui::layout::Length::Px(
                resolved.control_button_size,
            )));
        })
        .child(
            Select::new(options, selected)
                .size(pct(100.0), pct(100.0))
                .disable(disabled)
                .style_full(video_track_select_style(style, visual_identity))
                .on_change(ValueCommand::new(move |_, (rate_key, _label)| {
                    rate_controller.set_playback_rate(rate_key as f32 / 100.0);
                })),
        )
        .into()
}

fn playback_rate_key(rate: f32) -> i32 {
    if !rate.is_finite() {
        return 100;
    }
    (rate.clamp(0.25, 4.0) * 100.0).round() as i32
}

fn playback_rate_label(rate_key: i32) -> String {
    let rate = rate_key as f32 / 100.0;
    if rate.fract() == 0.0 {
        format!("Speed: {}x", rate as i32)
    } else {
        let value = format!("{rate:.2}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string();
        format!("Speed: {value}x")
    }
}

fn audio_track_selector<VM: 'static>(
    controller: VideoController,
    style: Option<StyleResolver<VideoStyle>>,
    visual_identity: VisualStyle,
) -> Option<Element<VM>> {
    let tracks = controller.audio_tracks().get();
    if tracks.is_empty() {
        return None;
    }

    let mut options = Vec::with_capacity(tracks.len() + 2);
    options.push(SelectOption::new(
        VideoAudioTrackSelection::Auto,
        "Audio: Auto".to_string(),
    ));
    options.push(SelectOption::new(
        VideoAudioTrackSelection::Disabled,
        "Audio: Off".to_string(),
    ));
    options.extend(tracks.iter().enumerate().map(|(index, track)| {
        SelectOption::new(
            VideoAudioTrackSelection::Stream(track.stream_index),
            audio_track_label(track, index),
        )
    }));

    let selected = controller
        .audio_track_selection()
        .map(|selection| Some(selection));
    let selection_controller = controller.clone();
    let disabled = controller.playback_state().map(|state| {
        matches!(
            state,
            VideoPlaybackState::Loading | VideoPlaybackState::Error(_)
        )
    });

    let layout_style = style.clone();
    Some(
        Stack::new()
            .runtime_layout(move |layout, _container, context, style_sheet, visual| {
                let resolved = resolve_video_style_with_sheet(
                    layout_style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    WidgetState::default(),
                );
                layout.width = Some(Value::Static(crate::ui::layout::Length::Px(
                    resolved.audio_track_width,
                )));
                layout.height = Some(Value::Static(crate::ui::layout::Length::Px(
                    resolved.control_button_size,
                )));
            })
            .child(
                Select::new(options, selected)
                    .size(pct(100.0), pct(100.0))
                    .disable(disabled)
                    .style_full(video_track_select_style(style, visual_identity))
                    .on_change(ValueCommand::new(move |_, (selection, _label)| {
                        selection_controller.set_audio_track_selection(selection);
                    })),
            )
            .into(),
    )
}

fn audio_track_label(track: &VideoAudioTrack, index: usize) -> String {
    let title = track
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty());
    let language = track
        .language
        .as_deref()
        .map(str::trim)
        .filter(|language| !language.is_empty());

    let base = title
        .map(ToString::to_string)
        .or_else(|| language.map(|language| language.to_ascii_uppercase()))
        .unwrap_or_else(|| format!("Track {}", index + 1));

    let mut details = Vec::new();
    if title.is_some() {
        if let Some(language) = language {
            details.push(language.to_ascii_uppercase());
        }
    }
    if track.channels > 0 {
        details.push(format!("{}ch", track.channels));
    }
    if track.sample_rate > 0 {
        details.push(format!("{}kHz", track.sample_rate / 1000));
    }

    if details.is_empty() {
        base
    } else {
        format!("{base} ({})", details.join(", "))
    }
}

fn subtitle_track_selector<VM: 'static>(
    controller: VideoController,
    style: Option<StyleResolver<VideoStyle>>,
    visual_identity: VisualStyle,
) -> Option<Element<VM>> {
    let tracks = controller.subtitle_tracks().get();
    if tracks.is_empty() {
        return None;
    }

    let mut options = Vec::with_capacity(tracks.len() + 1);
    options.push(SelectOption::new(
        VideoSubtitleTrackSelection::Disabled,
        "Subs: Off".to_string(),
    ));
    options.extend(tracks.iter().enumerate().map(|(index, track)| {
        SelectOption::new(
            VideoSubtitleTrackSelection::Stream(track.stream_index),
            subtitle_track_label(track, index),
        )
    }));

    let selected = controller
        .subtitle_track_selection()
        .map(|selection| Some(selection));
    let selection_controller = controller.clone();
    let disabled = controller.playback_state().map(|state| {
        matches!(
            state,
            VideoPlaybackState::Loading | VideoPlaybackState::Error(_)
        )
    });

    let layout_style = style.clone();
    Some(
        Stack::new()
            .runtime_layout(move |layout, _container, context, style_sheet, visual| {
                let resolved = resolve_video_style_with_sheet(
                    layout_style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    WidgetState::default(),
                );
                layout.width = Some(Value::Static(crate::ui::layout::Length::Px(
                    resolved.subtitle_track_width,
                )));
                layout.height = Some(Value::Static(crate::ui::layout::Length::Px(
                    resolved.control_button_size,
                )));
            })
            .child(
                Select::new(options, selected)
                    .size(pct(100.0), pct(100.0))
                    .disable(disabled)
                    .style_full(video_track_select_style(style, visual_identity))
                    .on_change(ValueCommand::new(move |_, (selection, _label)| {
                        selection_controller.set_subtitle_track_selection(selection);
                    })),
            )
            .into(),
    )
}

fn subtitle_track_label(track: &VideoSubtitleTrack, index: usize) -> String {
    let title = track
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty());
    let language = track
        .language
        .as_deref()
        .map(str::trim)
        .filter(|language| !language.is_empty());
    let codec = track
        .codec
        .as_deref()
        .map(str::trim)
        .filter(|codec| !codec.is_empty());

    let base = title
        .map(ToString::to_string)
        .or_else(|| language.map(|language| language.to_ascii_uppercase()))
        .unwrap_or_else(|| format!("Track {}", index + 1));

    let mut details = Vec::new();
    if title.is_some() {
        if let Some(language) = language {
            details.push(language.to_ascii_uppercase());
        }
    }
    if let Some(codec) = codec {
        details.push(codec.to_string());
    }

    if details.is_empty() {
        format!("Subs: {base}")
    } else {
        format!("Subs: {base} ({})", details.join(", "))
    }
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
    let text = status_text(controller, style.clone(), visual_identity);
    let runtime_style = style.clone();
    Stack::new()
        .position_absolute()
        .left(dp(12.0))
        .top(dp(12.0))
        .padding(crate::ui::layout::Insets::ZERO)
        .runtime_layout(move |layout, _container, context, style_sheet, visual| {
            let resolved = resolve_video_style_with_sheet(
                runtime_style.as_ref(),
                context,
                style_sheet,
                visual,
                WidgetState::default(),
            );
            layout.padding = Some(Value::Static(resolved.status_padding));
        })
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
    let initial_disabled_opacity =
        VideoStyle::default_for_theme(&crate::ui::theme::Theme::default()).control_disabled_opacity;
    let runtime_style = style.clone();
    let runtime_disabled = disabled.clone();
    Stack::new()
        .runtime_layout(move |layout, _container, context, style_sheet, visual| {
            let resolved = resolve_video_style_with_sheet(
                runtime_style.as_ref(),
                context,
                style_sheet,
                visual,
                WidgetState::default(),
            );
            layout.width = Some(Value::Static(crate::ui::layout::Length::Px(
                resolved.control_button_size,
            )));
            layout.height = Some(Value::Static(crate::ui::layout::Length::Px(
                resolved.control_button_size,
            )));
            visual.opacity =
                disabled_opacity(runtime_disabled.clone(), resolved.control_disabled_opacity);
        })
        .center()
        .style_full_with_style_sheet(video_icon_button_style(style))
        .opacity(disabled_opacity(disabled.clone(), initial_disabled_opacity))
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
    with_visual_identity(
        Icon::internal(icon)
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

fn video_subtitle_text_style(
    style: Option<StyleResolver<VideoStyle>>,
    subtitle_style: VideoSubtitleCueStyle,
    layer: SubtitleTextLayer,
) -> impl Fn(&StyleContext<'_>, &StyleSheet, &VisualStyle, WidgetState) -> TextWidgetStyle
       + Send
       + Sync
       + 'static {
    move |context, style_sheet, visual, state| {
        let resolved =
            resolve_video_style_with_sheet(style.as_ref(), context, style_sheet, visual, state);
        let mut text = TextWidgetStyle::default_for_theme(context.theme);
        text.color = match layer {
            SubtitleTextLayer::Foreground => subtitle_style
                .primary_color
                .map(Value::Static)
                .unwrap_or(resolved.subtitle_text_color),
            SubtitleTextLayer::Outline(color) | SubtitleTextLayer::Shadow(color) => {
                Value::Static(color)
            }
        };
        text.typography = resolved.subtitle_text_style;
        if let Some(font_weight) = subtitle_style.font_weight {
            text.typography.weight = font_weight;
        }
        if let Some(font_size) = subtitle_style.font_size_centi_px {
            let next_size = font_size as f32 / 100.0;
            let current_size = text.typography.size.get().max(1.0);
            let scale = next_size / current_size;
            text.typography.size = sp(next_size);
            if let Some(line_height) = text.typography.line_height {
                text.typography.line_height = Some(sp(line_height.get() * scale));
            }
        }
        text
    }
}

fn video_subtitle_surface_style(
    style: Option<StyleResolver<VideoStyle>>,
) -> impl Fn(&StyleContext<'_>, &StyleSheet, &VisualStyle, WidgetState) -> ContainerStyle
       + Send
       + Sync
       + 'static {
    move |context, style_sheet, visual, state| {
        let resolved =
            resolve_video_style_with_sheet(style.as_ref(), context, style_sheet, visual, state);
        let mut container = ContainerStyle::default_for_theme(context.theme);
        container.surface.background = Some(resolved.subtitle_background);
        container.surface.border_radius = Some(Value::Static(context.theme.radius.md));
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

fn video_track_select_style(
    style: Option<StyleResolver<VideoStyle>>,
    _visual_identity: VisualStyle,
) -> impl Fn(&StyleContext<'_>) -> SelectStyle + Send + Sync + 'static {
    move |context| {
        let resolved = resolve_video_style(style.as_ref(), context);
        let mut select = SelectStyle::default_for_theme(context.theme);
        select.background = StateValue::new(Value::Static(Color::hexa(0xFFFFFF1F)));
        select.text = StateValue::new(resolved.time_text_color.clone());
        select.placeholder = StateValue::new(resolved.time_text_color.clone());
        select.arrow = StateValue::new(resolved.control_icon_color.clone());
        select.border = StateValue::new(Value::Static(Color::hexa(0xFFFFFF33)));
        select.menu_background = resolved.overlay_background.clone();
        select.option_background = StateValue::new(Value::Static(Color::TRANSPARENT));
        select.selected_option_background = Value::Static(Color::hexa(0xFFFFFF2E));
        select.border_width = Value::Static(dp(0.0));
        select.radius = Value::Static(context.theme.radius.full);
        select.padding_x = context.theme.spacing.sm;
        select.padding_y = Dp::ZERO;
        select.min_height = resolved.control_button_size;
        select.option_height = resolved.control_button_size;
        select.menu_gap = context.theme.spacing.xxs;
        select.text_style = resolved.time_text_style.clone();
        select
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
