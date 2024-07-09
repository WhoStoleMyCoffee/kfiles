use std::{fs::{create_dir_all, File}, io::{self, Read, Write}, path::PathBuf, sync::{Mutex, MutexGuard, OnceLock}};

use nanoserde::{DeJson, DeJsonErr, SerJson};
use thiserror::Error;

use crate::{error, APP_NAME};


static GLOBAL: OnceLock<Mutex<Configs>> = OnceLock::new();



#[derive(Debug, Error)]
pub enum LoadError {
    #[error("invalid file name")]
    InvalidName,
    #[error(transparent)]
    IO(#[from] io::Error),
    #[error("failed to parse: {0}")]
    ParseError(#[from] DeJsonErr),
}

/// Gets the save path of the configs file
/// Returns `None` if no [`directories::BaseDirs`] was found
pub fn get_save_path() -> io::Result<PathBuf> {
    Ok(directories::BaseDirs::new()
        .ok_or_else(||
            io::Error::new(io::ErrorKind::NotFound, "Failed to get BaseDirs instance")
        )?
        .config_dir()
        .to_path_buf()
        .join(format!("{}/configs.json", APP_NAME)))
}


/// Load the configs file from disk
pub fn load_configs() -> Result<Configs, LoadError> {
    let mut contents = String::new();
    let path = get_save_path()?;
    File::open(path)?
        .read_to_string(&mut contents)?;

    let configs: Configs = Configs::deserialize_json(&contents)?;

    Ok(configs)
}

/// Save the given [`Configs`] to disk, creating directories if necessary
pub fn save_configs(configs: &Configs) -> io::Result<()> {
    let path = get_save_path()?;
    if !path.exists() {
        let dir = path.parent().expect("configs path should not be root or empty");
        create_dir_all(dir)?;
    }

    let string: String = configs.serialize_json();
    File::create(path)?
        .write_all(string.as_bytes())?;
    Ok(())
}



/// Gets the global [`Configs`] instance
/// This returns a [`MutexGuard`], so it will lock it until dropped
///
/// # Panics
///
/// Panics if the [`GLOBAL`] static is not initialized
/// See [`set_global`]
pub fn global() -> MutexGuard<'static, Configs> {
    let mutex = GLOBAL.get()
        .expect("global configs should be initialized");
    mutex.lock()
        .unwrap_or_else(|mut err| {
            error!("Error while getting global Configs instance:\n Mutex was poisonned");
            **err.get_mut() = Configs::default();
            mutex.clear_poison();
            err.into_inner()
        })
}

/// Attempts to set the global [`Configs`] instance
/// If already set, it will be unchanged, and this function will return the inputted `configs`
pub fn set_global(configs: Configs) -> Result<(), Configs> {
    #[allow(clippy::unwrap_used)]
    GLOBAL.set( Mutex::new(configs) )
        .map_err(|m| m.into_inner()
            // SAFETY: Will not panic because there are no other users of the new Mutex
            .unwrap()
        )
}



#[derive(Debug, Clone, SerJson, DeJson)]
pub struct Configs {
    pub thumbnail_cache_size: u64,
    pub thumbnail_thread_count: u8,
    pub thumbnail_update_prob: f32,
    pub thumbnail_check_count: u32,
    pub max_result_count: usize,
    pub max_results_per_tick: usize,
    pub update_rate_ms: u64,
}

impl Configs {
    #[inline]
    pub fn save(&self) -> io::Result<()> {
        save_configs(self)
    }
}

impl Default for Configs {
    fn default() -> Self {
        Configs {
            thumbnail_cache_size: 500_000,
            thumbnail_thread_count: 4,
            thumbnail_update_prob: 0.01,
            thumbnail_check_count: 4,
            max_results_per_tick: 10,
            max_result_count: 256,
            update_rate_ms: 100,
        }
    }
}


/*
 * SerConfigs
 * Bring this back if you need to if you're serializing types unsupported by nanoserde

#[derive(Debug, Clone, SerJson, DeJson)]
struct SerConfigs {
    thumbnail_cache_size: u64,
    max_result_count: usize,
    max_results_per_tick: usize,
    update_rate_ms: u64,
}

impl From<&Configs> for SerConfigs {
    fn from(value: &Configs) -> Self {
        SerConfigs {
            thumbnail_cache_size: value.thumbnail_cache_size,
            max_result_count: value.max_result_count,
            max_results_per_tick: value.max_results_per_tick,
            update_rate_ms: value.update_rate_ms,
        }
    }
}

impl From<SerConfigs> for Configs {
    fn from(value: SerConfigs) -> Self {
        Configs {
            thumbnail_cache_size: value.thumbnail_cache_size,
            max_result_count: value.max_result_count,
            max_results_per_tick: value.max_results_per_tick,
            update_rate_ms: value.update_rate_ms,
        }
    }
}
*/


