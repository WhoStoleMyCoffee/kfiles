use iced::{Application, Settings};

pub mod app;
pub mod tag;
pub mod search;

use app::TagExplorer;

fn main() -> iced::Result {
    TagExplorer::run(Settings {
        window: iced::window::Settings {
            size: iced::Size::new(600.0, 400.0),
            ..Default::default()
        },
        ..Default::default()
    })
}
