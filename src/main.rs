use iced::{
    Application,
    Settings,
};


pub mod app;

use app::TagExplorer;



const UPDATE_RATE_MS: u64 = 250;
const FOCUS_QUERY_KEYS: [&str; 3] = [
    "s",
    "/",
    ";",
];


fn main() -> iced::Result {
    TagExplorer::run(Settings {
        window: iced::window::Settings {
            size: iced::Size::new(600.0, 400.0),
            ..Default::default()
        },
        ..Default::default()
    })
}



