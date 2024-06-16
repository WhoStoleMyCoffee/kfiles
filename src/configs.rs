use std::{fs::{create_dir_all, File}, io::{self, Read, Write}, path::PathBuf, sync::{Mutex, MutexGuard, OnceLock}};

use nanoserde::{DeJson, DeJsonErr, SerJson};
use thiserror::Error;


static GLOBAL: OnceLock<Mutex<Configs>> = OnceLock::new();



#[derive(Debug, Error)]
pub enum SaveError {
    #[error(transparent)]
    IO(#[from] io::Error),
}

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("invalid file name")]
    InvalidName,
    #[error(transparent)]
    IO(#[from] io::Error),
    #[error("failed to parse: {0}")]
    ParseError(#[from] DeJsonErr),
}



/// TODO documentation
pub fn get_save_path() -> PathBuf {
    use crate::APP_NAME;

    directories::BaseDirs::new()
        .expect("failed to get base dirs")
        .config_dir()
        .to_path_buf()
        .join( format!("{APP_NAME}/configs.json") )
}


/// TODO documentation
pub fn load_configs() -> Result<Configs, LoadError> {
    let mut contents = String::new();
    let path = get_save_path();
    File::open(path)?
        .read_to_string(&mut contents)?;

    let configs: Configs = SerConfigs::deserialize_json(&contents)?
        .into();

    Ok(configs)
}

/// TODO documentation
pub fn save_configs(configs: &Configs) -> Result<(), SaveError> {
    let path = get_save_path();
    if !path.exists() {
        let dir = path.parent().expect("could not get parent dir");
        create_dir_all(dir)?;
    }

    let string = SerConfigs::from(configs).serialize_json();
    File::create(path)?
        .write_all(string.as_bytes())?;
    Ok(())
}



/// TODO documentation
pub fn global() -> MutexGuard<'static, Configs> {
    let mutex = GLOBAL.get()
        .expect("global configs not initialized");
    mutex.lock()
        .unwrap_or_else(|mut err| {
            println!("TODO error handling: mutex was poisonned");
            **err.get_mut() = Configs::default();
            mutex.clear_poison();
            err.into_inner()
        })
}

/// TODO documentation
pub fn set_global(configs: Configs) -> Result<(), Configs> {
    GLOBAL.set( Mutex::new(configs) )
        .map_err(|m| m.into_inner() .unwrap() )
}



#[derive(Debug, Clone)]
pub struct Configs {
    pub thumbnail_cache_size: u64,
}

impl Configs {
    #[inline]
    pub fn save(&self) -> Result<(), SaveError> {
        save_configs(self)
    }
}

impl Default for Configs {
    fn default() -> Self {
        Configs {
            thumbnail_cache_size: 500_000,
        }
    }
}



#[derive(Debug, Clone, SerJson, DeJson)]
struct SerConfigs {
    thumbnail_cache_size: u64,
}

impl From<&Configs> for SerConfigs {
    fn from(value: &Configs) -> Self {
        SerConfigs {
            thumbnail_cache_size: value.thumbnail_cache_size,
        }
    }
}

impl From<SerConfigs> for Configs {
    fn from(value: SerConfigs) -> Self {
        Configs {
            thumbnail_cache_size: value.thumbnail_cache_size,
        }
    }
}


