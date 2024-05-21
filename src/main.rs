use std::{path::PathBuf, sync::OnceLock};

use iced::{ Application, Settings};

pub mod app;
pub mod search;
pub mod tag;
pub mod thumbnail;
pub mod widget;
pub mod strmatch;

use app::TagExplorer;

static TEMP_DIR: OnceLock<PathBuf> = OnceLock::new();

fn main() -> iced::Result {
    let res = TagExplorer::run(Settings {
        window: iced::window::Settings {
            size: iced::Size::new(800.0, 400.0),
            ..Default::default()
        },
        ..Default::default()
    });

    // Trim thumbnail cache once we're done
    match thumbnail::trim_cache(thumbnail::MAX_CACHE_SIZE_BYTES) {
        Ok(bytes) if bytes > 0 => {
            println!("Sucessfully trimmed cache by {} bytes", bytes)
        }
        Err(err) => {
            println!("ERROR: failed to trim cache: {}", err);
        }
        _ => {}
    }

    res
}

pub fn get_temp_dir() -> &'static PathBuf {
    TEMP_DIR.get_or_init(|| {
        let pb = std::env::temp_dir().join(env!("CARGO_PKG_NAME"));
        if !pb.exists() {
            std::fs::create_dir_all(&pb).unwrap(); // handle this unwrap at some point lol
        }
        pb
    })
}


