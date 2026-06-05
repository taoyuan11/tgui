#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod demo_section;
mod navigation;
mod pages;
mod styles;

use app::App;
use tgui::prelude::*;

fn main() -> Result<(), TguiError> {
    App::run()
}
