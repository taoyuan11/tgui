use tgui::{
    dp, el, pct, sp, Axis, Button, Color, Command, Element, Flex, Insets, Observable, Text,
    ViewModelContext,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum HomeTab {
    Counter,
    Details,
}

#[derive(Clone)]
pub struct HomePage {
    tab: Observable<HomeTab>,
    counter: HomeCounterPage,
    details: HomeDetailsPage,
}

impl HomePage {
    pub fn new(context: &ViewModelContext) -> Self {
        Self {
            tab: context.observable(HomeTab::Counter),
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
        let tab = self.tab.binding();
        let counter = self.counter.clone();
        let details = self.details.clone();

        Flex::new(Axis::Vertical)
            .size(pct(60.0), pct(60.0))
            .padding(Insets::all(dp(20.0)))
            .background(Color::hex(0x0066FF))
            .border(dp(1.0), Color::WHITE)
            .child(el![
                Text::new("Home 内部也有页面切换")
                    .font_size(sp(22.0))
                    .color(Color::WHITE),
                Flex::new(Axis::Horizontal).gap(dp(10.0)).child(el![
                    Button::new(Text::new("Counter")).on_click(Command::new(Self::show_counter)),
                    Button::new(Text::new("Details")).on_click(Command::new(Self::show_details)),
                ]),
            ])
            .child(tab.map(move |tab| match tab {
                HomeTab::Counter => counter
                    .view()
                    .scope(|home: &mut HomePage| &mut home.counter),
                HomeTab::Details => details
                    .view()
                    .scope(|home: &mut HomePage| &mut home.details),
            }))
            .into()
    }
}

#[derive(Clone)]
struct HomeCounterPage {
    count: Observable<i32>,
}

impl HomeCounterPage {
    fn new(context: &ViewModelContext) -> Self {
        Self {
            count: context.observable(0),
        }
    }

    fn increment(&mut self) {
        self.count.update(|i| *i += 1);
    }

    fn view(&self) -> Element<Self> {
        Flex::new(Axis::Vertical)
            .padding(Insets::all(dp(14.0)))
            .background(Color::hex(0x0D47A1))
            .border(dp(1.0), Color::WHITE)
            .child(el![
                Text::new(self.count.binding().map(|i| format!("Home Counter 数量：{i}")))
                    .color(Color::WHITE),
                Button::new(Text::new("Counter +1")).on_click(Command::new(Self::increment)),
            ])
            .into()
    }
}

#[derive(Clone)]
struct HomeDetailsPage {
    visits: Observable<i32>,
}

impl HomeDetailsPage {
    fn new(context: &ViewModelContext) -> Self {
        Self {
            visits: context.observable(0),
        }
    }

    fn add_visit(&mut self) {
        self.visits.update(|visits| *visits += 1);
    }

    fn view(&self) -> Element<Self> {
        Flex::new(Axis::Vertical)
            .padding(Insets::all(dp(14.0)))
            .background(Color::hex(0x1565C0))
            .border(dp(1.0), Color::WHITE)
            .child(el![
                Text::new("Home Details 子页面").color(Color::WHITE),
                Text::new(self.visits.binding().map(|visits| format!("访问次数：{visits}")))
                    .color(Color::WHITE),
                Button::new(Text::new("记录访问")).on_click(Command::new(Self::add_visit)),
            ])
            .into()
    }
}
