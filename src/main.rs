use iced::{Application, Settings};

pub mod app;
pub mod tag;
pub mod search;

use app::TagExplorer;

fn main() -> iced::Result {

    // println!("{}", env!("CARGO_MANIFEST_DIR"));

    TagExplorer::run(Settings {
        window: iced::window::Settings {
            size: iced::Size::new(800.0, 400.0),
            ..Default::default()
        },
        ..Default::default()
    })
}
