use std::env;
use std::fmt::Display;
use std::path::PathBuf;
use std::sync::OnceLock;

use clean_path::Clean;
use config::{Configs, FavoritesList};
use confy::ConfyError;
use console_engine::KeyModifiers;

pub mod app;
pub mod config;
pub mod filebuffer;
pub mod search;
pub mod util;

use app::App;

// Search panel offset from the edges of the screen
const SEARCH_PANEL_MARGIN: (u32, u32) = (4, 2);
const VERSION: &str = env!("CARGO_PKG_VERSION");
const APPNAME: &str = env!("CARGO_PKG_NAME");
const CONFIG_PATH: &str = "configs";
const RECENT_DIRS_FILE_NAME: &str = "recent.txt";
const FAVORITES_LIST_FILE_NAME: &str = "favorites.txt";

const CONTROL_SHIFT: u8 = KeyModifiers::CONTROL.union(KeyModifiers::SHIFT).bits();

static CONFIGS: OnceLock<Configs> = OnceLock::new();

#[derive(Debug)]
pub enum AppError {
    ConfigError(confy::ConfyError),
    IO(std::io::Error),
    OpenError(opener::OpenError),
    Other(String),
}

impl From<confy::ConfyError> for AppError {
    fn from(value: confy::ConfyError) -> Self {
        Self::ConfigError(value)
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::IO(value)
    }
}

impl From<&str> for AppError {
    fn from(value: &str) -> Self {
        Self::Other(value.to_string())
    }
}

impl From<opener::OpenError> for AppError {
    fn from(value: opener::OpenError) -> Self {
        Self::OpenError(value)
    }
}

impl Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConfigError(err) => err.fmt(f),
            Self::IO(err) => err.fmt(f),
            Self::OpenError(err) => err.fmt(f),
            Self::Other(str) => write!(f, "{}", str),
        }
    }
}

impl std::error::Error for AppError {}

pub enum RunOption {
    AtPath(PathBuf),
    TryFavorite(String),
    AtDefaultPath,
    Help,
    Config,
}

fn main() {
    let cfg: Result<Configs, AppError> =
        confy::load(APPNAME, Some(CONFIG_PATH)).map_err(AppError::from);

    // Process command line arguments
    let (run_path, cfg): (PathBuf, Configs) = match (parse_cli(env::args()), cfg) {
        // We don't care about the configs when showing help message
        (Ok(RunOption::Help), _) => {
            print_help();
            pause(false);
            return;
        }

        (Ok(RunOption::Config), _) => {
            let Ok(cfg_path) = confy::get_configuration_file_path(APPNAME, Some(CONFIG_PATH))
            else {
                println!("Error: Failed to get configuration file path.");
                pause(true);
                return;
            };

            if let Err(err) = opener::open(&cfg_path) {
                println!("Error: Failed to open configuration file:\n\t{}.\nRevealing in file explorer instead...", err);
                if let Err(err) = opener::reveal(&cfg_path) {
                    println!(
                        "Error: Failed to reveal configuration file in file explorer:\n\t{}",
                        err
                    );
                    pause(false);
                }
                return;
            }

            return;
        }

        // Error occured somewhere
        (Err(err), _) | (_, Err(err)) => {
            handle_app_err(err);
            return;
        }

        // All good, keep parsing
        (Ok(opt), Ok(cfg)) => match opt {
            RunOption::AtDefaultPath => (cfg.default_path.clone(), cfg),

            RunOption::AtPath(p) => (p, cfg),

            RunOption::TryFavorite(query) => {
                let list_path: PathBuf = match get_favorites_list_path() {
                    Ok(p) => p,
                    Err(err) => {
                        println!("Error: Failed to get favorites list path.\n\t{}", err);
                        pause(false);
                        return;
                    }
                };

                let list = FavoritesList::load(&list_path).unwrap_or_default();
                (list.query(&query).unwrap_or(&cfg.default_path).clone(), cfg)
            }

            RunOption::Help | RunOption::Config => unreachable!(), // Already checked up there
        },
    };

    if CONFIGS.set(cfg).is_err() {
        println!("Error: Failed to set global configs.");
        pause(false);
        return;
    }

    // Setup app
    let mut app: App = match App::new(&run_path) {
        Ok(app) => app,

        Err(err) => {
            handle_app_err(err);
            return;
        }
    };

    // MAIN LOOP ----------------------------------------------------------------------------
    loop {
        let state: app::AppState = app.run();

        if let app::AppState::Exit(_exit_path) = state {
            // TODO why no work. eh?
            // if let Some(path) = exit_path {
            // 	let _ = env::set_current_dir(path);
            // }
            break;
        }
    }
}

fn parse_cli(mut args: env::Args) -> Result<RunOption, AppError> {
    if let Some(a) = args.nth(1) {
        match a.as_str() {
            "--help" | "-h" => Ok(RunOption::Help),

            "--config" | "--configs" | "-c" | "-cfg" | "--cfg" => Ok(RunOption::Config),

            "--favorites" | "-f" => {
                let query: String = args.next()
                    .ok_or("Invalid syntax for --favorites. Expected field <query>\n Syntax: \t --favorites <query> or -f <query>")?;
                Ok(RunOption::TryFavorite(query))
            }

            s => {
                let current_dir: PathBuf = env::current_dir().map_err(AppError::from)?;
                let joined = current_dir.join(s);
                Ok(RunOption::AtPath(if joined.exists() {
                    joined.clean()
                } else {
                    current_dir
                }))
            }
        }
    } else {
        Ok(RunOption::AtDefaultPath)
    }
}

fn get_recent_dirs_path() -> Result<PathBuf, AppError> {
    confy::get_configuration_file_path(APPNAME, None)
        .map_err(AppError::from)?
        .parent()
        .map(|path| path.with_file_name(RECENT_DIRS_FILE_NAME))
        .ok_or("Failed to load recent directories".into())
}

fn get_favorites_list_path() -> Result<PathBuf, AppError> {
    confy::get_configuration_file_path(APPNAME, None)
        .map_err(AppError::from)
        .map(|path| path.with_file_name(FAVORITES_LIST_FILE_NAME))
}

fn handle_app_err(err: AppError) {
    match err {
        // Wrong config format
        AppError::ConfigError(ConfyError::BadTomlData(err)) => {
            println!("Error while loading configs:\n	{}", err);
            if let Ok(p) = confy::get_configuration_file_path(APPNAME, Some(CONFIG_PATH)) {
                println!("Tip: You can find your config file at {}", p.display());
            }
            pause(true);
        }

        // General config error
        AppError::ConfigError(err) => {
            println!("Error while loading configs:\n	{}", err);
            pause(false);
        }

        // Engine init error
        AppError::IO(err) => {
            println!("Error: {}", err);
            pause(false);
        }

        AppError::OpenError(err) => {
            println!("Error opening: {}", err);
            pause(false);
        }

        AppError::Other(s) => {
            println!("Error: {}", s);
            pause(false);
        }
    }
}

fn pause(with_help_tip: bool) {
    if with_help_tip {
        println!("Tip: You can run KFiles with `kfiles --help` for more info");
    }
    println!("Press ENTER to continue...");
    let _ = std::io::stdin().read_line(&mut String::new());
}



macro_rules! printhelp {
    ($al:expr; $name:expr, $desc:expr) => {
        println!("    {}{}{}", $name, " ".repeat($al - $name.len() - 4), $desc);
    };

    ($t:expr => $al:expr; $name:expr, $desc:expr) => {
        println!("{}{}{}{}", " ".repeat($t), $name, " ".repeat($al - $name.len() - $t), $desc);
    };

    ($al:expr; $( $name:expr, $desc:expr );*;) => {
        {
            let mut v: Vec<String> = Vec::new();
            $(
                v.push( format!("    {}{}{}", $name, " ".repeat($al - $name.len() - 4), $desc ) );
            )*
            println!("{}", v.join("\n"));
        }
    };

    ($t:expr => $al:expr; $( $name:expr, $desc:expr );*;) => {
        {
            let mut v: Vec<String> = Vec::new();
            let t: &str = &" ".repeat($t);
            $(
                v.push( format!("{}{}{}{}", &t, $name, " ".repeat($al - $name.len() - $t), $desc ) );
            )*
            println!("{}", v.join("\n"));
        }
    };
}



fn print_help() {
    let align: usize = 32;
    let tab: &str = &" ".repeat(4);

    println!("Thank you for using {APPNAME} v{VERSION}\n\nUSAGE:");
    printhelp!{align;
        "kfiles", "Run the program at the default directory";
        "kfiles <path>", "Run the program at the specified directory";
	    "kfiles [options ..]", "";
    };

    println!("\n\nOPTIONS:");
    printhelp!{align;
	    "--help, -h", "Show this message";
	    "--favorites, -f <query>", "Opens the program with the first result that matches <query> in your favorites";
    };

	println!("{tab}--config, --configs, -c, -cfg, --cfg");
	printhelp!(align; "", "Opens the configuration file");

    if let Ok(p) = confy::get_configuration_file_path(APPNAME, Some(CONFIG_PATH)) {
        println!("\n\nCONFIGS:\nYou can find your config file at: {}", p.display());
        printhelp!{align;
            "scroll_margin", "Minimum spacing between cursor and edge";
            "max_search_stack", "How \"deep\" to search in search panel";
            "max_recent_count", "How many directories to keep track of in the recent list";
            "favorites", "List of favorite directories";
            "default_dir", "Default directory when the program is run";
            "update_rate", "The frames per second to run the program at";
            "search_ignore_types", "The types of files to ignore while searching";
        };
        println!("{tab}{tab}E.g. \"import,txt\" will ignore all .import and .txt files\n");

        println!("{tab}THEME (all in RGB color values):");
        printhelp!{align;
            "folder_color", "Color for displaying folders";
            "file_color", "Color for displaying files";
            "special_color", "Color for special text";
            "bg_color", "App's background color";
            "text_color", "Color for normal text";
            "comment_color", "Color for dimmed text (comments)";
            "error_color", "Color for errors";
        };
    }

    println!("\n\nKEYBINDS:\n{tab}NAVIGATION:");
    printhelp!{align;
        "j or down arrow", "Move cursor down";
        "k or up arrow", "Move cursor up";
        "Ctrl-c or Alt-F4", "Exit the program";
        "Enter", "Open selected folder, file, or program";
        "` or Tab", "Search favorites (Esc or ` to cancel)";
        "/ or ;", "Quick search";
        "g and G", "Jump to the start and end of the list";
        "- or Backspace", "Go back";
        "u and d", "Jump up or down half a page";
    };

    println!("\n{tab}OTHER:");
    printhelp!{align;
        "Ctrl-o", "Search recent directories";
        "Ctrl-p", "Search files (Esc to cancel)";
        "Ctrl-Shift-p", "Search folders (Esc to cancel)";
        "Ctrl-f", "Toggle current directory as favorite";
        "Ctrl-e", "Reveal current directory in default file explorer";
        "Ctrl-Shift-e", "Reveal current directory in default file explorer and exit KFiles";
        "Ctrl-n", "Create file";
        "Ctrl-Shift-n", "Create folder";
        "Ctrl-d", "Delete file / folder";
        "Ctrl-r", "Rename file / folder";
    };

    println!("\n{tab}WHEN IN SEARCH PANEL:");
    printhelp!{align;
        "up and down arrows", "Move cursor";
        "Enter", "Open selected file/folder";
    };
}


#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::fs::File;
    use std::{io::Write, path::PathBuf};

    use crate::util::*;
    use crate::{get_favorites_list_path, APPNAME};

    #[test]
    fn test_path() {
        let config_path = confy::get_configuration_file_path(APPNAME, None).unwrap();
        dbg!(&config_path);

        let recent_path = config_path
            .parent()
            .and_then(|path| Some(path.with_file_name("recent.txt")));
        dbg!(&recent_path);
    }

    #[test]
    fn test_parse() {
        let path = confy::get_configuration_file_path(APPNAME, None)
            .unwrap()
            .parent()
            .and_then(|path| Some(path.with_file_name("recent.txt")))
            .unwrap();

        dbg!(&path);
    }

    #[test]
    fn test_save_single() {
        let path: PathBuf = PathBuf::from(r"C:\Users\ddxte\AppData\Roaming\kfiles");

        let bytes = path.as_path().to_str().unwrap().as_bytes();

        let mut file = File::create("foo.txt").unwrap();
        file.write_all(bytes).unwrap();
    }

    #[test]
    fn test_save_multiple() {
        let paths = vec![
            PathBuf::from(r"C:\Users\ddxte\AppData\Roaming\kfiles"),
            PathBuf::from(r"C:\Users\ddxte\Documents\Projects\TankInSands\Sounds"),
            PathBuf::from(r"C:\Users\ddxte\Documents\Apps\Office Chaos"),
        ];

        let bup = paths
            .iter()
            .map(|pathbuf| pathbuf.as_path().to_str())
            .flatten()
            .collect::<Vec<&str>>()
            .join("\n");

        let mut file = File::create("foo.txt").unwrap();
        file.write_all(bup.as_bytes()).unwrap();
    }

    #[test]
    fn test_search_seq() {
        let path: PathBuf = PathBuf::from(r"C:/Users/ddxte");
        let max_stack_size: usize = 1024;
        let mut stack: VecDeque<PathBuf> = VecDeque::from([ path ]);

        let query: &str = "sprites";

        while let Some(search_path) = stack.pop_front() {
            let Ok(folders) = get_all_folders_at(search_path) else { continue; };
            stack.append(&mut folders.iter()
                .filter(|pathbuf| !path2string(pathbuf.file_name().unwrap_or_default()) .starts_with('.') )
                .take(max_stack_size - stack.len())
                .cloned()
                .collect()
            );

            let folders: Vec<PathBuf> = folders.into_iter()
                .filter(|pathbuf| {
                    path2string(pathbuf).to_lowercase().contains(query)
                })
                .collect();

            if folders.is_empty() { continue; }
            println!("{:#?}", folders);
        }
    }

    #[test]
    fn test_search_threaded() {
        use threads_pool::*;
        use std::sync::{ Arc, Mutex };

        let pool = ThreadPool::new(4);
        let path: PathBuf = PathBuf::from(r"C:/Users/ddxte");
        let max_stack_size: usize = 1024;

        let stack = Arc::new(Mutex::new( VecDeque::from([ path ]) ));
        let query: &str = "sprites";

        loop {
            let Some(search_path) = stack.lock().unwrap() .pop_front().clone() else { // Stack is empty
                if Arc::strong_count(&stack) == 1 { break; } // 1, not 0, because of the declaration up there ^
                continue;
            };

            let stack = Arc::clone(&stack);
            pool.execute(move || {
                let Ok(folders) = get_all_folders_at(search_path) else { return; };

                let mut s = stack.lock().unwrap();
                let len: usize = s.len();
                s.append(&mut folders.iter()
                         .filter(|pathbuf| !path2string(pathbuf.file_name().unwrap_or_default()) .starts_with('.') )
                         .take(max_stack_size - len)
                         .cloned()
                         .collect()
                        );
                drop(s); // Unlock mutex

                let folders: Vec<PathBuf> = folders.into_iter()
                    .filter(|pathbuf| {
                        path2string(pathbuf).to_lowercase().contains(query)
                    })
                .collect();

                if folders.is_empty() { return; }
                println!("{:#?}", folders);
            }).unwrap();

        }
    }

}
