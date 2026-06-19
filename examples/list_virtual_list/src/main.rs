use tgui::prelude::*;

#[derive(Clone)]
struct Contact {
    name: &'static str,
    role: &'static str,
    status: &'static str,
}

impl Contact {
    fn new(name: &'static str, role: &'static str, status: &'static str) -> Self {
        Self { name, role, status }
    }
}

struct AppVm {
    selected_keys: State<Vec<WidgetKey>>,
    loading: State<bool>,
    show_empty: State<bool>,
    status: State<String>,
    virtual_rows: Vec<String>,
}

impl AppVm {
    fn contacts() -> Vec<ListSection<Contact, Self>> {
        vec![
            ListSection::new(
                section_header("Product"),
                vec![
                    ListItem::keyed(
                        "ana",
                        Contact::new("Ana Torres", "Product lead", "Planning Q3 roadmap"),
                    ),
                    ListItem::keyed(
                        "mika",
                        Contact::new("Mika Chen", "Designer", "Reviewing interaction states"),
                    ),
                    ListItem::keyed(
                        "nora",
                        Contact::new("Nora Patel", "Research", "Disabled sample row"),
                    )
                    .disable(true),
                ],
            ),
            ListSection::new(
                section_header("Engineering"),
                vec![
                    ListItem::keyed(
                        "owen",
                        Contact::new("Owen Blake", "Runtime", "Keyboard navigation"),
                    ),
                    ListItem::keyed(
                        "li",
                        Contact::new("Li Wei", "Rendering", "Virtualized list rows"),
                    ),
                    ListItem::keyed(
                        "sam",
                        Contact::new("Sam Rivera", "Platform", "Context menus"),
                    ),
                ],
            ),
        ]
    }

    fn selection_summary(&self) -> Signal<String> {
        self.selected_keys.signal().map(|keys| {
            if keys.is_empty() {
                "No rows selected".to_string()
            } else {
                format!("{} selected: {:?}", keys.len(), keys)
            }
        })
    }

    fn status(&self) -> Signal<String> {
        self.status.signal()
    }

    fn set_selection(&mut self, change: ListSelectionChange) {
        let count = change.selected_keys.len();
        self.selected_keys.set(change.selected_keys);
        self.status.set(format!(
            "Selection changed by {:?}; focused={:?}; selected={count}",
            change.trigger, change.focused_key
        ));
    }

    fn open_contact(&mut self, action: ListItemAction) {
        self.status.set(format!(
            "Primary action fired for row {} ({:?})",
            action.index, action.key
        ));
    }

    fn context_action(&mut self) {
        self.status
            .set("Context menu command selected for the current row".to_string());
    }

    fn clear_selection(&mut self) {
        self.selected_keys.set(Vec::new());
        self.status.set("Selection cleared".to_string());
    }

    fn toggle_loading(&mut self) {
        self.loading.update(|loading| *loading = !*loading);
        self.status.set("Loading slot toggled".to_string());
    }

    fn toggle_empty(&mut self) {
        self.show_empty.update(|empty| *empty = !*empty);
        self.status.set("Empty slot toggled".to_string());
    }
}

impl ViewModel for AppVm {
    fn new(ctx: &ViewModelContext) -> Self {
        let virtual_rows = (0..10_000)
            .map(|index| format!("Log row #{index:04} - virtualized data item"))
            .collect();
        Self {
            selected_keys: ctx.state(vec![WidgetKey::from("ana")]),
            loading: ctx.state(false),
            show_empty: ctx.state(false),
            status: ctx
                .state("Click rows, Shift-click ranges, press Enter, or right-click.".into()),
            virtual_rows,
        }
    }

    fn view(&self) -> Element<Self> {
        Stack::new()
            .size(pct(100.0), pct(100.0))
            .style_full(root_style)
            .padding(Insets::all(dp(24.0)))
            .child(
                Flex::horizontal()
                    .gap(dp(18.0))
                    .child(self.list_panel())
                    .child(self.virtual_panel()),
            )
            .into()
    }
}

impl AppVm {
    fn list_panel(&self) -> Element<Self> {
        Flex::vertical()
            .width(dp(460.0))
            .height(pct(100.0))
            .gap(dp(12.0))
            .padding(Insets::all(dp(16.0)))
            .style_full(panel_style)
            .child(Text::new("List").style_full(title_style))
            .child(Text::new(self.selection_summary()).style_full(muted_text_style))
            .child(
                Flex::horizontal()
                    .gap(dp(8.0))
                    .child(
                        Button::new("Toggle loading").on_click(Command::new(Self::toggle_loading)),
                    )
                    .child(Button::new("Show empty").on_click(Command::new(Self::toggle_empty)))
                    .child(Button::new("Clear").on_click(Command::new(Self::clear_selection))),
            )
            .child(
                List::sections(
                    if self.show_empty.get() {
                        Vec::new()
                    } else {
                        Self::contacts()
                    },
                    contact_row,
                )
                .width(pct(100.0))
                .height(dp(360.0))
                .item_layout(ItemLayout::Measured {
                    estimate: dp(64.0),
                    spacing: dp(4.0),
                    overscan: 3,
                })
                .style_full(contact_list_style)
                .selection_mode(ListSelectionMode::Multiple)
                .selected_keys(self.selected_keys.signal())
                .loading(self.loading.signal())
                .loading_view(state_view("Loading contact rows..."))
                .empty(state_view("No contacts"))
                .context_menu(vec![
                    MenuItem::new("Mark as reviewed").on_select(Command::new(Self::context_action)),
                    MenuItem::new("Open profile").on_select(Command::new(Self::context_action)),
                ])
                .on_selection_change(ValueCommand::new(Self::set_selection))
                .on_item_action(ValueCommand::new(Self::open_contact)),
            )
            .child(Text::new(self.status()).style_full(status_text_style))
            .into()
    }

    fn virtual_panel(&self) -> Element<Self> {
        Flex::vertical()
            .grow(1.0)
            .height(pct(100.0))
            .gap(dp(12.0))
            .padding(Insets::all(dp(16.0)))
            .style_full(panel_style)
            .child(Text::new("VirtualList").style_full(title_style))
            .child(
                Text::new("10,000 rows, fixed 32dp item layout, overscan 4")
                    .style_full(muted_text_style),
            )
            .child(
                VirtualList::new_with_context(self.virtual_rows.clone(), virtual_row)
                    .item_layout(ItemLayout::Fixed {
                        item_extent: dp(32.0),
                        spacing: dp(2.0),
                        overscan: 4,
                    })
                    .width(pct(100.0))
                    .height(dp(430.0))
                    .style_full(virtual_surface_style),
            )
            .into()
    }
}

fn section_header(text: &'static str) -> Element<AppVm> {
    Stack::new()
        .height(dp(30.0))
        .padding(Insets::symmetric(dp(12.0), dp(6.0)))
        .child(Text::new(text).style_full(section_text_style))
        .into()
}

fn contact_row(ctx: ListItemContext<Contact>) -> Element<AppVm> {
    let accent = if ctx.selected {
        Color::hexa(0xA7F3D0FF)
    } else if ctx.disabled {
        Color::hexa(0x94A3B8FF)
    } else {
        Color::hexa(0xF8FAFCFF)
    };
    Flex::vertical()
        .gap(dp(2.0))
        .child(Text::new(ctx.item.name).style_full(move |ctx| row_title_style(ctx, accent)))
        .child(
            Text::new(format!("{} - {}", ctx.item.role, ctx.item.status))
                .style_full(muted_text_style),
        )
        .into()
}

fn virtual_row(ctx: ListItemContext<String>) -> Element<AppVm> {
    let color = if ctx.index % 2 == 0 {
        Color::hexa(0xE2E8F0FF)
    } else {
        Color::hexa(0xCBD5E1FF)
    };
    Stack::new()
        .padding(Insets::symmetric(dp(12.0), dp(6.0)))
        .child(Text::new(ctx.item).style_full(move |ctx| row_title_style(ctx, color)))
        .into()
}

fn state_view(text: &'static str) -> Element<AppVm> {
    Stack::new()
        .height(dp(160.0))
        .center()
        .style_full(empty_state_style)
        .child(Text::new(text).style_full(muted_text_style))
        .into()
}

fn panel_style(ctx: &StyleContext<'_>) -> ContainerStyle {
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(Color::hexa(0x182235FF).into());
    style.surface.border_color = Some(Color::hexa(0x334155FF).into());
    style.surface.border_width = Some(dp(1.0).into());
    style.surface.border_radius = Some(dp(8.0).into());
    style
}

fn root_style(ctx: &StyleContext<'_>) -> ContainerStyle {
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(Color::hexa(0x101828FF).into());
    style
}

fn virtual_surface_style(ctx: &StyleContext<'_>) -> ContainerStyle {
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(Color::hexa(0x0F172AFF).into());
    style.surface.border_color = Some(Color::hexa(0x263244FF).into());
    style.surface.border_width = Some(dp(1.0).into());
    style.surface.border_radius = Some(dp(8.0).into());
    style
}

fn empty_state_style(ctx: &StyleContext<'_>) -> ContainerStyle {
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(Color::hexa(0x0F172AFF).into());
    style.surface.border_color = Some(Color::hexa(0x334155FF).into());
    style.surface.border_width = Some(dp(1.0).into());
    style.surface.border_radius = Some(dp(8.0).into());
    style
}

fn contact_list_style(ctx: &StyleContext<'_>) -> ListStyle {
    let mut style = ListStyle::default_for_theme(ctx.theme);
    style.item_height = dp(64.0);
    style.item_padding = Insets::symmetric(dp(12.0), dp(8.0));
    style.item_radius = dp(8.0);
    style.item_hover_background = Color::hexa(0xFFFFFF14).into();
    style.item_selected_background = Color::hexa(0x2563EB55).into();
    style.item_disabled_background = Color::hexa(0x0F172A99).into();
    style.group_header_background = Color::TRANSPARENT.into();
    style.group_header_text = Color::hexa(0x93C5FDFF).into();
    style
}

fn title_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for_theme(ctx.theme);
    style.typography.size = sp(24.0);
    style.typography.weight = FontWeight::SemiBold;
    style.color = Color::hexa(0xF8FAFCFF).into();
    style
}

fn section_text_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for_theme(ctx.theme);
    style.typography.size = sp(13.0);
    style.typography.weight = FontWeight::SemiBold;
    style.color = Color::hexa(0x93C5FDFF).into();
    style
}

fn row_title_style(ctx: &StyleContext<'_>, color: Color) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for_theme(ctx.theme);
    style.typography.size = sp(15.0);
    style.typography.weight = FontWeight::Medium;
    style.color = color.into();
    style
}

fn muted_text_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for_theme(ctx.theme);
    style.typography.size = sp(13.0);
    style.color = Color::hexa(0xCBD5E1FF).into();
    style
}

fn status_text_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = muted_text_style(ctx);
    style.color = Color::hexa(0xA7F3D0FF).into();
    style
}

fn main() -> Result<(), TguiError> {
    Application::new()
        .title("tgui List / VirtualList")
        .window_size(dp(1040.0), dp(640.0))
        .clear_color(Color::hexa(0x101828FF))
        .with_view_model(AppVm::new)
        .root_view(AppVm::view)
        .run()
}
