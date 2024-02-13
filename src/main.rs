use std::{env, error::Error};
use std::fmt::Display;
use std::path::PathBuf;
use std::sync::OnceLock;

use clean_path::Clean;
use config::{Configs, FavoritesList};
use confy::ConfyError;
use console_engine::{ KeyModifiers, Color };
use dialoguer::{ Confirm, theme::ColorfulTheme };

pub mod app;
pub mod config;
pub mod filebuffer;
pub mod search;
pub mod util;

use app::{ App, AppState };
use help::print_help;

use crate::help::print_help_tip;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const APPNAME: &str = env!("CARGO_PKG_NAME");
const CONFIG_PATH: &str = "configs";
const RECENT_DIRS_FILE_NAME: &str = "recent.txt";
const FAVORITES_LIST_FILE_NAME: &str = "favorites.txt";

/// Search panel offset from the edges of the screen
const SEARCH_PANEL_MARGIN: (u32, u32) = (4, 2);

const CONTROL_SHIFT: u8 = KeyModifiers::CONTROL.union(KeyModifiers::SHIFT).bits();

static CONFIGS: OnceLock<Configs> = OnceLock::new();


macro_rules! cmdline {
    ($app:expr, $b:block) => {
        $app.engine = None;
        $b
        $app.engine = Some( App::init_engine()? );
    };
}




#[derive(Debug)]
pub enum AppError {
    ConfigError(confy::ConfyError),
    IO(std::io::Error),
    OpenError {
        source: opener::OpenError,
        path: PathBuf,
    },
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

impl From<String> for AppError {
    fn from(value: String) -> Self {
        Self::Other(value)
    }
}

impl Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConfigError(err) => err.fmt(f),
            Self::IO(err) => err.fmt(f),
            Self::OpenError{ source, path } => {
                write!(f, "{:?}\n while opening {}", source, path.display())
            },
            Self::Other(str) => write!(f, "{}", str),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ConfigError(err) => Some(err),
            Self::IO(err) => Some(err),
            Self::OpenError { source, .. } => Some(source),
            _ => None,
        }
    }
}

pub enum RunOption {
    AtPath(PathBuf),
    TryFavorite(String),
    AtDefaultPath,
    Help,
    Config,
}


#[derive(Debug, Clone, Copy)]
pub enum Action {
    Exit,
    Help,
    ToggleFavorite,
    OpenConfigFile,
    OpenConfigFolder,
    ClearRecent,
    AddToRecent,
}

impl Action {
    const fn display_list() -> &'static [Self] {
        &[
            Self::Exit,
            Self::Help,
            Self::ToggleFavorite,
            Self::OpenConfigFile,
            Self::OpenConfigFolder,
            Self::AddToRecent,
            Self::ClearRecent,
        ]
    }
}

impl ToString for Action {
    fn to_string(&self) -> String {
        use Action as A;
        match self {
            A::Exit => "Exit KFiles",
            A::Help => "Help",
            A::ToggleFavorite => "Toggle favorites",
            A::OpenConfigFile => "Open configuration file",
            A::OpenConfigFolder => "Open configuration folder",
            A::ClearRecent => "Clear recent list",
            A::AddToRecent => "Add current directory to recent list",
        }.to_string()
    }
}







fn main() {
    let cfg: Result<Configs, AppError> = confy::load(APPNAME, Some(CONFIG_PATH)).map_err(AppError::from);

    // Process command line arguments
    let (run_path, cfg): (PathBuf, Configs) = match (parse_cli(env::args()), cfg) {
        // We don't care about the configs when showing help message
        (Ok(RunOption::Help), _) => {
            help::print_help();
            pause!();
            return;
        }

        (Ok(RunOption::Config), _) => {
            match help::open_config_file() {
                Ok(_) => { return; },
                Err(AppError::ConfigError(err)) => {
                    println!("Failed to get configuration file path: {}", err);
                    print_help_tip();
                },
                Err(err @ AppError::OpenError { .. }) => println!("Failed to open configuration file: {}", err),
                Err(err) => println!("Error: {}", err),
            }
            pause!();
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
                        pause!();
                        return;
                    }
                };

                let list = FavoritesList::load(&list_path).unwrap_or_default();
                (
                    list.query(&query).unwrap_or(&cfg.default_path).clone(),
                    cfg
                )
            }

            RunOption::Help | RunOption::Config => unreachable!(), // Already checked up there
        },
    };

    if CONFIGS.set(cfg).is_err() {
        println!("Error: Failed to set global configs (somehow?)");
        pause!();
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
        let state: &AppState = app.run();
        match state {
            AppState::Running => {},
            AppState::Exit(_exit_path) => {
                break;
            },

            AppState::Action(action) => {
                match handle_action(*action, &mut app) {
                    Ok(_) => { },
                    Err(err) => {
                        handle_app_err(err);
                        return;
                    }
                }
            },

        }
    }

}



#[allow(unreachable_patterns)]
fn handle_action(action: Action, app: &mut App) -> Result<(), AppError> {
    match action {
        Action::Exit => {
            app.exit();
            return Ok(());
        },

        Action::Help => {
            cmdline!(app, {
                print_help();
                pause!();
            });
        },

        Action::OpenConfigFile => {
            cmdline!(app, {
                match help::open_config_file() {
                    Ok(_) => help::print_config_help(),
                    Err(AppError::ConfigError(err)) => {
                        println!("Failed to get configuration file path:\n {}", err);
                        print_help_tip();
                    },
                    Err(err @ AppError::OpenError { .. }) => println!("Failed to open configuration file:\n {}", err),
                    Err(err) => println!("Error:\n {}", err),
                }
                pause!();
            });
        },

        Action::OpenConfigFolder => {
            if let Err(err) = help::reveal_config_folder() {
                cmdline!(app, {
                    match err {
                        AppError::ConfigError(err) => {
                            println!("Failed to get configuration file path:\n {}", err);
                            print_help_tip();
                        },
                        err @ AppError::OpenError { .. } => {
                            println!("Failed to reveal configuration folder:\n {}", err);
                        },
                        err => println!("Error:\n {}", err),
                    }
                    pause!();
                });
            }
        },

        Action::ToggleFavorite => {
            match app.toggle_current_as_favorite() {
                Err(err) => {
                    app.status_line_mut()
                        .error(err, Some("Error saving configs: \n"));
                },
                Ok(true) => {
                    app.status_line_mut().normal()
                        .set_text("Added path to favorites")
                        .set_color(themevar!(special_color));
                },
                Ok(false) => {
                    app.status_line_mut().normal()
                        .set_text("Removed path from favorites")
                        .set_color(themevar!(special_color));
                },
            }
        },

        Action::ClearRecent => {
            app.clear_recent_list();
            app.status_line_mut()
                .normal()
                .set_text("Cleared recent list")
                .set_color( themevar!(special_color) );
        },

        Action::AddToRecent => {
            app.add_current_to_recent();
            app.status_line_mut()
                .normal()
                .set_text("Added current directory to recent list (Ctrl-o)")
                .set_color( themevar!(special_color) );
        },

        _ => {
            app.status_line_mut()
                .error(format!("Unimplemented action: {:?}", action).into(), None);
        },
    }

    app.state = AppState::Running;
    Ok(())
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

/// Pauses automatically (via `pause!()`)
fn handle_app_err(err: AppError) {
    use help::*;

    match err {
        // Wrong config format
        AppError::ConfigError(ConfyError::BadTomlData(err)) => {
            println!("Error while loading configs:\n	{}", err);
            if confy::get_configuration_file_path(APPNAME, Some(CONFIG_PATH)).is_err() {
                print_help_tip();
                pause!();
                return;
            };

            let do_open_cfg = Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt("Do you wish to open the configuration file?")
                .default(true)
                .interact();

            match do_open_cfg {
                Ok(true) => {
                    match open_config_file() {
                        Ok(_) => help::print_config_help(),
                        Err(err) => println!("Error: {}", err),
                    }
                },
                Ok(false) => print_help_tip(),
                Err(err) => println!("Error: {}", err),
            }
        }

        // General config error
        AppError::ConfigError(err) => {
            println!("Config error:\n	{}", err);
        }

        // Engine init error
        AppError::IO(err) => {
            println!("IO error: {}", err);
        }

        err @ AppError::OpenError { .. } => {
            println!("Error opening: {}", err);
        }

        AppError::Other(s) => {
            println!("Error: {}", s);
        }
    }
    pause!();
}




#[macro_use]
mod help {
    use crate::{ CONFIG_PATH, APPNAME, VERSION, AppError };
    use crate::pause;

    const ALIGN: usize = 32;
    const TAB: &str = "    ";
    /// Artificial delay when printing help message
    /// The aim is to have the user subconsciously know that there is more than one screen worth of
    /// help text
    const DELAY: u64 = 100;

    pub fn open_config_file() -> Result<(), AppError> {
        let cfg_path = confy::get_configuration_file_path(APPNAME, Some(CONFIG_PATH))?;

        if opener::open(&cfg_path).is_err() {
            opener::reveal(&cfg_path)
                .map_err(|source| AppError::OpenError { source, path: cfg_path })?;
        }

        Ok(())
    }

    pub fn reveal_config_folder() -> Result<(), AppError> {
        let cfg_path = confy::get_configuration_file_path(APPNAME, Some(CONFIG_PATH))?;
        opener::reveal(&cfg_path)
            .map_err(|source| AppError::OpenError { source, path: cfg_path })?;
        Ok(())
    }



    #[inline]
    pub fn print_help_tip() {
        println!("Tip: You can run KFiles with `kfiles --help` for more info");
    }



    #[macro_export]
    macro_rules! pause {
        () => {
            println!("Press ENTER to continue...");
            let _ = std::io::stdin().read_line(&mut String::new());
        };

        ($t:expr) => {
            std::thread::sleep( std::time::Duration::from_millis($t) );
        }
    }


    macro_rules! printtabbed {
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



    pub fn print_help() {
        println!("Thank you for using {APPNAME} v{VERSION}\n\nUSAGE:");
        pause!(DELAY);
        printtabbed!{ALIGN;
            "kfiles", "Run the program at the default directory";
            "kfiles <path>", "Run the program at the specified directory";
            "kfiles [options ..]", "";
        };

        pause!(DELAY);
        println!("\n\nOPTIONS:");
        printtabbed!{ALIGN;
            "--help, -h", "Show this message";
            "--favorites, -f <query>", "Opens the program with the first result that matches <query> in your favorites";
        };
        println!("{TAB}--config, --configs, -c, -cfg, --cfg");
        printtabbed!(ALIGN; "", "Opens the configuration file");

        pause!(DELAY);
        print_config_help();

        // Show keybinds at the bottom so it's the first thing the user sees
        // when they summon the almighty help message
        pause!(DELAY);
        print_keybind_help();
    }

    pub fn print_keybind_help() {
        println!("\n\nKEYBINDS:\n{TAB}NAVIGATION:");
        printtabbed!{ALIGN;
            "j or down arrow", "Move cursor down";
            "k or up arrow", "Move cursor up";
            "Ctrl-c or Alt-F4", "Exit the program";
            "Enter", "Open selected folder, file, or program";
            "` or Tab", "Search favorites (Esc or ` to cancel)";
            "/ or ;", "Quick search";
            "g and G", "Jump to the start and end of the list";
            "u and d", "Jump up or down half a page";
        };
        println!("{TAB}- or Backspace or Alt-up arrow");
        printtabbed!(ALIGN; "", "Go back");

        pause!(DELAY);
        println!("\n{TAB}OTHER:");
        printtabbed!{ALIGN;
            "F1", "Command palette";
            "F5", "Refresh";
            "Ctrl-o", "Search recent directories";
            "Ctrl-p", "Search files";
            "Ctrl-Shift-p", "Search folders";
            "Ctrl-e", "Reveal current directory in default file explorer";
            "Ctrl-Shift-e", "Reveal current directory in default file explorer and exit KFiles";
            "Ctrl-n", "Create file";
            "Ctrl-Shift-n", "Create folder";
            "Ctrl-d", "Delete file / folder";
            "Ctrl-r", "Rename file / folder";
        };

        pause!(DELAY);
        println!("\n{TAB}WHEN IN SEARCH PANEL:");
        printtabbed!{ALIGN;
            "up and down arrows", "Move cursor";
            "Enter", "Open selected file/folder";
            "Ctrl-Backspace", "Clear prompt";
        };
    }
    
    /// Prints the CONFIGS chapter of the help message
    pub fn print_config_help() {
        let p = match confy::get_configuration_file_path(APPNAME, Some(CONFIG_PATH)) {
            Ok(p) => p,
            Err(err) => {
                println!("\n\nCONFIGS:");
                println!("Error: Failed to get configuration file path: \n\t{}", err);
                return;
            },
        };
        println!("\n\nCONFIGS:\nYou can find your config file at: {}", p.display());
        println!("or run with --config to open it");
        printtabbed!{ALIGN;
            "scroll_margin", "Minimum spacing between cursor and edge of the window";
            "default_path", "Default directory when the program is run";
            "search_ignore_types", "The types of files to ignore while searching";
            "", "E.g. [\"import\" ,\"txt\"] will ignore all .import and .txt files";
            "max_recent_count", "How many directories to keep track of in the recent list";
        };

        println!("\n{TAB}THEME (all in RGB color values):");
        printtabbed!{ALIGN;
            "folder_color", "Color for displaying folders";
            "file_color", "Color for displaying files";
            "special_color", "Color for special text";
            "bg_color", "App's background color";
            "text_color", "Color for normal text";
            "comment_color", "Color for dimmed text (comments)";
            "error_color", "Color for errors";
        };

        println!("\n{TAB}PERFORMANCE OPTIONS:");
        println!("{TAB}These settings mosly affect the behavior in the search panel");
        printtabbed!(ALIGN;
            "update_rate", "The FPS (frames per second) to run the program at";
            "max_search_queue_len", "(Optional; default = unlimited) How long to allow the queue to be when searching";
            "", "This setting may allow you to save memory, but if set *too* low, it may cause it to skip some directories when searching";
            "search_thread_count", "How many threads to use while searching";
            "thread_active_ms", "How fast active search threads should be working";
            "", "This option is to prevent \"hyperactive threads\" over-using CPU resources";
            "thread_inactive_ms", "How fast inactive search threads should be working";
            "", "This option is to prevent \"hyperactive threads\" over-using CPU resources";
        );

        println!("Tip: you can delete your `configs.toml` file to reset all settings");
    }
}




#[cfg(test)]
mod tests {
    use std::{path::Path, process::Command};

    use crate::{util, APPNAME};

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
    fn test_strmatch() {
        let haystack = "It is always bup time";
        let needle = "alwbu";

        // For every char nch in needle
        let mut cost: usize = 0;
        let mut h_chars = haystack.chars();
        for nch in needle.chars() {
            // Look for nch in haystack
            let Some(i) = h_chars.position(|ch| ch == nch) else {
                break;
            };
            cost += i;
        }

        println!("Cost: {cost}");
    }

    #[test]
    fn test_strmatch_multiple() {
        let haystacks = [
            "It is always bup time",
            "Never gonna give you up",
            "Never gonna let you down",
            "Continuously crawling from the mouth of the abyss",
        ];
        let needle = "le";
        let search_fn = util::str_match_cost;

        println!("Searching needle = {needle}");

        let res1: Vec<(usize, usize)> = haystacks.iter().enumerate()
            .filter_map(|(i, str)| search_fn(needle, str) .map(|cost| (i, cost)) )
            .collect();

        for (i, cost) in res1.iter() {
            println!("cost = {cost} \t str = {}", haystacks[*i]);
        }

        /* for haystack in haystacks.iter() {
            let cost = search_fn(needle, haystack);
            dbg!(cost);
        } */

    }

    #[test]
    fn test_cd() {
        let path = &Path::new("C:/Users/ddxte/Documents/Projects/kfiles");
        // std::env::set_current_dir(path) .unwrap();

        Command::new("cd")
            .arg("C:/Users/ddxte/Documents/Projects")
            .spawn()
            .unwrap();
    }

}
