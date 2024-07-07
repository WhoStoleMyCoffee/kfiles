use std::fs::create_dir_all;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;
use std::{io, thread};

use image::io::Reader as ImageReader;
use image::{ImageError, ImageFormat};
use iced::widget;
use thiserror::Error;

use crate::{error, get_temp_dir, log};

const FORMAT: &str = "jpg";


#[derive(Debug, Error)]
pub enum ThumbnailError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    ImageError(#[from] ImageError),
}

impl ThumbnailError {
    pub fn is_unsupported(&self) -> bool {
        matches!(
            *self,
            ThumbnailError::ImageError(ImageError::Unsupported(_))
        )
    }
}

#[allow(unused)]
enum ThumbnailSize {
    Icon,
    Small,
    Medium,
    Large,
    Custom(u32, u32),
}

impl ThumbnailSize {
    pub fn size(&self) -> (u32, u32) {
        match *self {
            ThumbnailSize::Icon => (64, 64),
            ThumbnailSize::Small => (128, 128),
            ThumbnailSize::Medium => (256, 256),
            ThumbnailSize::Large => (512, 512),
            ThumbnailSize::Custom(w, h) => (w, h),
        }
    }
}




/// Get the file where the thumbnail for `path` should be
pub fn get_thumbnail_cache_path(path: &Path) -> PathBuf {
    get_cache_dir_or_create().join(format!("{}.{}", hash_path(path), FORMAT))
}

/// Create a thumbnail for `path`, assuming it is an image file
pub fn create_thumbnail(path: &Path) -> Result<(), ThumbnailError> {
    let img = ImageReader::open(path)?
        .decode()?;

    let (w, h) = ThumbnailSize::Icon.size();
    img.thumbnail(w, h)
        .into_rgb8()
        .save(get_thumbnail_cache_path(path))?;
    Ok(())
}




#[derive(Debug)]
pub struct BuildError {
    pub path: PathBuf,
    pub error: ThumbnailError,
}

type Worker = JoinHandle<Result<(), BuildError>>;

/// Builds thumbnails.
/// You're welcome
#[derive(Debug)]
pub struct ThumbnailBuilder {
    workers: Vec<Option<Worker>>,
}

impl ThumbnailBuilder {
    /// Create a new [`ThumbnailBuilder`] with `thread_count` threads
    pub fn new(thread_count: u8) -> Self {
        ThumbnailBuilder {
            workers: (0..thread_count).map(|_| None).collect(),
        }
    }

    /// Updates the thread pool and return results from the joined threads
    /// See also [`ThumbnailBuilder::build`]
    #[must_use]
    pub fn update(&mut self) -> Vec<Result<(), BuildError>> {
        let mut results = Vec::new();

        for worker_maybe in self.workers.iter_mut() {
            // bro i want `take_if()`
            let Some(handle) = worker_maybe else { continue; };
            if !handle.is_finished() {
                continue; 
            }

            // SAFETY: we already checked above that `worker_maybe` is `Some`
            let handle = unsafe { worker_maybe.take().unwrap_unchecked() };
            let res = handle.join() .expect("Couldn't join thumbnail worker thread");
            results.push(res);
        }

        results
    }

    /// Builds a thumbnail for `path` on a thread, replacing empty slots in the thread pool
    /// Ideally, you'd call [`ThumbnailBuilder::update`] to join finished threads first
    /// Returns whether the job was accepted
    pub fn build(&mut self, path: &Path) -> bool {
        for worker_maybe in self.workers.iter_mut() {
            if worker_maybe.is_some() {
                continue;
            }

            let handle = ThumbnailBuilder::build_for_path(path);
            *worker_maybe = Some(handle);

            return true;
        }

        false
    }

    /// Build the thumbnail for `path` on a thread
    /// Returns the spawned [`Worker`]
    fn build_for_path(path: &Path) -> Worker {
        let path = path.to_path_buf();
        thread::spawn(move || {
            create_thumbnail(&path)
                .map_err(|error| BuildError { path, error })
        })
    }

    pub fn join_threads(&mut self) {
        for worker in self.workers.iter_mut() {
            let Some(handle) = worker.take() else {
                continue;
            };

            if let Err(err) = handle.join() {
                error!("[ThumbnailBuilder::drop()] Failed to join worker thread:\n {err:?}");
            }
        }
    }

}

impl Drop for ThumbnailBuilder {
    fn drop(&mut self) {
        self.join_threads();
    }
}


pub fn get_cache_dir() -> PathBuf {
    get_temp_dir().join("thumbnails/")
}

pub fn get_cache_dir_or_create() -> PathBuf {
    let path = get_cache_dir();
    if !path.exists() {
        if let Err(err) = create_dir_all(&path) {
            error!("Failed to create thumbnail cache dir:\n {:?}", err);
        }
    }
    path
}


/// Get the hash of `path`
fn hash_path(path: &Path) -> u64 {
    let mut s = DefaultHasher::new();
    path.hash(&mut s);
    s.finish()
}

/// Returns whether the file `path` is a valid file for thumbnailing
pub fn is_file_supported(path: &Path) -> bool {
    ImageFormat::from_path(path).is_ok()
}

/// Deletes the thumbnail cache dir
pub fn clear_thumbnails_cache() -> std::io::Result<()> {
    std::fs::remove_dir_all(get_cache_dir())
}

/// Get the size of the thumbnail cache folder
pub fn cache_size() -> io::Result<u64> {
    let size: u64 = get_cache_dir()
        .read_dir()?
        .flatten()
        .flat_map(|de| de.metadata())
        .fold(0, |acc, meta| acc + meta.len());
    Ok(size)
}

/// Removes cached thumbnails randomly until `max_size_bytes`.
/// May remove more or less depending on rng ¯\_(ツ)_/¯
/// The idea is that unused files will slowly be removed over time, and frequently
/// used ones will just be rebuilt when necessary.
/// Returns how many bytes were trimmed
pub fn trim_cache(max_size_bytes: u64) -> io::Result<u64> {
    let size: u64 = cache_size()?;
    // No need to trim
    if size < max_size_bytes {
        return Ok(0);
    }

    trim_cache_percent(1.0 - max_size_bytes as f64 / size as f64)
}

/// Removes cached thumbnails randomly until `max_size_bytes`.
/// The idea is that unused files will slowly be removed over time, and frequently
/// used ones will just be rebuilt when necessary.
/// Returns how many bytes were trimmed
pub fn trim_cache_strict(max_size_bytes: u64) -> io::Result<u64> {
    let original_size: u64 = cache_size()?;
    // No need to trim
    if original_size < max_size_bytes {
        return Ok(0);
    }

    let mut size: u64 = original_size;
    while size >= max_size_bytes {
        size -= trim_cache_percent(1.0 - max_size_bytes as f64 / size as f64)?;
    }

    Ok(original_size - size)
}

/// Removes `target_percentage` (between 0 and 1) percent of cached thumbnails randomly.
/// The idea is that unused files will slowly be removed over time, and frequently
/// used ones will just be rebuilt when necessary.
/// Returns how many bytes were trimmed
fn trim_cache_percent(target_percentage: f64) -> io::Result<u64> {
    if target_percentage <= 0.0 {
        return Ok(0);
    }

    use rand::Rng;
    let mut rng = rand::thread_rng();

    let readdir = get_cache_dir().read_dir()?.flatten().map(|de| de.path());
    let mut trimmed_bytes: u64 = 0;
    for pb in readdir {
        if !rng.gen_bool(target_percentage) {
            continue;
        }

        let size = pb.metadata().map(|m| m.len()).unwrap_or(0);

        if std::fs::remove_file(&pb).is_ok() {
            trimmed_bytes += size;
        }
    }
    Ok(trimmed_bytes)
}



pub fn load_thumbnail_for_path(path: &Path) -> widget::Image<widget::image::Handle> {
    let cache_path = get_thumbnail_cache_path(&path);
    if cache_path.exists() {
        return widget::image(cache_path);
    } else if path.is_dir() {
        return widget::image("assets/file_icons/folder.png");
    }
    widget::image("assets/file_icons/file.png")
    // Custom file icons ...
}

