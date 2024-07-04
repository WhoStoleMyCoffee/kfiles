use std::{io, path::{Path, PathBuf}, sync::OnceLock};

use configs::Configs;
use iced::{ Application, Settings};

pub mod app;
pub mod search;
pub mod tagging;
pub mod thumbnail;
pub mod widget;
pub mod strmatch;
pub mod configs;

use app::KFiles;
use tagging::{entries::Entries, Tag};

const APP_NAME: &str = std::env!("CARGO_PKG_NAME");
const VERSION: &str = std::env!("CARGO_PKG_VERSION");

static TEMP_DIR: OnceLock<PathBuf> = OnceLock::new();


fn main() -> iced::Result {
    // Load configs
    let configs = load_configs();

    // Initialize tags
    if should_reinit_tags() {
        println!("Initializing default tags...");
        init_default_tags();
    }

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


fn load_configs() -> Configs {
    configs::load_configs()
        .unwrap_or_else(|err| {
            match err {
                configs::LoadError::IO(err)
                    if err.kind() == io::ErrorKind::NotFound =>
                    println!("Initializing default configs..."),
                err => println!("Failed to load Configs: {:?}", err),
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
            eprintln!("[should_reinit_tags()] Failed to read save dir: {err:?}");
            true
        }
    }
}


fn init_default_tags() {
    let Some(user_dirs) = directories::UserDirs::new() else {
        eprintln!("[init_default_tags()] Failed to get user dirs.");
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
                eprintln!("Failed to save default tag \"{}\": {:?}", &tag.id, err);
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
        println!("Failed to save default tag \"{}\": {:?}", home.id, err);
    }
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
