use std::fmt::{Arguments, Display};
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::{APP_NAME, VERSION};

static GLOBAL: OnceLock<Mutex<Log>> = OnceLock::new();

/// TODO documentation
#[macro_export]
macro_rules! trace {
    () => {
        $crate::log::Log::global().log(
            $crate::log::Level::Trace,
            format_args!("[{}:{}:{}]", std::file!(), std::line!(), std::column!())
        )
    };

    (notify, log_context = $ctx:expr; $($arg:tt)*) => {
        $crate::log!(
            notify, log_context = $ctx; 
            $crate::log::Level::Trace;
            $($arg)*
        )
    };

    (notify; $($arg:tt)*) => {
        $crate::log!(
            notify;
            $crate::log::Level::Trace;
            $($args)*
        )
    };

    ($($arg:tt)*) => {
        $crate::log!($crate::log::Level::Trace; $($arg)*)
    };
}

/// Logs at the `Level::Info` level
/// TODO documentation
#[macro_export]
macro_rules! info {
    (notify, log_context = $ctx:expr; $($arg:tt)*) => {
        $crate::log!(
            notify, log_context = $ctx; 
            $crate::log::Level::Info;
            $($arg)*
        )
    };

    (notify; $($arg:tt)*) => {
        $crate::log!(
            notify;
            $crate::log::Level::Info;
            $($arg)*
        )
    };

    ($($arg:tt)*) => {
        $crate::log!($crate::log::Level::Info; $($arg)*)
    };
}

/// Logs at the `Level::Warning` level
/// Valid forms:
/// - `warn!()`
/// - `warn!( format args... )`
#[macro_export]
macro_rules! warn {
    (notify, log_context = $ctx:expr; $($arg:tt)*) => {
        $crate::log!(
            notify, log_context = $ctx; 
            $crate::log::Level::Warning;
            $($arg)*
        )
    };

    (notify; $($arg:tt)*) => {
        $crate::log!(
            notify;
            $crate::log::Level::Warning;
            $($arg)*
        )
    };

    ($($arg:tt)*) => {
        $crate::log!($crate::log::Level::Warning; $($arg)*)
    };
}

/// Logs at the `Level::Error` level
/// Valid forms:
/// - `error!()`
/// - `error!( format args... )`
#[macro_export]
macro_rules! error {
    (notify, log_context = $ctx:expr; $($arg:tt)*) => {
        $crate::log!(
            notify, log_context = $ctx; 
            $crate::log::Level::Error;
            $($arg)*
        )
    };

    (notify; $($arg:tt)*) => {
        $crate::log!(
            notify;
            $crate::log::Level::Error;
            $($arg)*
        )
    };

    ($($arg:tt)*) => {
        $crate::log!($crate::log::Level::Error; $($arg)*)
    };
}

/// TODO documentation all of these yknow
#[macro_export]
macro_rules! log {
    (notify, log_context = $ctx:expr; $level:expr; $($arg:tt)*) => {{
        $crate::log::Log::global().log(
            $level,
            format_args!("[{}] {}", $ctx, format_args!( $($arg)* ) )
        );

        $crate::log::notification::Notification::new(
            $level,
            format!( $($arg)* )
        )
    }};

    (notify; $level:expr; $($arg:tt)*) => {{
        $crate::log::Log::global().log(
            $level,
            format_args!( $($arg)* )
        );

        $crate::log::notification::Notification::new(
            $level,
            format!( $($arg)* )
        )
    }};

    ($level:expr; $($arg:tt)*) => {
        $crate::log::Log::global().log(
            $level,
            format_args!( $($arg)* )
        )
    };
}




/// A log level
#[derive(Debug, Clone)]
pub enum Level {
    Trace,
    Info,
    Warning,
    Error,
}









impl Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Level::Trace => write!(f, ""),
            Level::Info => write!(f, "[INFO]"),
            Level::Warning => write!(f, "[WARNING]"),
            Level::Error => write!(f, "[ERROR]"),
        }
    }
}



/// Struct that handles logging
pub struct Log {
    start: Instant,
    sink: Box<dyn Write + Send>,
}

impl Log {
    /// Create a new [`Log`]
    /// Logs to a log file if in `release` profile, or `stderr` if `dev`
    fn new() -> Log {
        let sink = match Log::get_sink() {
            Ok(s) => s,
            Err(err) => {
                eprintln!("Failed to get logs sink: {err:?}. Returning stderr instead.");
                Box::new(std::io::stderr())
            },
        };

        Log {
            start: Instant::now(),
            sink,
        }
    }

    /// Initializes logging
    pub fn init() {
        let mut log = Log::new();

        // Startup logs
        if cfg!(debug_assertions) {
            // Debug
            let _ = writeln!(&mut log, "{} debug version {}\nInitialized logging", APP_NAME, VERSION);
        } else {
            // Release
            let _ = writeln!(&mut log, "\n{} release version {}\nInitialized logging", APP_NAME, VERSION);
        }

        if GLOBAL.set(Mutex::new(log)).is_err() {
            // If `OnceLock.set()` fails, that means there's already a global `Log` instance
            error!("Global Log instance already initialized");
        }
    }

    /// Get the global [`Log`] instance
    pub fn global() -> MutexGuard<'static, Log> {
        let mutex = GLOBAL.get() .expect("global Logs not initialized");
        mutex.lock()
            .unwrap_or_else(|mut err| {
                error!("Error while getting global Logs instance:\n Mutex was poisonned");
                **err.get_mut() = Log::new();
                mutex.clear_poison();
                err.into_inner()
            })
    }

    pub fn log(&mut self, level: Level, args: Arguments) {
        let now = self.start.elapsed();
        let seconds = now.as_secs();
        let hours = seconds / 3600;
        let minutes = (seconds / 60) % 60;
        let seconds = seconds % 60;
        let miliseconds = now.subsec_millis();
        // let thread_id = thread::current().id();

        let _ = writeln!(
            self.sink,
            "[{:02}:{:02}:{:02}.{:03}] {} {}",
            hours,
            minutes,
            seconds,
            miliseconds,
            level,
            args,
        );
    }

    /// Removes log files that are older than `max_days` since creation
    /// 
    /// # Panics
    ///
    /// Panics if global Log instance is not initialized. See [`Log::init`]
    pub fn remove_old_logs(max_days: u64) {
        trace!("Removing old logs...");

        let Some(path) = get_logs_dir() else {
            error!("[remove_old_logs] Failed to get logs dir");
            return;
        };

        let it = match fs::read_dir(path) {
            Ok(it) => it,
            Err(err) => {
                error!("[remove_old_logs] Failed to read logs dir:\n {err:?}");
                return;
            }
        };

        let max_dur = Duration::from_secs(60 * 24 * max_days);
        
        let mut count: usize = 0;
        for de in it.flatten() {
            let Some(created) = de.metadata().ok().and_then(|m| m.created().ok()) else {
                continue;
            };

            // Removal check:
            if !created.elapsed().map_or(true, |dur| dur > max_dur) {
                continue;
            }

            let path = de.path();
            if let Err(err) = fs::remove_file(&path) {
                error!("Failed to remove log file \"{}\":\n {:?}", path.display(), err);
            }
            count += 1;
        }

        trace!("Successfully removed {count} log files");
    }

    /// Gets the sink to be used for the current profile:
    /// - A log file if `release`
    /// - `stderr` if `dev`
    #[cfg(debug_assertions)]
    fn get_sink() -> io::Result<Box<dyn Write + Send>> {
        Ok( Box::new(std::io::stderr()) )
    }

    /// Gets the sink to be used for the current profile:
    /// - A log file if `release`
    /// - `stderr` if `dev`
    #[cfg(not(debug_assertions))]
    fn get_sink() -> io::Result<Box<dyn Write + Send>> {
        use std::fs::{self, OpenOptions};

        let path = get_log_path()
            .ok_or( io::Error::new(io::ErrorKind::Other, "failed to get log path") )?;
        if !path.exists() {
            let dir = path.parent() .expect("log path should not be root or empty");
            fs::create_dir_all(dir)?;
        }

        Ok(Box::new(
            OpenOptions::new()
                .write(true)
                .append(true)
                .create(true)
                .open(path)?
        ))
    }

}


impl Write for Log {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.sink.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.sink.flush()
    }
}



/// Gets the path for today's log file
pub fn get_log_path() -> Option<PathBuf> {
    let date = chrono::Local::now() .date_naive();
    get_logs_dir()
        .map(|p| p.join( format!("log_{}.log", date) ))
}

/// Gets the directory where logs are saved
/// Returns `None` if no [`directories::BaseDirs`] could be created
pub fn get_logs_dir() -> Option<PathBuf> {
    Some(
        directories::BaseDirs::new()?
            .data_dir()
            .to_path_buf()
            .join(format!("{}/logs/", APP_NAME))
    )
}



pub mod notification {
    use std::time::{Duration, Instant};
    use iced::widget::Text;
    use iced_aw::Bootstrap;

    use crate::{app, icon};
    use super::Level;


    impl Level {
        pub fn get_icon(&self) -> Option<Text> {
            use app::theme;
            match self {
                Level::Info => Some(icon!(Bootstrap::InfoCircle, theme::INFO_COLOR)),
                Level::Warning => Some(icon!(Bootstrap::ExclamationTriangle, theme::WARNING_COLOR)),
                Level::Error => Some(icon!(Bootstrap::ExclamationTriangleFill, theme::ERROR_COLOR)),
                _ => None,
            }
        }

        pub fn get_title(&self) -> &str {
            match self {
                Level::Info => "Info",
                Level::Warning => "Warning",
                Level::Error => "Error",
                _ => "",
            }
        }
    }


    #[derive(Debug, Clone)]
    pub struct Notification {
        pub level: Level,
        pub content: String,
        pub expire_at: Option<Instant>,
    }

    impl Notification {
        pub const DEFAULT_LIFETIME: f32 = 10.0;

        /// Create a new [`Notification`]
        /// Also logs `contents` (see [`log::Log`] )
        pub fn new(level: Level, content: String) -> Self {
            Notification {
                level,
                content,
                expire_at: Some(Instant::now() + Duration::from_secs_f32(Notification::DEFAULT_LIFETIME)),
            }
        }

        pub fn no_expiration(mut self) -> Self {
            self.expire_at = None;
            self
        }

        pub fn with_lifetime(mut self, duration_seconds: f32) -> Self {
            self.expire_at = Some(Instant::now() + Duration::from_secs_f32(duration_seconds));
            self
        }

        pub fn is_expired(&self) -> bool {
            if let Some(expiration) = self.expire_at {
                return Instant::now() >= expiration;
            }
            false
        }

        #[inline]
        pub fn get_title(&self) -> &str {
            self.level.get_title()
        }

        #[inline]
        pub fn get_icon(&self) -> Option<Text> {
            self.level.get_icon()
        }

    }

}
