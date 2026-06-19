use tgui::prelude::*;

#[derive(Clone)]
struct FileInfo {
    name: &'static str,
    kind: &'static str,
}

#[derive(Clone, PartialEq)]
struct FileNode {
    key: &'static str,
    name: &'static str,
    kind: &'static str,
    disabled: bool,
    children: Vec<FileNode>,
}

impl FileNode {
    fn folder(key: &'static str, name: &'static str, children: Vec<FileNode>) -> Self {
        Self {
            key,
            name,
            kind: "folder",
            disabled: false,
            children,
        }
    }

    fn file(key: &'static str, name: &'static str) -> Self {
        Self {
            key,
            name,
            kind: "file",
            disabled: false,
            children: Vec::new(),
        }
    }

    fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }
}

struct AppVm {
    nodes: State<Vec<FileNode>>,
    expanded: State<Vec<WidgetKey>>,
    selected: State<Vec<WidgetKey>>,
    checked: State<Vec<WidgetKey>>,
    loading: State<bool>,
    show_empty: State<bool>,
    status: State<String>,
}

impl ViewModel for AppVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            nodes: ctx.state(sample_nodes()),
            expanded: ctx.state(vec![WidgetKey::from("src"), WidgetKey::from("widgets")]),
            selected: ctx.state(vec![WidgetKey::from("tree")]),
            checked: ctx.state(vec![WidgetKey::from("tree")]),
            loading: ctx.state(false),
            show_empty: ctx.state(false),
            status: ctx.state("Tree ready".to_string()),
        }
    }

    fn view(&self) -> Element<Self> {
        Flex::vertical()
            .size(pct(100.0), pct(100.0))
            .padding(Insets::all(dp(24.0)))
            .gap(dp(14.0))
            .style_full(root_style)
            .child(Text::new("Tree").style_full(title_style))
            .child(Text::new(self.summary()).style_full(muted_style))
            .child(
                Flex::horizontal()
                    .gap(dp(8.0))
                    .child(Button::new("Reset").on_click(Command::new(Self::reset)))
                    .child(
                        Button::new("Toggle loading").on_click(Command::new(Self::toggle_loading)),
                    )
                    .child(Button::new("Show empty").on_click(Command::new(Self::toggle_empty)))
                    .child(Button::new("Clear checks").on_click(Command::new(Self::clear_checks))),
            )
            .child(
                Tree::<FileInfo, Self>::new(self.tree_nodes(), file_row)
                    .width(pct(100.0))
                    .height(dp(420.0))
                    .expanded_keys(self.expanded.signal())
                    .selected_keys(self.selected.signal())
                    .selection_mode(TreeSelectionMode::Multiple)
                    .checkable(true)
                    .checked_keys(self.checked.signal())
                    .loading(self.loading.signal())
                    .loading_view(state_view("Loading tree rows"))
                    .empty(state_view("No nodes"))
                    .draggable(true)
                    .context_menu(vec![
                        MenuItem::new("Select checked")
                            .on_select(Command::new(Self::select_checked)),
                        MenuItem::new("Clear selection")
                            .on_select(Command::new(Self::clear_selection)),
                    ])
                    .style_full(tree_style)
                    .on_expand_change(ValueCommand::new(Self::set_expanded))
                    .on_selection_change(ValueCommand::new(Self::set_selected))
                    .on_check_change(ValueCommand::new(Self::set_checked))
                    .on_node_action(ValueCommand::new(Self::open_node))
                    .on_drop(ValueCommand::new(Self::drop_node)),
            )
            .child(Text::new(self.status.signal()).style_full(status_style))
            .into()
    }
}

impl AppVm {
    fn tree_nodes(&self) -> Vec<TreeNode<FileInfo>> {
        if self.show_empty.get() {
            Vec::new()
        } else {
            self.nodes
                .get()
                .into_iter()
                .map(tree_node_from_file)
                .collect()
        }
    }

    fn summary(&self) -> String {
        format!(
            "{} selected, {} checked, {} expanded",
            self.selected.get().len(),
            self.checked.get().len(),
            self.expanded.get().len()
        )
    }

    fn set_expanded(&mut self, change: TreeExpandChange) {
        self.expanded.set(change.expanded_keys);
        self.status
            .set(format!("Expand {:?}: {}", change.key, change.expanded));
    }

    fn set_selected(&mut self, change: TreeSelectionChange) {
        let count = change.selected_keys.len();
        self.selected.set(change.selected_keys);
        self.status.set(format!(
            "Selection via {:?}; focused={:?}; selected={count}",
            change.trigger, change.focused_key
        ));
    }

    fn set_checked(&mut self, change: TreeCheckChange) {
        let count = change.checked_keys.len();
        self.checked.set(change.checked_keys);
        self.status.set(format!(
            "Check {:?}: {:?}; affected={}, checked={count}",
            change.key,
            change.check_state,
            change.affected_keys.len()
        ));
    }

    fn open_node(&mut self, action: TreeNodeAction) {
        self.status.set(format!(
            "Primary action for {:?} at index {}",
            action.key, action.index
        ));
    }

    fn drop_node(&mut self, event: TreeDropEvent) {
        let mut nodes = self.nodes.get();
        let Some(dragged) = remove_node(&mut nodes, &event.dragged_key) else {
            self.status
                .set(format!("Drop skipped; missing {:?}", event.dragged_key));
            return;
        };
        if insert_node(&mut nodes, &event.target_key, event.position, dragged) {
            if event.position == TreeDropPosition::Inside {
                self.expanded.update(|keys| {
                    if !keys.iter().any(|key| key == &event.target_key) {
                        keys.push(event.target_key.clone());
                    }
                });
            }
            self.nodes.set(nodes);
            self.status.set(format!(
                "Dropped {:?} {:?} {:?}",
                event.dragged_key, event.position, event.target_key
            ));
        } else {
            self.status.set(format!(
                "Drop skipped; missing target {:?}",
                event.target_key
            ));
        }
    }

    fn reset(&mut self) {
        self.nodes.set(sample_nodes());
        self.expanded
            .set(vec![WidgetKey::from("src"), WidgetKey::from("widgets")]);
        self.selected.set(vec![WidgetKey::from("tree")]);
        self.checked.set(vec![WidgetKey::from("tree")]);
        self.loading.set(false);
        self.show_empty.set(false);
        self.status.set("Tree reset".to_string());
    }

    fn toggle_loading(&mut self) {
        self.loading.update(|loading| *loading = !*loading);
        self.status.set("Loading slot toggled".to_string());
    }

    fn toggle_empty(&mut self) {
        self.show_empty.update(|empty| *empty = !*empty);
        self.status.set("Empty slot toggled".to_string());
    }

    fn clear_checks(&mut self) {
        self.checked.set(Vec::new());
        self.status.set("Checks cleared".to_string());
    }

    fn select_checked(&mut self) {
        self.selected.set(self.checked.get());
        self.status.set("Selected checked nodes".to_string());
    }

    fn clear_selection(&mut self) {
        self.selected.set(Vec::new());
        self.status.set("Selection cleared".to_string());
    }
}

fn sample_nodes() -> Vec<FileNode> {
    vec![
        FileNode::folder(
            "src",
            "src",
            vec![
                FileNode::folder(
                    "widgets",
                    "ui/widget",
                    vec![
                        FileNode::file("tree", "tree/mod.rs"),
                        FileNode::file("list", "list/mod.rs"),
                        FileNode::file("table", "table/mod.rs"),
                    ],
                ),
                FileNode::folder(
                    "runtime",
                    "runtime/input",
                    vec![
                        FileNode::file("tree-runtime", "tree.rs"),
                        FileNode::file("keyboard", "key_repeat.rs"),
                    ],
                ),
            ],
        ),
        FileNode::folder(
            "docs",
            "docs",
            vec![
                FileNode::file("readme", "README.md"),
                FileNode::file("roadmap", "COMPONENTS_ROADMAP.md").disabled(),
            ],
        ),
    ]
}

fn tree_node_from_file(node: FileNode) -> TreeNode<FileInfo> {
    let children = node.children.into_iter().map(tree_node_from_file);
    TreeNode::keyed(
        node.key,
        FileInfo {
            name: node.name,
            kind: node.kind,
        },
    )
    .disable(node.disabled)
    .children(children)
}

fn remove_node(nodes: &mut Vec<FileNode>, key: &WidgetKey) -> Option<FileNode> {
    if let Some(index) = nodes
        .iter()
        .position(|node| WidgetKey::from(node.key) == *key)
    {
        return Some(nodes.remove(index));
    }
    for node in nodes {
        if let Some(found) = remove_node(&mut node.children, key) {
            return Some(found);
        }
    }
    None
}

fn insert_node(
    nodes: &mut Vec<FileNode>,
    target_key: &WidgetKey,
    position: TreeDropPosition,
    dragged: FileNode,
) -> bool {
    if position == TreeDropPosition::Inside {
        for node in nodes {
            if WidgetKey::from(node.key) == *target_key {
                node.children.push(dragged);
                return true;
            }
            if insert_node(&mut node.children, target_key, position, dragged.clone()) {
                return true;
            }
        }
        return false;
    }

    if let Some(index) = nodes
        .iter()
        .position(|node| WidgetKey::from(node.key) == *target_key)
    {
        let insert_at = match position {
            TreeDropPosition::Before => index,
            TreeDropPosition::After => index + 1,
            TreeDropPosition::Inside => unreachable!(),
        };
        nodes.insert(insert_at, dragged);
        return true;
    }

    for node in nodes {
        if insert_node(&mut node.children, target_key, position, dragged.clone()) {
            return true;
        }
    }
    false
}

fn file_row(ctx: TreeNodeContext<FileInfo>) -> Element<AppVm> {
    let selected = ctx.selected;
    let disabled = ctx.disabled;
    let label = if ctx.disabled {
        format!("{} ({}, disabled)", ctx.item.name, ctx.item.kind)
    } else {
        format!("{} ({})", ctx.item.name, ctx.item.kind)
    };
    Text::new(label)
        .style_full(move |ctx| row_text_style(ctx, selected, disabled))
        .into()
}

fn state_view(text: &'static str) -> Element<AppVm> {
    Stack::new()
        .height(dp(160.0))
        .center()
        .style_full(empty_state_style)
        .child(Text::new(text).style_full(muted_style))
        .into()
}

fn root_style(ctx: &StyleContext<'_>) -> ContainerStyle {
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(match ctx.mode {
        ResolvedThemeMode::Light => Color::hexa(0xF7F8FBFF).into(),
        ResolvedThemeMode::Dark => Color::hexa(0x101418FF).into(),
    });
    style
}

fn tree_style(ctx: &StyleContext<'_>) -> TreeStyle {
    let mut style = TreeStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(match ctx.mode {
        ResolvedThemeMode::Light => Color::hexa(0xFFFFFFFF).into(),
        ResolvedThemeMode::Dark => Color::hexa(0x161B22FF).into(),
    });
    style.surface.border_color = Some(match ctx.mode {
        ResolvedThemeMode::Light => Color::hexa(0xD7DEE8FF).into(),
        ResolvedThemeMode::Dark => Color::hexa(0x303846FF).into(),
    });
    style.surface.border_width = Some(dp(1.0).into());
    style.surface.border_radius = Some(dp(8.0).into());
    style.item_selected_background = match ctx.mode {
        ResolvedThemeMode::Light => Color::hexa(0xD8EAFEFF).into(),
        ResolvedThemeMode::Dark => Color::hexa(0x28537AFF).into(),
    };
    style.item_hover_background = match ctx.mode {
        ResolvedThemeMode::Light => Color::hexa(0xEEF3F8FF).into(),
        ResolvedThemeMode::Dark => Color::hexa(0x222B36FF).into(),
    };
    style
}

fn title_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for_theme(ctx.theme);
    style.typography.size = sp(26.0);
    style.typography.weight = FontWeight::SemiBold;
    style
}

fn muted_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for_theme(ctx.theme);
    style.typography.size = sp(14.0);
    style.color = match ctx.mode {
        ResolvedThemeMode::Light => Color::hexa(0x4B5563FF).into(),
        ResolvedThemeMode::Dark => Color::hexa(0xC8D0DAFF).into(),
    };
    style
}

fn status_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = muted_style(ctx);
    style.typography.weight = FontWeight::Medium;
    style
}

fn row_text_style(ctx: &StyleContext<'_>, selected: bool, disabled: bool) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for_theme(ctx.theme);
    style.typography.size = sp(14.0);
    style.typography.weight = if selected {
        FontWeight::SemiBold
    } else {
        FontWeight::Regular
    };
    style.color = if disabled {
        match ctx.mode {
            ResolvedThemeMode::Light => Color::hexa(0x9AA4B2FF).into(),
            ResolvedThemeMode::Dark => Color::hexa(0x697586FF).into(),
        }
    } else {
        match ctx.mode {
            ResolvedThemeMode::Light => Color::hexa(0x111827FF).into(),
            ResolvedThemeMode::Dark => Color::hexa(0xF6F8FBFF).into(),
        }
    };
    style
}

fn empty_state_style(ctx: &StyleContext<'_>) -> ContainerStyle {
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(match ctx.mode {
        ResolvedThemeMode::Light => Color::hexa(0xF1F5F9FF).into(),
        ResolvedThemeMode::Dark => Color::hexa(0x1F2732FF).into(),
    });
    style.surface.border_radius = Some(dp(8.0).into());
    style
}

fn main() -> Result<(), TguiError> {
    Application::new()
        .title("tgui Tree")
        .window_size(dp(760.0), dp(620.0))
        .with_view_model(AppVm::new)
        .root_view(AppVm::view)
        .run()
}
