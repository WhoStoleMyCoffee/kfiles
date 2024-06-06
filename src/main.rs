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






mod fs {
    use std::{cmp::Ordering, path::PathBuf, thread, time::{Duration, Instant, SystemTime}};

    pub struct Timeout;

    pub trait Watcher {
        type Error;

        /// idk how works lmao
        fn wait(&self, timeout: Duration) -> Result<(), Self::Error>;
    }


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


    /// TODO proofread this
    pub struct ModificationWatcher {
        pub path: PathBuf,
        pub check_interval: Duration,
        pub since: SystemTime,
    }

    impl Watcher for ModificationWatcher {
        type Error = ();

        /// TODO proofread this
        fn wait(&self, timeout: Duration) -> Result<(), Self::Error> {
            let start = Instant::now();

            loop {
                std::thread::sleep(self.check_interval);

                let Some(modified) = self.path.metadata().ok()
                    .and_then(|m| m.modified().ok())
                else {
                    return Err(());
                };

                if modified >= self.since {
                    return Ok(());
                }
                if start.elapsed() >= timeout {
                    return Err(());
                }

            }
        }
    }



    // TODO delete this at some point
    #[test]
    fn test_systime() {
        let a = SystemTime::now();
        thread::sleep(Duration::from_millis(2000));
        let b = SystemTime::now();

        assert_eq!(
            a.cmp(&b),
            Ordering::Less
        );

        assert_eq!(
            b.cmp(&a),
            Ordering::Greater
        );
    }

}
