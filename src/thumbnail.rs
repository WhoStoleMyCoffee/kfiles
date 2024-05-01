use std::fs::create_dir;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use image::io::Reader as ImageReader;
use image::{ImageError, ImageFormat};
use thiserror::Error;

use crate::get_temp_dir;


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




pub fn get_cache_dir() -> &'static PathBuf {
    CACHE_DIR.get_or_init(|| {
        let pb = get_temp_dir() .join("thumbnails/");
        if !pb.exists() {
            create_dir(&pb).unwrap(); // handle this unwrap at some point lol
        }
        pb
    })
}

fn hash_path(path: &Path) -> u64 {
    let mut s = DefaultHasher::new();
    path.hash(&mut s);
    s.finish()
}

pub fn is_file_supported(path: &Path) -> Result<bool, ImageError> {
    match ImageFormat::from_path(path) {
        Ok(_) => Ok(true),
        Err(ImageError::Unsupported(_)) => Ok(false),
        Err(err) => Err(err),
    }
}





/// TODO impl Thumbnail for AsRef<Path>?
pub trait Thumbnail {
    /// TODO rename
    fn get_cache_path(&self) -> PathBuf;

    fn create_thumbnail(&self) -> Result<(), ThumbnailError>;
}

impl Thumbnail for &Path {
    #[inline]
    // todo maybe check out other image formats like png, *qoi* (promising!)
    fn get_cache_path(&self) -> PathBuf {
        get_cache_dir() .join( format!("{}.jpeg", hash_path(self)) )
    }

    fn create_thumbnail(&self) -> Result<(), ThumbnailError> {
        let img = ImageReader::open(self)?
            .decode()?;

        let (w, h) = ThumbnailSize::Icon.size();
        img.thumbnail(w, h)
            .into_rgb8()
            .save(self.get_cache_path())?;
        Ok(())
    }

}






#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::Path;
    use walkdir:: WalkDir;

    use super::*;

    #[test]
    #[ignore]
    fn rebuild_thumbnails() {
        let it = WalkDir::new("C:/Users/ddxte/Pictures/")
            .into_iter()
            .filter_entry(|de|
                !de.file_name()
                    .to_str()
                    .map(|s| s.starts_with('.'))
                    .unwrap_or(false)
            )
            .flatten()
            .map(|de| de.into_path())
            .filter(|pb| pb.is_file());

        for pb in it {

            std::thread::spawn(move || {
                match pb.as_path().create_thumbnail() {
                    Ok(()) => {
                        println!("Created thumbnail for {}", pb.display());
                    },

                    Err(err) => {
                        if !err.is_unsupported() {
                            println!("Error on {}: {}", pb.display(), err);
                        }
                    },
                }
            });

        }
    }


    #[test]
    #[ignore = "non-funtional"]
    fn test_cache_size() {
        // TODO use fs_extra: https://stackoverflow.com/questions/60041710/how-to-check-directory-size
        let meta = std::fs::metadata("assets/wimdy.jpg") .unwrap();
        let len = meta.len();
        println!("Size of assets folder: {} bytes", len);
    }

    #[test]
    #[ignore = "wee bit expensive. Run with --nocapture"]
    fn test_hash() {
        let mut map = HashSet::new();
        let it = WalkDir::new("C:/Users/ddxte/")
            .into_iter()
            .filter_entry(|de|
                !de.file_name()
                    .to_str()
                    .map(|s| s.starts_with('.'))
                    .unwrap_or(false)
            )
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

}
