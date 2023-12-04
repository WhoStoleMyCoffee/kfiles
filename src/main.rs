use std::path::PathBuf;
use std::env;

use confy::ConfyError;
use clean_path::Clean;
use console_engine::KeyModifiers;

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
const RECENT_DIRS_FILE_NAME: &str = "recent.txt";

const CONTROL_SHIFT: u8 = KeyModifiers::CONTROL.union(KeyModifiers::SHIFT).bits();


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
                        pause(true);
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
			println!("Error while loading configs:\n	{}", err);
			if let Ok(p) = confy::get_configuration_file_path(APPNAME, Some(CONFIG_PATH)) {
				println!("Tip: You can find your config file at {}", p.display());
			}
            pause(true);
			return;
		},

		// General config error
		Err(AppError::ConfigError(err)) => {
			println!("Error while loading configs:\n	{}", err);
            pause(false);
			return;
		},
		
		// Engine init error
		Err(AppError::EngineError(err)) => {
			println!("Error creating console engine: {}", err);
            pause(false);
			return;
		},

		Err(AppError::OpenError(err)) => {
			println!("Error opening: {}", err);
            pause(false);
			return;
		},

		Err(AppError::Other(s)) => {
			println!("Error: {}", s);
            pause(false);
			return;
		},
	};

	// MAIN LOOP ----------------------------------------------------------------------------
	loop {
		let state: app::AppState = app.run();

		if let app::AppState::Exit(exit_path) = state {
			if let Some(path) = exit_path {
				let _ = env::set_current_dir(path); // TODO why no work. eh?
			}
			break;
		}

	}
}



fn get_recent_dirs_path() -> Result<PathBuf, AppError> {
	confy::get_configuration_file_path(APPNAME, None)
		.map_err(AppError::from)
		.unwrap()
		.parent()
		.map(|path| path.with_file_name(RECENT_DIRS_FILE_NAME))
		.ok_or("Failed to load recent directories".into())
}


fn pause(with_help_tip: bool) {
    if with_help_tip {
        println!("Tip: You can run KFiles with `kfiles --help` for more info");
    }
    println!("Press ENTER to continue...");
    let _ = std::io::stdin().read_line(&mut String::new());
}




fn print_help() {
	println!(r#"
Thank you for using {APPNAME} v{VERSION}

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
	max_recent_count	How many directories to keep track of in the recent list
	favorites		List of favorite directories
	default_dir		Default directory when the program is run
	update_rate		The frames per second to run the program at
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
	Navigation:
	j or down arrow		Move cursor down
	k or up arrow		Move cursor up
	Ctrl-c or Alt-F4		Exit the program
	Enter		Open selected folder, file, or program
	` or Tab		Search favorites (Esc or ` to cancel)
	/ or ;	  Quick search
	g and G	 Jump to the start and end of the list
	- or Backspace	  Go back

	Other:
	Ctrl-o		Search recent directories
	Ctrl-p		Search files (Esc to cancel)
	Ctrl-Shift-p		Search folders (Esc to cancel)
	Ctrl-f		Toggle current directory as favorite
	Ctrl-e		Reveal current directory in default file explorer
	Ctrl-Shift-e		Reveal current directory in default file explorer and exit KFiles
	Ctrl-n	  Create file
	Ctrl-Shift-n	  Create folder
	Ctrl-d	  Delete file / folder
	Ctrl-r	  Rename file / folder

	When in search panel:
	up and down arrows		Move cursor
	Enter		Open selected file/folder
"#);
}


#[cfg(test)]
mod tests {
	use std::{path::PathBuf, io::Write};
	use std::fs::File;

	use crate::APPNAME;

	#[test]
	fn test_path() {
		let config_path = confy::get_configuration_file_path(APPNAME, None) .unwrap();
		dbg!(&config_path);

		let recent_path = config_path.parent()
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

		let bytes = path.as_path().to_str() .unwrap() .as_bytes();
		
		let mut file = File::create("foo.txt") .unwrap();
		file.write_all(bytes) .unwrap();
	}

	#[test]
	fn test_save_multiple() {
		let paths = vec![
			PathBuf::from(r"C:\Users\ddxte\AppData\Roaming\kfiles"),
			PathBuf::from(r"C:\Users\ddxte\Documents\Projects\TankInSands\Sounds"),
			PathBuf::from(r"C:\Users\ddxte\Documents\Apps\Office Chaos"),
		];

		let bup = paths.iter()
			.map(|pathbuf| pathbuf.as_path().to_str() )
			.flatten()
			.collect::<Vec<&str>>()
			.join("\n");

		let mut file = File::create("foo.txt") .unwrap();
		file.write_all( bup.as_bytes() ) .unwrap();
	}

}

