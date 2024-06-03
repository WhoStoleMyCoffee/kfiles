use std::{path::{Path, PathBuf}, sync::OnceLock};

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
        let pb: PathBuf = std::env::temp_dir().join(env!("CARGO_PKG_NAME"));
        if pb.exists() {
            return pb;
        }
        if let Err(err) = std::fs::create_dir_all(&pb) {
            eprintln!("Failed to create temp directory: {:?}", err);
        }
        pb
    })
}




trait ToPrettyString {
    fn to_pretty_string(&self) -> String;
}

impl ToPrettyString for Path {
    fn to_pretty_string(&self) -> String {
        self.display()
            .to_string()
            .replace('\\', "/")
    }
}

impl ToPrettyString for PathBuf {
    fn to_pretty_string(&self) -> String {
        self.as_path().to_pretty_string()
    }
}


