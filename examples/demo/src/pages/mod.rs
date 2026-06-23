pub(crate) mod basics;
pub(crate) mod data;
pub(crate) mod feedback;
pub(crate) mod forms;
pub(crate) mod icons;
pub(crate) mod media;
pub(crate) mod overlays;

use crate::app::App;
use crate::navigation::DemoPage;
use tgui::prelude::*;

pub(crate) fn render(app: &App, page: DemoPage) -> Element<App> {
    match page {
        DemoPage::Basics => basics::page(app),
        DemoPage::Icons => icons::page(app),
        DemoPage::Forms => forms::page(app),
        DemoPage::Feedback => feedback::page(app),
        DemoPage::Overlays => overlays::page(app),
        DemoPage::Data => data::page(app),
        DemoPage::MediaCanvas => media::page(app),
    }
}
