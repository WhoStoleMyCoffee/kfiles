use std::fs::create_dir;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use image::io::Reader as ImageReader;
use image::ImageError;
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
enum ThumbnalSize {
    Icon,
    Small,
    Medium,
    Large,
    Custom(u32, u32),
}

impl ThumbnalSize {
    pub fn size(&self) -> (u32, u32) {
        match *self {
            ThumbnalSize::Icon => (64, 64),
            ThumbnalSize::Small => (128, 128),
            ThumbnalSize::Medium => (256, 256),
            ThumbnalSize::Large => (512, 512),
            ThumbnalSize::Custom(w, h) => (w, h),
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





/// TODO impl Thumbnail for AsRef<Path>?
trait Thumbnail {
    fn get_cache_path(&self) -> PathBuf;

    fn create_thumbnail(&self) -> Result<(), ThumbnailError>;

    fn get_thumbnail(&self);
}

impl Thumbnail for &Path {
    #[inline]
    fn get_cache_path(&self) -> PathBuf {
        get_cache_dir() .join( format!("{}.jpg", hash_path(self)) )
    }

    fn create_thumbnail(&self) -> Result<(), ThumbnailError> {
        let img = ImageReader::open(self)?
            .decode()?;

        let (w, h) = ThumbnalSize::Icon.size();
        img.thumbnail(w, h)
            .into_rgb8()
            .save(self.get_cache_path())?;
        Ok(())
    }

    fn get_thumbnail(&self) {
        todo!()
    }
}





#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::Path;
    use walkdir:: WalkDir;

    use super::*;

    #[test]
    fn thumbnails_single() {
        let path = Path::new("C:/Users/ddxte/Pictures/bread.JPG");
        assert!( path.create_thumbnail().is_ok() );

        let path = Path::new("C:/Users/ddxte/Documents/AutoClicker.exe");
        let err = path.create_thumbnail() .unwrap_err();
        assert!( err.is_unsupported() );
    }

    #[test]
    fn test_thumbnails_bulk() {
        let it = WalkDir::new("C:/Users/ddxte/Pictures/art stuff/")
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
            match pb.as_path().create_thumbnail() {
                Ok(()) => {
                    println!("Created thumbnail for {}", pb.display());
                },

                Err(err) => {
                    if err.is_unsupported() {
                        println!("Ignoring {}: Unsupported", pb.display());
                        continue;
                    }

                    println!("Error on {}: {}", pb.display(), err);
                },
            }

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
