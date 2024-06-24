use std::{io, path::{Path, PathBuf}, sync::OnceLock};

use iced::{ Application, Settings};

pub mod app;
pub mod search;
pub mod tagging;
pub mod thumbnail;
pub mod widget;
pub mod strmatch;
pub mod configs;

use app::KFiles;

const APP_NAME: &str = std::env!("CARGO_PKG_NAME");

static TEMP_DIR: OnceLock<PathBuf> = OnceLock::new();


fn main() -> iced::Result {
    // Load configs
    let configs = configs::load_configs()
        .unwrap_or_else(|err| {
            match err {
                configs::LoadError::IO(err)
                    if err.kind() == io::ErrorKind::NotFound =>
                    println!("Configs file not found, using default Configs instead"),
                err => println!("Failed to load Configs: {:?}", err),
            }
            configs::Configs::default()
        });

    configs::set_global(configs) .expect("failed to initialize configs");

    // Run program...
    let res = KFiles::run(Settings {
        window: iced::window::Settings {
            size: iced::Size::new(800.0, 400.0),
            ..Default::default()
        },
        ..Default::default()
    });

    // Trim thumbnail cache once we're done
    println!("Trimming thumbnail cache...");
    match thumbnail::trim_cache( configs::global().thumbnail_cache_size ) {
        Ok(0) => println!("Thumbnail cache already clear"),
        Ok(bytes) => println!("Sucessfully trimmed thumbnail cache by {} bytes", bytes),
        Err(err) => match err.kind() {
            io::ErrorKind::NotFound => println!("Thumbnail cache already clear"),
            _ => println!("ERROR: failed to trim thumbnail cache:\n{}", err),
        }
    }

    // Save configs
    if let Err(err) = configs::global().save() {
        println!("ERROR: Failed to save Configs:\n {err:?}");
    }

    res
}

pub fn get_temp_dir() -> &'static PathBuf {
    let path = TEMP_DIR.get_or_init(||
        std::env::temp_dir().join(env!("CARGO_PKG_NAME"))
    );

    if !path.exists() {
        if let Err(err) = std::fs::create_dir_all(path) {
            eprintln!("Failed to create temp directory: {:?}", err);
        }
    }

    path
}




trait ToPrettyString {
    fn to_pretty_string(&self) -> String;
}

impl ToPrettyString for &Path {
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






mod fs {
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    #[derive(Debug)]
    pub struct Timeout;

    pub trait Watcher {
        type Error;

        /// idk how works lmao
        fn wait(&self, timeout: Duration) -> Result<(), Self::Error>;
    }


    #[derive(Debug)]
    pub struct DeletionWatcher {
        pub path: PathBuf,
        pub check_interval: Duration,
    }

    impl Watcher for DeletionWatcher {
        type Error = Timeout;

        fn wait(&self, timeout: Duration) -> Result<(), Self::Error> {
            let start = Instant::now();

            loop {
                if !self.path.exists() {
                    return Ok(());
                }
                if start.elapsed() >= timeout {
                    return Err(Timeout);
                }

                std::thread::sleep(self.check_interval);
            }
        }
    }

    #[derive(Debug)]
    pub struct CreationWatcher {
        pub path: PathBuf,
        pub check_interval: Duration,
    }

    impl Watcher for CreationWatcher {
        type Error = Timeout;

        fn wait(&self, timeout: Duration) -> Result<(), Self::Error> {
            let start = Instant::now();

            loop {
                if self.path.exists() {
                    return Ok(());
                }
                if start.elapsed() >= timeout {
                    return Err(Timeout);
                }

                std::thread::sleep(self.check_interval);
            }
        }
    }

    // There is no ModificationWatcher;
    // Creation and deletion may not be instant, but modification apparently is
}
