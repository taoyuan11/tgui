use tgui::prelude::*;

fn text_style(ctx: &StyleContext<'_>, size: Sp, color: Color) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for_theme(ctx.theme);
    style.typography.size = size;
    style.color = color.into();
    style
}

fn panel_style(ctx: &StyleContext<'_>, background: Color) -> ContainerStyle {
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(background.into());
    style.surface.border_color = Some(Color::WHITE.into());
    style.surface.border_width = Some(dp(1.0).into());
    style
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HomeTab {
    Counter,
    Details,
}

#[derive(Clone)]
pub struct HomePage {
    tab: State<HomeTab>,
    counter: HomeCounterPage,
    details: HomeDetailsPage,
}

impl HomePage {
    pub fn new(context: &ViewModelContext) -> Self {
        Self {
            tab: context.state(HomeTab::Counter),
            counter: HomeCounterPage::new(context),
            details: HomeDetailsPage::new(context),
        }
    }

    fn show_counter(&mut self) {
        self.tab.set(HomeTab::Counter);
    }

    fn show_details(&mut self) {
        self.tab.set(HomeTab::Details);
    }

    pub fn view(&self) -> Element<Self> {
        let counter = self.counter.clone();
        let details = self.details.clone();
        let content = match self.tab.get() {
            HomeTab::Counter => counter
                .view()
                .scope(|home: &mut HomePage| &mut home.counter),
            HomeTab::Details => details
                .view()
                .scope(|home: &mut HomePage| &mut home.details),
        };

        Flex::new(Axis::Vertical)
            .size(pct(60.0), pct(60.0))
            .padding(Insets::all(dp(20.0)))
            .style_full(|ctx| panel_style(ctx, Color::hex(0x0066FF)))
            .child(el![
                Text::new("Home 内部也有页面切换")
                    .style_full(|ctx| text_style(ctx, sp(22.0), Color::WHITE)),
                Flex::new(Axis::Horizontal).gap(dp(10.0)).child(el![
                    Button::new("Counter").on_click(Command::new(Self::show_counter)),
                    Button::new("Details").on_click(Command::new(Self::show_details)),
                ]),
            ])
            .child(content)
            .into()
    }
}

#[derive(Clone)]
struct HomeCounterPage {
    count: State<i32>,
}

impl HomeCounterPage {
    fn new(context: &ViewModelContext) -> Self {
        Self {
            count: context.state(0),
        }
    }

    fn increment(&mut self) {
        self.count.update(|i| *i += 1);
    }

    fn view(&self) -> Element<Self> {
        Flex::new(Axis::Vertical)
            .padding(Insets::all(dp(14.0)))
            .style_full(|ctx| panel_style(ctx, Color::hex(0x0D47A1)))
            .child(el![
                Text::new(self.count.signal().map(|i| format!("Home Counter 数量：{i}")))
                    .style_full(|ctx| text_style(ctx, sp(16.0), Color::WHITE)),
                Button::new("Counter +1").on_click(Command::new(Self::increment)),
            ])
            .into()
    }
}

#[derive(Clone)]
struct HomeDetailsPage {
    visits: State<i32>,
}

impl HomeDetailsPage {
    fn new(context: &ViewModelContext) -> Self {
        Self {
            visits: context.state(0),
        }
    }

    fn add_visit(&mut self) {
        self.visits.update(|visits| *visits += 1);
    }

    fn view(&self) -> Element<Self> {
        Flex::new(Axis::Vertical)
            .padding(Insets::all(dp(14.0)))
            .style_full(|ctx| panel_style(ctx, Color::hex(0x1565C0)))
            .child(el![
                Text::new("Home Details 子页面")
                    .style_full(|ctx| text_style(ctx, sp(16.0), Color::WHITE)),
                Text::new(self.visits.signal().map(|visits| format!("访问次数：{visits}")))
                    .style_full(|ctx| text_style(ctx, sp(16.0), Color::WHITE)),
                Button::new("记录访问").on_click(Command::new(Self::add_visit)),
            ])
            .into()
    }
}
