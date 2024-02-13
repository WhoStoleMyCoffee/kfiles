use std::fs::{self, File};
use std::io::{ self, Write };
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};

use directories::UserDirs;
use serde::{Deserialize, Serialize};

use crate::util::{self, get_all_files_at_recursive, read_lines};
use crate::{AppError, CONFIGS};

#[macro_export]
macro_rules! themevar {
    ($c:ident) => {
        Color::from(Configs::global().theme.$c)
    };
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Configs {
    pub scroll_margin: u8,
    pub default_path: PathBuf,
    pub search_ignore_extensions: Vec<String>,
    pub max_recent_count: usize,
    pub theme: ColorTheme,
    pub performance: PerformanceConfigs,
}

impl Configs {
    #[inline]
    pub fn global() -> &'static Self {
        CONFIGS.get().expect("Configs not initialized")
    }

    pub fn theme() -> &'static ColorTheme {
        &Self::global().theme
    }

    pub fn performance() -> &'static PerformanceConfigs {
        &Self::global().performance
    }
}

impl Default for Configs {
    fn default() -> Self {
        let userdir: UserDirs = UserDirs::new().expect("Could not find home directory");
        let home_dir: &Path = userdir.home_dir();

        Self {
            scroll_margin: 4,
            default_path: PathBuf::from(home_dir),
            search_ignore_extensions: Vec::new(),
            max_recent_count: 64,
            theme: ColorTheme::default(),
            performance: PerformanceConfigs::default(),
        }
    }
}

pub struct FavoritesList(Vec<PathBuf>);

impl FavoritesList {
    pub fn load(file: &Path) -> io::Result<Self> {
        let mut list: FavoritesList = FavoritesList(Vec::new());

        *list = read_lines(file)?
            .map_while(Result::ok)
            .map(PathBuf::from)
            .filter(|pathbuf| pathbuf.exists())
            .collect();

        Ok(list)
    }

    pub fn query(&self, query: &str) -> Option<&PathBuf> {
        let query: String = query.to_lowercase();
        self.iter()
            .find(|p| util::path2string(p).to_lowercase().contains(&query))
    }

    /// Returns true if path was added to favorites, false otherwise
    pub fn toggle(&mut self, path: &PathBuf) -> bool {
        let list = &mut self.0;
        if let Some(index) = list.iter().position(|p| p == path) {
            list.remove(index);
            return false;
        }
        list.push(path.clone());
        true
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let content: String = self.0.iter()
            .filter_map(|p| p.as_path().to_str())
            .collect::<Vec<&str>>()
            .join("\n");

        let mut file = File::create(path)?;
        file.write_all(content.as_bytes())?;
        Ok(())
    }
}

impl Default for FavoritesList {
    fn default() -> Self {
        let userdir: UserDirs = UserDirs::new().expect("Could not find home directory");
        FavoritesList(vec![userdir.home_dir().into()])
    }
}

impl AsRef<Vec<PathBuf>> for FavoritesList {
    fn as_ref(&self) -> &Vec<PathBuf> {
        &self.0
    }
}

impl Deref for FavoritesList {
    type Target = Vec<PathBuf>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for FavoritesList {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug)]
pub struct RecentList {
    pub max_len: usize,
    list: Vec<PathBuf>,
}

impl RecentList {
    pub fn new(max_len: usize) -> Self {
        Self {
            max_len,
            list: Vec::new(),
        }
    }

    pub fn load(file: &Path, max_len: usize) -> Result<Self, std::io::Error> {
        let mut list: RecentList = RecentList::new(max_len);

        *list = read_lines(file)?
            .map_while(Result::ok)
            .map(PathBuf::from)
            .filter(|pathbuf| pathbuf.exists())
            .collect();

        Ok(list)
    }

    pub fn push(&mut self, item: PathBuf) {
        if let Some(i) = self.list.iter().position(|element| element == &item) {
            self.list.remove(i);
        }

        self.list.insert(0, item);
        self.list.truncate(self.max_len);
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let content: String = self
            .list
            .iter()
            .filter_map(|p| p.as_path().to_str())
            .collect::<Vec<&str>>()
            .join("\n");

        let mut file = File::create(path)?;
        file.write_all(content.as_bytes())?;
        Ok(())
    }
}

impl Deref for RecentList {
    type Target = Vec<PathBuf>;

    fn deref(&self) -> &Self::Target {
        &self.list
    }
}

impl DerefMut for RecentList {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.list
    }
}

impl AsRef<Vec<PathBuf>> for RecentList {
    fn as_ref(&self) -> &Vec<PathBuf> {
        &self.list
    }
}



pub trait ScriptKey {
    fn get_run_path(&self, list: &[PathBuf]) -> Option<PathBuf>;
}

impl ScriptKey for usize {
    fn get_run_path(&self, list: &[PathBuf]) -> Option<PathBuf> {
        list.get(*self).cloned()
    }
}

impl ScriptKey for PathBuf {
    fn get_run_path(&self, _: &[PathBuf]) -> Option<PathBuf> {
        Some(self.clone())
    }
}

impl ScriptKey for &str {
    fn get_run_path(&self, list: &[PathBuf]) -> Option<PathBuf> {
        list.iter()
            .filter(|pb| pb.ends_with(self))
            .nth(0)
            .cloned()
    }
}


#[derive(Debug, Default)]
pub struct ScriptsList(Vec<PathBuf>);

impl ScriptsList {
    pub fn load(dir: &Path) -> io::Result<Self> {
        let files: Vec<PathBuf> = match get_all_files_at_recursive(dir) {
            Ok(files) => files,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                    if let Err(err) = fs::create_dir(dir) {
                        return Err(err);
                    } else {
                        Vec::new()
                    }
            },
            Err(err) => {
                return Err(err);
            },
        };
        Ok(ScriptsList( files ))
    }

    pub fn run<S>(&self, script: S, at_dir: Option<&Path>) -> Result<(), AppError>
    where S: ScriptKey {
        let run_path: PathBuf = script.get_run_path(&self.0)
            .ok_or(io::Error::new(io::ErrorKind::NotFound, "Script not found") )?;

        use std::process::{ Command, Stdio };

        let mut command: Command;
        if cfg!(target_os = "windows") {
            command = Command::new("cmd");
            command.args([ "/C", "start", &run_path.display().to_string() ]);
        } else {
            todo!("KFiles start_terminal not yet implemented for OS' other than windows")
        };

        let dir: &Path = match at_dir {
            Some(dir) => dir,
            None => run_path.parent()
                .ok_or(io::Error::other("Failed to get script parent directory") )?
        };

        command.current_dir(dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        Ok(())
    }
}

impl AsRef<Vec<PathBuf>> for ScriptsList {
    fn as_ref(&self) -> &Vec<PathBuf> {
        &self.0
    }
}

impl Deref for ScriptsList {
    type Target = Vec<PathBuf>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ScriptsList {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}






pub type Col8 = (u8, u8, u8);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ColorTheme {
    pub folder_color: Col8,
    pub file_color: Col8,
    pub special_color: Col8,
    pub bg_color: Col8,
    pub text_color: Col8,
    pub comment_color: Col8,
    pub error_color: Col8,
}

impl Default for ColorTheme {
    fn default() -> Self {
        Self {
            folder_color: (255, 209, 84),
            file_color: (206, 217, 214),
            special_color: (110, 209, 255),
            bg_color: (35, 47, 54),
            text_color: (220, 220, 200),
            comment_color: (100, 100, 100),
            error_color: (224, 88, 79),
        }
    }
}



#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfigs {
    pub update_rate: u32,
    pub max_search_queue_len: Option<usize>,
    pub search_thread_count: u8,
    pub thread_active_ms: u16,
    pub thread_inactive_ms: u16,
}

impl Default for PerformanceConfigs {
    fn default() -> Self {
        Self {
            max_search_queue_len: Some(1024),
            search_thread_count: 4,
            update_rate: 12,
            thread_active_ms: 1,
            thread_inactive_ms: 500,
        }
    }
}



