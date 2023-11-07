use std::path::PathBuf;
use std::env;

use confy::ConfyError;
use clean_path::Clean;

pub mod util;
pub mod app;
pub mod config;
pub mod filebuffer;
pub mod search;

use app::{ App, RunArg, AppError };


// Search panel offset from the edges of the screen
const SEARCH_PANEL_MARGIN: (u32, u32) = (4, 2);
const VERSION: &str = env!("CARGO_PKG_VERSION");
const APPNAME: &str = env!("CARGO_PKG_NAME");
const CONFIG_PATH: &str = "configs";


fn main() {
	// Load command line variables
	let mut args = env::args().skip(1);
	let run_args: RunArg = if let Some(a) = args.next() {
		match a.as_str() {
			"--help" | "-h" => {
				print_help();
				return;
			},

			"--favorites" | "-f" => {
				RunArg::TryFavorite(match args.next() {
					Some(arg) => arg,
					None => {
						println!("Error: Invalid syntax for --favorites. Expected field <query>\n Syntax: \t --favorites, -f <query>");
						return;
					},
				})
			},

			s => {
				let path: PathBuf = env::current_dir() .expect("Failed to get current working directory") .join(s);
				RunArg::AtPath( path.clean() )
			},
		}
	} else {
		RunArg::AtDefaultPath
	};


	// Setup app
	let mut app: App = match App::new(run_args) {
		Ok(app) => app,

		// Wrong config format
		Err(AppError::ConfigError(ConfyError::BadTomlData(err))) => {
			println!("Error while loading configs:\n    {}", err);
			if let Ok(p) = confy::get_configuration_file_path(APPNAME, Some(CONFIG_PATH)) {
				println!("You can find your config file at {}", p.display());
			}
			return;
		},

		// General config error
		Err(AppError::ConfigError(err)) => {
			println!("Error while loading configs:\n    {:?}", err);
			return;
		},
		
		// Engine init error
		Err(AppError::EngineError(err)) => {
			println!("Error creating console engine: {}", err);
			return;
		},
	};

	// MAIN LOOP ----------------------------------------------------------------------------
	loop {
		let state: app::AppState = app.run();

		if let app::AppState::Exit(exit_path) = state {
			if let Some(path) = exit_path {
				env::set_current_dir(path) .unwrap(); // TODO why no work. eh?
			}
			break;
		}

	}
}




fn print_help() {
	println!(r#"
{APPNAME} v{VERSION}

Usage:
	kfiles		Run the program at the default directory
	kfiles <path>		Run the program at the specified directory
	kfiles [options ..]

Options:
	--help, -h		Show this message
	--favorites, -f <query>		Opens the program with the first result that matches <query> in your favorites
"#);

	if let Ok(p) = confy::get_configuration_file_path(APPNAME, Some(CONFIG_PATH)) {
		println!(r#"
Configs:
You can find your config file at {}
	scroll_margin		Minimum spacing between cursor and edge
	max_search_stack	How "deep" to search in search panel
	favorites		List of favorite directories
	default_dir		Default directory when the program is run
	target_fps		The frames per second to run the program at
	search_ignore_types		The types of files to ignore while searching
		E.g. "import,txt" will ignore all .import and .txt files

	folder_color		The RGB color values for displaying folders
	file_color		The RGB color values for displaying files
	special_color		The RGB color values for displaying special text
	bg_color		The RGB color values for the background
"#, p.display());
	}

	println!(r#"
Keybinds:
	j, down arrow		Move cursor down
	k, up arrow		Move cursor up
	Ctrl-c or Alt-F4		Exit the program
	Enter		Open selected folder, file, or program
	`		Search favorites (Esc or ` to cancel)
	Ctrl-p		Search files (Esc to cancel)
	Ctrl-Shift-p		Search folders (Esc to cancel)
	Ctrl-f		Toggle current directory as favorite
	Ctrl-e		Reveal current directory in default file explorer
	Ctrl-Shift-e		Reveal current directory in default file explorer and exit KFiles

    When in search panel:
	up and down arrows		Move cursor
	Enter		Open selected file/folder
"#);
}


#[cfg(test)]
mod tests {
    use crate::APPNAME;

	#[test]
	fn test_path() {
		let config_path = confy::get_configuration_file_path(APPNAME, None);
		dbg!(&config_path);
	}
}

