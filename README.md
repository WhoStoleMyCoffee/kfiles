# kfiles v1.0.0
A keybind-heavy 'OK' file explorer written in Rust

<img width="674" alt="kfiles_screenshot" src="https://github.com/WhoStoleMyCoffee/kfiles/assets/79783809/ae7c54c6-d2d5-49cb-8244-46e84d7c3cfb">

*Keybind heavy? Does that mean I have to learn a new set of keybinds just for a file explorer?*

Mostly not!

KFiles' [keybinds](#keybinds) are inspired by VIM, so the learning curve is as smooth as possible if you're already familiar with it!

(Besides, you can quickly open the help message -- aka the *keybind cheat-sheet* -- at any point if you need to.)

# Why?
The aim of KFiles is to have a file explorer that is _fast_, _configurable_, and _lightweight_.

If you spend a lot of time jumping from directory to directory, looking around for files in the default file explorer you're not exactly satisfied with (looking at you, Windows File Explorer!), KFiles is for you.

If you wish your file explorer had color theme and performance (yes, performance) options, KFiles is also for you.

# Features

1. **Mouse support**

	KFiles does offer mouse support, although somewhat limited because it kind of defeats the whole purpose of it being keybind-heavy

1. **Quick search**
Enter quick search mode via `/` or `;` (`Esc` to exit), where you can \*quickly* \*search* through the files and folders of the current directory.

1. **Deeper search through files**

	It is also possible to make a deeper search for files and folders via `Ctrl-p` (for files) and `Ctrl-Shift-p` (for folders).

	The searching is multi-threaded with [a configurable number of threads](#performance-options) if you're low on resources.


1. **Favorites and recently visited directories list**

1. **Creating, renaming, and deleting files and folders**

1. **Customizability!**

	KFiles is pretty [customizable](#configs), with more configuration options coming soon!

1. **Run in the terminal**

	If you configure the environment variable to it, you can also [run it in the terminal](#run-options).

# Usage
You can run the .exe like a normal app, or through the terminal (if you set the PATH environment variable to it) with `kfiles`. 

E.g.
- `kfiles` (run at default directory)
- `kfiles .` (run here)
- `kfiles --help` (show help message)

> Tip: KFiles is a standalone executable, so you can even change the executable's name to fit your needs. For example, changing it from `kfiles.exe` to `kf.exe` to make it quicker to type on the command line.

## Run options
(for command line users)

`--help` or `-h`: Show the help message

`--favorites <query>` or `-f <query>`: Run the app with the first result that matches `<query>` in your favorites

`--config`, `--configs`, `-c`, `-cfg`, or `--cfg`: Opens the configuration file

`<path>`: Run at the specified `<path>`

## Keybinds
- `j` or `down arrow`: Move cursor down
- `k` or `up arrow`: Move cursor up
- `g` or `G`: Jump to start or end of list
- `-` or `Backspace`: Go back one level
- `u and d` or `Page up and Page down`: Jump up or down half a page

- `Ctrl-c` or `Alt-F4`: Exit the program
- `Enter`: Open selected folder, file, or program
- \` or `Tab`: Search favorites (Esc or \` or `Tab` again to cancel)
- `/` or `;`: Enter quick search
- `F1`: Show help message
- `Ctrl-p`: Search files (Esc to cancel)
- `Ctrl-Shift-p`: Search folders (Esc to cancel)
- `Ctrl-f`: Toggle current directory as favorite
- `Ctrl-e`: Reveal current directory in default file explorer
- `Ctrl-Shift-e`: Reveal current directory in default file explorer and exit KFiles
- `Ctrl-o`: Search recent directories
- `Ctrl-n`: Create new file
- `Ctrl-Shift-n`: Create new folder
- `Ctrl-d`: Delete selected file / folder
- `Ctrl-r`: Rename selected file / folder

### When in search panel:
- `up & down arrows`: Move cursor
- `Enter`: Confirm

## Configs:
You can find your config file at your `AppData/Roaming/kfiles/config/configs.toml`

- `scroll_margin`: Minimum spacing between cursor and edge of the window
- `max_recent_count`: How many directories to keep track of in the recent list
- `favorites`: List of favorite directories (you can re-order them here)
- `default_path`: Default directory when the program is run
- `search_ignore_types`: The types of files to ignore while searching.
	
	E.g. "import,txt" will ignore all .import and .txt files

### Performance options:
- `update_rate`: The frames per second to run the program at. It's not really significant but it could be nice sometimes to be able to configure it
- `max_search_queue_len`: How "deep" to search in search panel
- `search_thread_count`: How many threads to use while searching


### Theme options:
(all in RGB color values)
- `folder_color`: Color for displaying folders
- `file_color`: Color for displaying files
- `special_color`: Color for special text
- `bg_color`: App's background color
- `text_color`: Color for normal text
- `comment_color`: Color for dimmed text (comments)
- `error_color`: Color for errors


# Features to come
- Customizable keybinds (soon!)
- More color theming options, including presets!
