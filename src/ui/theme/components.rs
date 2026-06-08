use std::fmt;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use super::StyleContext;
use crate::ui::widget::{
    AvatarStyle, BadgeStyle, BreadcrumbStyle, ButtonStyle, CanvasStyle, CardStyle, CarouselStyle,
    CheckboxStyle, CollapseStyle, ComboboxStyle, ContainerStyle, DataGridStyle, DividerStyle,
    DrawerStyle, IconStyle, ImageStyle, InputStyle, ListStyle, MenuBarStyle, MenuStyle, ModalStyle,
    PaginationStyle, PopoverStyle, ProgressBarStyle, RadioStyle, RatingStyle, RichTextStyle,
    SelectStyle, SkeletonStyle, SliderStyle, SpinnerStyle, SplitterStyle, SwitchStyle, TabsStyle,
    TextWidgetStyle, TextareaStyle, ToastStyle, TooltipStyle, TreeStyle, VideoSurfaceStyle,
};

static NEXT_COMPONENT_RULE_ID: AtomicU64 = AtomicU64::new(1);

type ComponentMutator<T> = Arc<dyn Fn(&mut T, &StyleContext<'_>) + Send + Sync>;

struct ComponentRule<T> {
    id: u64,
    mutator: ComponentMutator<T>,
}

impl<T> Clone for ComponentRule<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            mutator: self.mutator.clone(),
        }
    }
}

pub struct ComponentStyle<T> {
    rules: Arc<[ComponentRule<T>]>,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Clone for ComponentStyle<T> {
    fn clone(&self) -> Self {
        Self {
            rules: self.rules.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T> Default for ComponentStyle<T> {
    fn default() -> Self {
        Self {
            rules: Arc::from([]),
            _marker: PhantomData,
        }
    }
}

impl<T> fmt::Debug for ComponentStyle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ComponentStyle")
            .field("rules", &self.rules.len())
            .finish()
    }
}

impl<T> PartialEq for ComponentStyle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.rules
            .iter()
            .map(|rule| rule.id)
            .eq(other.rules.iter().map(|rule| rule.id))
    }
}

impl<T> ComponentStyle<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn patch(mutator: impl Fn(&mut T, &StyleContext<'_>) + Send + Sync + 'static) -> Self {
        Self::new().push(mutator)
    }

    pub fn push(
        mut self,
        mutator: impl Fn(&mut T, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        let mut rules = self.rules.iter().cloned().collect::<Vec<_>>();
        rules.push(ComponentRule {
            id: NEXT_COMPONENT_RULE_ID.fetch_add(1, Ordering::Relaxed),
            mutator: Arc::new(mutator),
        });
        self.rules = Arc::from(rules);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub(crate) fn apply(&self, style: &mut T, context: &StyleContext<'_>) {
        for rule in self.rules.iter() {
            (rule.mutator)(style, context);
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComponentThemes {
    pub text: ComponentStyle<TextWidgetStyle>,
    pub container: ComponentStyle<ContainerStyle>,
    pub image: ComponentStyle<ImageStyle>,
    pub canvas: ComponentStyle<CanvasStyle>,
    pub video_surface: ComponentStyle<VideoSurfaceStyle>,
    pub button: ComponentStyle<ButtonStyle>,
    pub checkbox: ComponentStyle<CheckboxStyle>,
    pub radio: ComponentStyle<RadioStyle>,
    pub switch: ComponentStyle<SwitchStyle>,
    pub select: ComponentStyle<SelectStyle>,
    pub input: ComponentStyle<InputStyle>,
    pub textarea: ComponentStyle<TextareaStyle>,
    pub slider: ComponentStyle<SliderStyle>,
    pub progress_bar: ComponentStyle<ProgressBarStyle>,
    pub spinner: ComponentStyle<SpinnerStyle>,
    pub divider: ComponentStyle<DividerStyle>,
    pub tabs: ComponentStyle<TabsStyle>,
    pub list: ComponentStyle<ListStyle>,
    pub tree: ComponentStyle<TreeStyle>,
    pub data_grid: ComponentStyle<DataGridStyle>,
    pub menu: ComponentStyle<MenuStyle>,
    pub menu_bar: ComponentStyle<MenuBarStyle>,
    pub tooltip: ComponentStyle<TooltipStyle>,
    pub popover: ComponentStyle<PopoverStyle>,
    pub modal: ComponentStyle<ModalStyle>,
    pub toast: ComponentStyle<ToastStyle>,
    pub drawer: ComponentStyle<DrawerStyle>,
    pub badge: ComponentStyle<BadgeStyle>,
    pub avatar: ComponentStyle<AvatarStyle>,
    pub skeleton: ComponentStyle<SkeletonStyle>,
    pub collapse: ComponentStyle<CollapseStyle>,
    pub splitter: ComponentStyle<SplitterStyle>,
    pub breadcrumb: ComponentStyle<BreadcrumbStyle>,
    pub pagination: ComponentStyle<PaginationStyle>,
    pub card: ComponentStyle<CardStyle>,
    pub rating: ComponentStyle<RatingStyle>,
    pub icon: ComponentStyle<IconStyle>,
    pub rich_text: ComponentStyle<RichTextStyle>,
    pub carousel: ComponentStyle<CarouselStyle>,
    pub combobox: ComponentStyle<ComboboxStyle>,
}

macro_rules! component_theme_methods {
    ($($method:ident : $field:ident : $style:ty),* $(,)?) => {
        impl ComponentThemes {
            $(
                pub fn $method(
                    mut self,
                    mutator: impl Fn(&mut $style, &StyleContext<'_>) + Send + Sync + 'static,
                ) -> Self {
                    self.$field = self.$field.push(mutator);
                    self
                }
            )*
        }
    };
}

component_theme_methods! {
    text: text: TextWidgetStyle,
    container: container: ContainerStyle,
    image: image: ImageStyle,
    canvas: canvas: CanvasStyle,
    video_surface: video_surface: VideoSurfaceStyle,
    button: button: ButtonStyle,
    checkbox: checkbox: CheckboxStyle,
    radio: radio: RadioStyle,
    switch: switch: SwitchStyle,
    select: select: SelectStyle,
    input: input: InputStyle,
    textarea: textarea: TextareaStyle,
    slider: slider: SliderStyle,
    progress_bar: progress_bar: ProgressBarStyle,
    spinner: spinner: SpinnerStyle,
    divider: divider: DividerStyle,
    tabs: tabs: TabsStyle,
    list: list: ListStyle,
    tree: tree: TreeStyle,
    data_grid: data_grid: DataGridStyle,
    menu: menu: MenuStyle,
    menu_bar: menu_bar: MenuBarStyle,
    tooltip: tooltip: TooltipStyle,
    popover: popover: PopoverStyle,
    modal: modal: ModalStyle,
    toast: toast: ToastStyle,
    drawer: drawer: DrawerStyle,
    badge: badge: BadgeStyle,
    avatar: avatar: AvatarStyle,
    skeleton: skeleton: SkeletonStyle,
    collapse: collapse: CollapseStyle,
    splitter: splitter: SplitterStyle,
    breadcrumb: breadcrumb: BreadcrumbStyle,
    pagination: pagination: PaginationStyle,
    card: card: CardStyle,
    rating: rating: RatingStyle,
    icon: icon: IconStyle,
    rich_text: rich_text: RichTextStyle,
    carousel: carousel: CarouselStyle,
    combobox: combobox: ComboboxStyle,
}
