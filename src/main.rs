#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::io;

use configs::Configs;
use iced::{ Application, Settings};

pub mod app;
pub mod search;
pub mod tagging;
pub mod thumbnail;
pub mod widget;
pub mod strmatch;
pub mod configs;
pub mod log;

use app::KFiles;
use log::Log;
use tagging::{entries::Entries, Tag};


const APP_NAME: &str = std::env!("CARGO_PKG_NAME");
const VERSION: &str = std::env!("CARGO_PKG_VERSION");


fn main() -> iced::Result {
    Log::init();
    Log::remove_old_logs(7);

    // Load configs
    let configs: Configs = load_configs();

    // Initialize tags
    if should_reinit_tags() {
        info!("Initializing default tags...");
        init_default_tags();
    }

    configs::set_global(configs) .expect("global Configs instance shouldn't be set before this");

    // Run program...
    let res = KFiles::run(Settings {
        window: iced::window::Settings {
            size: iced::Size::new(800.0, 400.0),
            ..Default::default()
        },
        ..Default::default()
    });

    trace!("Program terminated with result {:?}", res);
    shutdown();

    res
}


pub fn shutdown() {
    // Trim thumbnail cache once we're done
    trace!("Trimming thumbnail cache...");
    match thumbnail::trim_cache( configs::global().thumbnail_cache_size ) {
        Ok(bytes) => info!("Sucessfully trimmed thumbnail cache by {} bytes", bytes),
        Err(err) if err.kind() == io::ErrorKind::NotFound => trace!("Thumbnail cache already clear"),
        Err(err) => error!("Failed to trim thumbnail cache:\n {err:?}"),
    }

    // Save configs
    if let Err(err) = configs::global().save() {
        error!("Failed to save Configs:\n {err:?}");
    }
}


pub fn get_temp_dir() -> PathBuf {
    std::env::temp_dir().join(env!("CARGO_PKG_NAME"))
}


fn load_configs() -> Configs {
    configs::load_configs().unwrap_or_else(|err| {
        match err {
            configs::LoadError::IO(err) if err.kind() == io::ErrorKind::NotFound => {
                info!("Initializing default configs.");
            }
            err => {
                error!("Failed to load configs file:\n {:?}", err);
            }
        }
        configs::Configs::default()
    })
}


fn should_reinit_tags() -> bool {
    let path = tagging::get_save_dir();
    match path.read_dir() {
        Ok(mut it) => it.next().is_none(),
        Err(err) if err.kind() == io::ErrorKind::NotFound => true,
        Err(err) => {
            error!("[should_reinit_tags()] Failed to read save dir:\n {err:?}");
            true
        }
    }
}


fn init_default_tags() {
    let Some(user_dirs) = directories::UserDirs::new() else {
        error!("[init_default_tags()] Failed to get user dirs.");
        return;
    };

    let tags = [
        ("documents", user_dirs.document_dir()),
        ("pictures", user_dirs.picture_dir()),
        ("videos", user_dirs.video_dir()),
        ("music", user_dirs.audio_dir()),
    ];
    let tags = tags.into_iter()
        .flat_map(|(id, p)|
            p.map(|p| (id, p.to_path_buf()) )
        )
        .map(|(id, p)| {
            Tag::create(id) .with_entries(Entries::from(vec![ p ]))
        })
        .filter(|tag| match tag.save() {
            Ok(()) => true,
            Err(err) => {
                error!("Failed to save default tag \"{}\":\n {:?}", &tag.id, err);
                false
            },
        })
        .collect::<Vec<Tag>>();

    let home_dirs = [
        user_dirs.desktop_dir(),
        user_dirs.download_dir(),
    ];
    let mut home = Tag::create("home")
        .with_entries(Entries::from_iter(
            home_dirs.into_iter()
                .flatten()
                .map(|p| p.to_path_buf())
        ));

    for t in tags.iter() {
        home.add_subtag(&t.id) .expect("failed to add subtag");
    }


    if let Err(err) = home.save() {
        error!("Failed to save default tag \"{}\":\n {:?}", home.id, err);
    }
}




/// Trait to convert to [`String`] but pretty way
/// Like [`Display`] but I make the rules
/// ### Why?
/// Because Windows uses backslashes `\\` for paths and that's ugly especially
/// when they mix with regular slashes `/`
trait ToPrettyString {
    fn to_pretty_string(&self) -> String;
}

impl<T> ToPrettyString for T
where
    T: AsRef<Path>
{
    fn to_pretty_string(&self) -> String {
        self.as_ref()
            .display()
            .to_string()
            .replace('\\', "/")
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




#[test]
fn test_log() {
    Log::init();

    trace!();
    trace!("Hello, World!");
    trace!("Hello, {}!", "World");
    warn!("Do not the cat.");
    info!("The cat is called \"{}\" btw", "Blasphemous rumours");
    error!("He the cat {}.", "😔");
}

