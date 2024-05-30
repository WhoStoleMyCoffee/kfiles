use std::fs::create_dir;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::thread::JoinHandle;
use std::{io, thread};

use image::io::Reader as ImageReader;
use image::{ImageError, ImageFormat};
use iced::widget;
use thiserror::Error;

use crate::get_temp_dir;

pub const MAX_CACHE_SIZE_BYTES: u64 = 500_000;

static CACHE_DIR: OnceLock<PathBuf> = OnceLock::new();

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

/// Trait that handles thumbnail building
pub trait Thumbnail {
    fn get_thumbnail_cache_path(&self) -> PathBuf;

    fn create_thumbnail(&self) -> Result<(), ThumbnailError>;
}

impl Thumbnail for &Path {
    #[inline]
    fn get_thumbnail_cache_path(&self) -> PathBuf {
        get_cache_dir().join(format!("{}.jpg", hash_path(self)))
    }

    fn create_thumbnail(&self) -> Result<(), ThumbnailError> {
        let img = ImageReader::open(self)?.decode()?;

        let (w, h) = ThumbnailSize::Icon.size();
        img.thumbnail(w, h)
            .into_rgb8()
            .save(self.get_thumbnail_cache_path())?;
        Ok(())
    }
}

impl Thumbnail for PathBuf {
    #[inline]
    fn get_thumbnail_cache_path(&self) -> PathBuf {
        self.as_path().get_thumbnail_cache_path()
    }

    fn create_thumbnail(&self) -> Result<(), ThumbnailError> {
        self.as_path().create_thumbnail()
    }
}




type Worker = JoinHandle<Result<(), ThumbnailError>>;

#[derive(Debug)]
pub struct ThumbnailBuilder( Vec<Option<Worker>> );

impl ThumbnailBuilder {
    pub fn new(thread_count: usize) -> Self {
        ThumbnailBuilder((0..thread_count).map(|_| None).collect())
    }

    /// Builds a thumbnail for `path` on a thread, replacing a previous worker in the pool
    /// - `Ok(bool)` containing whether the job was accepted
    /// - `Err(`[`ThumbnailError`]`)` containing any error that occured during the previous
    ///   building process (of the replaced worker)
    pub fn build_for_path(&mut self, path: &Path) -> Result<bool, ThumbnailError> {
        for worker_maybe in self.0.iter_mut() {
            let is_done = worker_maybe
                .as_ref()
                .map(|h| h.is_finished())
                .unwrap_or(true);
            if !is_done {
                continue;
            }

            let handle = ThumbnailBuilder::build(path);
            if let Some(old_worker) = worker_maybe.replace(handle) {
                old_worker.join() .expect("Couldn't join thumbnail builder thread")?;
            }

            return Ok(true);
        }

        Ok(false)
    }

    fn build(path: &Path) -> Worker {
        let path = path.to_path_buf();
        thread::spawn(move || path.create_thumbnail())
    }
}

impl Drop for ThumbnailBuilder {
    fn drop(&mut self) {
        // Join all threads
        println!("[ThumbnailBuilder::drop()] Joining threads...");
        for worker in self.0.iter_mut() {
            if let Some(handle) = worker.take() {
                let _ = handle.join();
            }
        }
    }
}

pub fn get_cache_dir() -> &'static PathBuf {
    CACHE_DIR.get_or_init(|| {
        let pb = get_temp_dir().join("thumbnails/");
        if pb.exists() {
            return pb;
        }
        if let Err(err) = create_dir(&pb) {
            eprintln!("Failed to create thumbnail cache dir: {:?}", err);
        }
        pb
    })
}

fn hash_path(path: &Path) -> u64 {
    let mut s = DefaultHasher::new();
    path.hash(&mut s);
    s.finish()
}

// TODO return Result?
// pub fn is_file_supported(path: &Path) -> Result<bool, ImageError> {
//     match ImageFormat::from_path(path) {
//         Ok(_) => Ok(true),
//         Err(ImageError::Unsupported(_)) => Ok(false),
//         Err(err) => Err(err),
//     }
// }
pub fn is_file_supported(path: &Path) -> bool {
    ImageFormat::from_path(path).is_ok()
}

pub fn clear_thumbnails() -> std::io::Result<()> {
    std::fs::remove_dir_all(get_cache_dir())
}

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
    let cache_path = path.get_thumbnail_cache_path();
    if cache_path.exists() {
        return widget::image(cache_path);
    } else if path.is_dir() {
        return widget::image("assets/folder.png");
    }
    widget::image("assets/file.png")
    // Custom file icons ...
}




#[cfg(test)]
mod tests {
    use rand::Rng;
    use std::collections::HashSet;
    use walkdir::WalkDir;

    use super::*;

    #[test]
    #[ignore]
    fn rebuild_thumbnails() {
        let it = WalkDir::new("C:/Users/ddxte/Pictures/")
            .into_iter()
            .filter_entry(|de| {
                !de.file_name()
                    .to_str()
                    .map(|s| s.starts_with('.'))
                    .unwrap_or(false)
            })
            .flatten()
            .map(|de| de.into_path())
            .filter(|pb| pb.is_file());

        let thread_count = 10;
        let mut builder = ThumbnailBuilder::new(thread_count);

        println!("Building thumbnails... {thread_count} threads");
        for pb in it {
            if pb.is_dir() || !is_file_supported(&pb) {
                continue;
            }

            loop {
                match builder.build_for_path(&pb) {
                    Ok(true) => break,
                    Ok(false) => {},
                    Err(_) => break,
                }
            }
        }

        drop(builder);
        println!("Finishing...");
        thread::sleep(std::time::Duration::from_millis(1000));

        println!("Done");
        println!("Size = {}", cache_size().unwrap());
    }

    #[test]
    #[ignore]
    fn clear_cache() {
        println!("Clearing cache...");
        clear_thumbnails().unwrap();
    }

    #[test]
    #[ignore = "wee bit expensive. Run with --nocapture"]
    fn test_hash() {
        let mut map = HashSet::new();
        let it = WalkDir::new("C:/Users/ddxte/")
            .into_iter()
            .filter_entry(|de| {
                !de.file_name()
                    .to_str()
                    .map(|s| s.starts_with('.'))
                    .unwrap_or(false)
            })
            .flatten()
            .map(|de| de.into_path())
            .filter(|pb| pb.is_file());

        let mut total_count: usize = 0;
        let mut duplicate_count: usize = 0;
        for pb in it {
            let hash: u64 = hash_path(&pb);
            let is_duplicate: bool = !map.insert(hash);
            if is_duplicate {
                println!("Found duplicate: {} \t#{}", pb.display(), hash);
                duplicate_count += 1;
            }
            total_count += 1;
        }

        println!("Total duplicates: {duplicate_count} / {total_count}");
        assert!(duplicate_count < 10);
    }

    #[test]
    #[ignore]
    fn test_prune_cache() {
        let max_size = 100_000;
        let size = cache_size().unwrap();

        if size < max_size {
            println!("No need for pruning");
            dbg!(size, max_size);
            return;
        }

        println!("Size before pruning: {size}");

        let readdir = get_cache_dir()
            .read_dir()
            .unwrap()
            .flatten()
            .map(|de| de.path());

        let mut rng = rand::thread_rng();
        let prob: f64 = 1.0 - max_size as f64 / size as f64;
        let mut count: usize = 0;
        let mut files_count: usize = 0;

        println!("Pruning... target = {}%", prob * 100.0);

        for pb in readdir {
            files_count += 1;
            if !rng.gen_bool(prob) {
                continue;
            }
            let res = std::fs::remove_file(pb);
            if res.is_ok() {
                count += 1;
            }
        }

        println!(
            "Removed {} / {} files ({}%)",
            count,
            files_count,
            (count as f32 / files_count as f32) * 100.0
        );
        let size = cache_size().unwrap();
        println!("Size after pruning: {size}");
    }
}
