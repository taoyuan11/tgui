use std::fmt;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::ui::theme::{ControlSize, StyleContext, WidgetState};
use crate::ui::widget::common::{ButtonVariantKind, VisualStyle};

use super::{
    AvatarStyle, BadgeStyle, BreadcrumbStyle, ButtonStyle, CanvasStyle, CardStyle, CarouselStyle,
    CheckboxStyle, CollapseStyle, ComboboxStyle, ContainerStyle, DividerStyle, DrawerStyle,
    IconStyle, ImageStyle, InputStyle, MenuBarStyle, MenuStyle, ModalStyle, PaginationStyle,
    PopoverStyle, ProgressBarStyle, RadioStyle, RatingStyle, RichTextStyle, SelectStyle,
    SkeletonStyle, SliderStyle, SpinnerStyle, SplitterStyle, SwitchStyle, TabsStyle,
    TextWidgetStyle, TextareaStyle, ToastStyle, TooltipStyle, VideoStyle, VideoSurfaceStyle,
};
use crate::ui::widget::{DataGridStyle, ListStyle, TreeStyle};

static NEXT_STYLE_RULE_ID: AtomicU64 = AtomicU64::new(1);

type StyleMutator<T> = Arc<dyn Fn(&mut T, &StyleContext<'_>) + Send + Sync>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StyleSelector {
    class: Option<String>,
    style_id: Option<String>,
    state: Option<WidgetState>,
}

impl StyleSelector {
    pub fn any() -> Self {
        Self::default()
    }

    pub fn class(class: impl Into<String>) -> Self {
        Self::any().with_class(class)
    }

    pub fn style_id(style_id: impl Into<String>) -> Self {
        Self::any().with_style_id(style_id)
    }

    pub fn state(state: WidgetState) -> Self {
        Self::any().with_state(state)
    }

    pub fn with_class(mut self, class: impl Into<String>) -> Self {
        self.class = Some(class.into());
        self
    }

    pub fn with_style_id(mut self, style_id: impl Into<String>) -> Self {
        self.style_id = Some(style_id.into());
        self
    }

    pub fn with_state(mut self, state: WidgetState) -> Self {
        self.state = Some(state);
        self
    }

    fn matches_identity(&self, visual: &VisualStyle) -> bool {
        if let Some(style_id) = self.style_id.as_deref() {
            if visual.style_id.as_deref() != Some(style_id) {
                return false;
            }
        }
        if let Some(class) = self.class.as_deref() {
            if !visual.classes.iter().any(|candidate| candidate == class) {
                return false;
            }
        }
        true
    }

    fn has_state(&self) -> bool {
        self.state.is_some()
    }

    fn matches_static(&self, visual: &VisualStyle) -> bool {
        !self.has_state() && self.matches_identity(visual)
    }

    fn matches_state(&self, visual: &VisualStyle, state: WidgetState) -> bool {
        let Some(expected) = self.state else {
            return false;
        };
        self.matches_identity(visual) && widget_state_matches(expected, state)
    }
}

fn widget_state_matches(expected: WidgetState, actual: WidgetState) -> bool {
    if expected == WidgetState::default() {
        return actual == WidgetState::default();
    }
    (!expected.hovered || actual.hovered)
        && (!expected.pressed || actual.pressed)
        && (!expected.focused || actual.focused)
        && (!expected.focus_visible || actual.focus_visible)
        && (!expected.disabled || actual.disabled)
        && (!expected.selected || actual.selected)
        && (!expected.checked || actual.checked)
        && (!expected.open || actual.open)
        && (!expected.invalid || actual.invalid)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ButtonSelector {
    base: StyleSelector,
    variant: Option<ButtonVariantKind>,
    size: Option<ControlSize>,
}

impl ButtonSelector {
    pub fn any() -> Self {
        Self::default()
    }

    pub fn primary() -> Self {
        Self::variant(ButtonVariantKind::Primary)
    }

    pub fn secondary() -> Self {
        Self::variant(ButtonVariantKind::Secondary)
    }

    pub fn ghost() -> Self {
        Self::variant(ButtonVariantKind::Ghost)
    }

    pub fn danger() -> Self {
        Self::variant(ButtonVariantKind::Danger)
    }

    pub fn variant(variant: ButtonVariantKind) -> Self {
        Self {
            base: StyleSelector::any(),
            variant: Some(variant),
            size: None,
        }
    }

    pub fn class(mut self, class: impl Into<String>) -> Self {
        self.base = self.base.with_class(class);
        self
    }

    pub fn style_id(mut self, style_id: impl Into<String>) -> Self {
        self.base = self.base.with_style_id(style_id);
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = Some(size);
        self
    }

    pub fn state(mut self, state: WidgetState) -> Self {
        self.base = self.base.with_state(state);
        self
    }

    fn matches(&self, variant: ButtonVariantKind, visual: &VisualStyle) -> bool {
        if self.variant.is_some_and(|expected| expected != variant) {
            return false;
        }
        self.base.matches_static(visual)
    }

    fn matches_state(
        &self,
        variant: ButtonVariantKind,
        visual: &VisualStyle,
        state: WidgetState,
    ) -> bool {
        if self.variant.is_some_and(|expected| expected != variant) {
            return false;
        }
        self.base.matches_state(visual, state)
    }
}

#[derive(Clone)]
struct StyleRule<T, S> {
    id: u64,
    selector: S,
    mutator: StyleMutator<T>,
    _marker: PhantomData<fn() -> T>,
}

impl<T, S: fmt::Debug> fmt::Debug for StyleRule<T, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StyleRule")
            .field("id", &self.id)
            .field("selector", &self.selector)
            .finish()
    }
}

impl<T, S: PartialEq> PartialEq for StyleRule<T, S> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.selector == other.selector
    }
}

type Rule<T> = StyleRule<T, StyleSelector>;
type ButtonRule = StyleRule<ButtonStyle, ButtonSelector>;

#[derive(Clone, Default)]
pub struct StyleSheet {
    version: u64,
    text_rules: Vec<Rule<TextWidgetStyle>>,
    container_rules: Vec<Rule<ContainerStyle>>,
    image_rules: Vec<Rule<ImageStyle>>,
    canvas_rules: Vec<Rule<CanvasStyle>>,
    video_rules: Vec<Rule<VideoStyle>>,
    video_surface_rules: Vec<Rule<VideoSurfaceStyle>>,
    button_rules: Vec<ButtonRule>,
    checkbox_rules: Vec<Rule<CheckboxStyle>>,
    radio_rules: Vec<Rule<RadioStyle>>,
    switch_rules: Vec<Rule<SwitchStyle>>,
    select_rules: Vec<Rule<SelectStyle>>,
    input_rules: Vec<Rule<InputStyle>>,
    textarea_rules: Vec<Rule<TextareaStyle>>,
    slider_rules: Vec<Rule<SliderStyle>>,
    progress_bar_rules: Vec<Rule<ProgressBarStyle>>,
    spinner_rules: Vec<Rule<SpinnerStyle>>,
    divider_rules: Vec<Rule<DividerStyle>>,
    tabs_rules: Vec<Rule<TabsStyle>>,
    list_rules: Vec<Rule<ListStyle>>,
    tree_rules: Vec<Rule<TreeStyle>>,
    data_grid_rules: Vec<Rule<DataGridStyle>>,
    menu_rules: Vec<Rule<MenuStyle>>,
    menu_bar_rules: Vec<Rule<MenuBarStyle>>,
    tooltip_rules: Vec<Rule<TooltipStyle>>,
    popover_rules: Vec<Rule<PopoverStyle>>,
    modal_rules: Vec<Rule<ModalStyle>>,
    toast_rules: Vec<Rule<ToastStyle>>,
    drawer_rules: Vec<Rule<DrawerStyle>>,
    badge_rules: Vec<Rule<BadgeStyle>>,
    avatar_rules: Vec<Rule<AvatarStyle>>,
    skeleton_rules: Vec<Rule<SkeletonStyle>>,
    collapse_rules: Vec<Rule<CollapseStyle>>,
    splitter_rules: Vec<Rule<SplitterStyle>>,
    breadcrumb_rules: Vec<Rule<BreadcrumbStyle>>,
    pagination_rules: Vec<Rule<PaginationStyle>>,
    card_rules: Vec<Rule<CardStyle>>,
    rating_rules: Vec<Rule<RatingStyle>>,
    icon_rules: Vec<Rule<IconStyle>>,
    rich_text_rules: Vec<Rule<RichTextStyle>>,
    carousel_rules: Vec<Rule<CarouselStyle>>,
    combobox_rules: Vec<Rule<ComboboxStyle>>,
}

impl fmt::Debug for StyleSheet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StyleSheet")
            .field("version", &self.version)
            .field("text_rules", &self.text_rules.len())
            .field("container_rules", &self.container_rules.len())
            .field("button_rules", &self.button_rules.len())
            .field("input_rules", &self.input_rules.len())
            .field("select_rules", &self.select_rules.len())
            .field("tabs_rules", &self.tabs_rules.len())
            .field("list_rules", &self.list_rules.len())
            .field("tree_rules", &self.tree_rules.len())
            .field("data_grid_rules", &self.data_grid_rules.len())
            .field("menu_rules", &self.menu_rules.len())
            .field("modal_rules", &self.modal_rules.len())
            .field("toast_rules", &self.toast_rules.len())
            .field("badge_rules", &self.badge_rules.len())
            .field("avatar_rules", &self.avatar_rules.len())
            .field("skeleton_rules", &self.skeleton_rules.len())
            .field("collapse_rules", &self.collapse_rules.len())
            .field("splitter_rules", &self.splitter_rules.len())
            .field("breadcrumb_rules", &self.breadcrumb_rules.len())
            .field("pagination_rules", &self.pagination_rules.len())
            .field("card_rules", &self.card_rules.len())
            .field("rating_rules", &self.rating_rules.len())
            .field("icon_rules", &self.icon_rules.len())
            .field("rich_text_rules", &self.rich_text_rules.len())
            .field("carousel_rules", &self.carousel_rules.len())
            .field("video_rules", &self.video_rules.len())
            .field("combobox_rules", &self.combobox_rules.len())
            .finish()
    }
}

impl StyleSheet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn is_empty(&self) -> bool {
        self.text_rules.is_empty()
            && self.container_rules.is_empty()
            && self.image_rules.is_empty()
            && self.canvas_rules.is_empty()
            && self.video_rules.is_empty()
            && self.video_surface_rules.is_empty()
            && self.button_rules.is_empty()
            && self.checkbox_rules.is_empty()
            && self.radio_rules.is_empty()
            && self.switch_rules.is_empty()
            && self.select_rules.is_empty()
            && self.input_rules.is_empty()
            && self.textarea_rules.is_empty()
            && self.slider_rules.is_empty()
            && self.progress_bar_rules.is_empty()
            && self.spinner_rules.is_empty()
            && self.divider_rules.is_empty()
            && self.tabs_rules.is_empty()
            && self.list_rules.is_empty()
            && self.tree_rules.is_empty()
            && self.data_grid_rules.is_empty()
            && self.menu_rules.is_empty()
            && self.menu_bar_rules.is_empty()
            && self.tooltip_rules.is_empty()
            && self.popover_rules.is_empty()
            && self.modal_rules.is_empty()
            && self.toast_rules.is_empty()
            && self.drawer_rules.is_empty()
            && self.badge_rules.is_empty()
            && self.avatar_rules.is_empty()
            && self.skeleton_rules.is_empty()
            && self.collapse_rules.is_empty()
            && self.splitter_rules.is_empty()
            && self.breadcrumb_rules.is_empty()
            && self.pagination_rules.is_empty()
            && self.card_rules.is_empty()
            && self.rating_rules.is_empty()
            && self.icon_rules.is_empty()
            && self.rich_text_rules.is_empty()
            && self.carousel_rules.is_empty()
            && self.combobox_rules.is_empty()
    }

    pub fn button(
        mut self,
        selector: ButtonSelector,
        mutator: impl Fn(&mut ButtonStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.button_rules.push(StyleRule {
            id: NEXT_STYLE_RULE_ID.fetch_add(1, Ordering::Relaxed),
            selector,
            mutator: Arc::new(mutator),
            _marker: PhantomData,
        });
        self.bump_version();
        self
    }

    pub fn button_class(
        self,
        class: impl Into<String>,
        mutator: impl Fn(&mut ButtonStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.button(ButtonSelector::any().class(class), mutator)
    }

    pub fn button_id(
        self,
        style_id: impl Into<String>,
        mutator: impl Fn(&mut ButtonStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.button(ButtonSelector::any().style_id(style_id), mutator)
    }

    pub fn class(
        self,
        class: impl Into<String>,
        mutator: impl Fn(&mut ButtonStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.button_class(class, mutator)
    }

    pub(crate) fn apply_button(
        &self,
        style: &mut ButtonStyle,
        context: &StyleContext<'_>,
        variant: ButtonVariantKind,
        visual: &VisualStyle,
    ) {
        for rule in &self.button_rules {
            if rule.selector.matches(variant, visual) {
                (rule.mutator)(style, context);
            }
        }
    }

    pub(crate) fn apply_button_state(
        &self,
        style: &mut ButtonStyle,
        context: &StyleContext<'_>,
        variant: ButtonVariantKind,
        visual: &VisualStyle,
        state: WidgetState,
    ) {
        for rule in &self.button_rules {
            if rule.selector.matches_state(variant, visual, state) {
                (rule.mutator)(style, context);
            }
        }
    }

    fn bump_version(&mut self) {
        self.version = self.version.wrapping_add(1);
    }
}

macro_rules! style_sheet_methods {
    ($($method:ident, $class_method:ident, $id_method:ident, $apply_method:ident, $apply_state_method:ident, $rules:ident, $style:ty),* $(,)?) => {
        impl StyleSheet {
            $(
                pub fn $method(
                    mut self,
                    selector: StyleSelector,
                    mutator: impl Fn(&mut $style, &StyleContext<'_>) + Send + Sync + 'static,
                ) -> Self {
                    self.$rules.push(StyleRule {
                        id: NEXT_STYLE_RULE_ID.fetch_add(1, Ordering::Relaxed),
                        selector,
                        mutator: Arc::new(mutator),
                        _marker: PhantomData,
                    });
                    self.bump_version();
                    self
                }

                pub fn $class_method(
                    self,
                    class: impl Into<String>,
                    mutator: impl Fn(&mut $style, &StyleContext<'_>) + Send + Sync + 'static,
                ) -> Self {
                    self.$method(StyleSelector::class(class), mutator)
                }

                pub fn $id_method(
                    self,
                    style_id: impl Into<String>,
                    mutator: impl Fn(&mut $style, &StyleContext<'_>) + Send + Sync + 'static,
                ) -> Self {
                    self.$method(StyleSelector::style_id(style_id), mutator)
                }

                #[allow(dead_code)] // Generated hooks are used according to enabled widget features.
                pub(crate) fn $apply_method(
                    &self,
                    style: &mut $style,
                    context: &StyleContext<'_>,
                    visual: &VisualStyle,
                ) {
                    for rule in &self.$rules {
                        if rule.selector.matches_static(visual) {
                            (rule.mutator)(style, context);
                        }
                    }
                }

                #[allow(dead_code)] // Generated hooks are used according to enabled widget features.
                pub(crate) fn $apply_state_method(
                    &self,
                    style: &mut $style,
                    context: &StyleContext<'_>,
                    visual: &VisualStyle,
                    state: WidgetState,
                ) {
                    for rule in &self.$rules {
                        if rule.selector.matches_state(visual, state) {
                            (rule.mutator)(style, context);
                        }
                    }
                }
            )*
        }
    };
}

style_sheet_methods! {
    text, text_class, text_id, apply_text, apply_text_state, text_rules, TextWidgetStyle,
    container, container_class, container_id, apply_container, apply_container_state, container_rules, ContainerStyle,
    image, image_class, image_id, apply_image, apply_image_state, image_rules, ImageStyle,
    canvas, canvas_class, canvas_id, apply_canvas, apply_canvas_state, canvas_rules, CanvasStyle,
    video, video_class, video_id, apply_video, apply_video_state, video_rules, VideoStyle,
    video_surface, video_surface_class, video_surface_id, apply_video_surface, apply_video_surface_state, video_surface_rules, VideoSurfaceStyle,
    checkbox, checkbox_class, checkbox_id, apply_checkbox, apply_checkbox_state, checkbox_rules, CheckboxStyle,
    radio, radio_class, radio_id, apply_radio, apply_radio_state, radio_rules, RadioStyle,
    switch, switch_class, switch_id, apply_switch, apply_switch_state, switch_rules, SwitchStyle,
    select, select_class, select_id, apply_select, apply_select_state, select_rules, SelectStyle,
    input, input_class, input_id, apply_input, apply_input_state, input_rules, InputStyle,
    textarea, textarea_class, textarea_id, apply_textarea, apply_textarea_state, textarea_rules, TextareaStyle,
    slider, slider_class, slider_id, apply_slider, apply_slider_state, slider_rules, SliderStyle,
    progress_bar, progress_bar_class, progress_bar_id, apply_progress_bar, apply_progress_bar_state, progress_bar_rules, ProgressBarStyle,
    spinner, spinner_class, spinner_id, apply_spinner, apply_spinner_state, spinner_rules, SpinnerStyle,
    divider, divider_class, divider_id, apply_divider, apply_divider_state, divider_rules, DividerStyle,
    tabs, tabs_class, tabs_id, apply_tabs, apply_tabs_state, tabs_rules, TabsStyle,
    list, list_class, list_id, apply_list, apply_list_state, list_rules, ListStyle,
    tree, tree_class, tree_id, apply_tree, apply_tree_state, tree_rules, TreeStyle,
    data_grid, data_grid_class, data_grid_id, apply_data_grid, apply_data_grid_state, data_grid_rules, DataGridStyle,
    menu, menu_class, menu_id, apply_menu, apply_menu_state, menu_rules, MenuStyle,
    menu_bar, menu_bar_class, menu_bar_id, apply_menu_bar, apply_menu_bar_state, menu_bar_rules, MenuBarStyle,
    tooltip, tooltip_class, tooltip_id, apply_tooltip, apply_tooltip_state, tooltip_rules, TooltipStyle,
    popover, popover_class, popover_id, apply_popover, apply_popover_state, popover_rules, PopoverStyle,
    modal, modal_class, modal_id, apply_modal, apply_modal_state, modal_rules, ModalStyle,
    toast, toast_class, toast_id, apply_toast, apply_toast_state, toast_rules, ToastStyle,
    drawer, drawer_class, drawer_id, apply_drawer, apply_drawer_state, drawer_rules, DrawerStyle,
    badge, badge_class, badge_id, apply_badge, apply_badge_state, badge_rules, BadgeStyle,
    avatar, avatar_class, avatar_id, apply_avatar, apply_avatar_state, avatar_rules, AvatarStyle,
    skeleton, skeleton_class, skeleton_id, apply_skeleton, apply_skeleton_state, skeleton_rules, SkeletonStyle,
    collapse, collapse_class, collapse_id, apply_collapse, apply_collapse_state, collapse_rules, CollapseStyle,
    splitter, splitter_class, splitter_id, apply_splitter, apply_splitter_state, splitter_rules, SplitterStyle,
    breadcrumb, breadcrumb_class, breadcrumb_id, apply_breadcrumb, apply_breadcrumb_state, breadcrumb_rules, BreadcrumbStyle,
    pagination, pagination_class, pagination_id, apply_pagination, apply_pagination_state, pagination_rules, PaginationStyle,
    card, card_class, card_id, apply_card, apply_card_state, card_rules, CardStyle,
    rating, rating_class, rating_id, apply_rating, apply_rating_state, rating_rules, RatingStyle,
    icon, icon_class, icon_id, apply_icon, apply_icon_state, icon_rules, IconStyle,
    rich_text, rich_text_class, rich_text_id, apply_rich_text, apply_rich_text_state, rich_text_rules, RichTextStyle,
    carousel, carousel_class, carousel_id, apply_carousel, apply_carousel_state, carousel_rules, CarouselStyle,
    combobox, combobox_class, combobox_id, apply_combobox, apply_combobox_state, combobox_rules, ComboboxStyle,
}
