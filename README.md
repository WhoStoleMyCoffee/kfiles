# kfiles v0.3.0
A keybind-heavy 'OK' file explorer

<img width="674" alt="kfiles_screenshot" src="https://github.com/WhoStoleMyCoffee/kfiles/assets/79783809/ae7c54c6-d2d5-49cb-8244-46e84d7c3cfb">

# Usage
You can run the .exe like a normal app, or through the terminal (if you set the PATH environment variable to it) with `kfiles`. 

E.g.
- `kfiles` (run at default directory)
- `kfiles .` (run here)
- `kfiles --help` (show help message)

## Run options
`--help` or `-h`: Show the help message

`--favorites <query>` or `-f <query>`: Run the app with the first result that matches `<query>` in your favorites

`<path>`: Run at the specified `<path>`

## Keybinds
- `j` or `down arrow`: Move cursor down
- `k` or `up arrow`: Move cursor up
- `g` or `G`: Jump to start or end of list
- `-`: Go back one level
- `Ctrl-c` or `Alt-F4`: Exit the program
- `Enter`: Open selected folder, file, or program
- \`: Search favorites (Esc or \` to cancel)
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

- `scroll_margin` Minimum spacing between cursor and upper/lower edge

- `max_search_stack` How "deep" to search in search panel

- `favorites` List of favorite directories (you can re-order them here)

- `default_dir` Default directory when the program is run

- `update_rate` The frames per second to run the program at. It's not really significant but it could be nice sometimes to be able to configure it

- `search_ignore_types`	The types of files to ignore while searching.
	E.g. "import,txt" will ignore all .import and .txt files

- `folder_color` The RGB color values for displaying folders

- `file_color` The RGB color values for displaying files

- `special_color` The RGB color values for displaying special text

- `bg_color` The RGB color values for the background


# Features to come
- Better movement options!
- More customizability!


# Motivation
I was once working on a project, doing my thing, and was looking for a folder stuffed _somewhere_ in a directory I often used. So, I opened Windows file explorer and... was stuck waiting for _a solid couple of seconds_ for it to open, then spent _several more_ just searching for that folder.

Now, I know waiting several seconds for an app to open isn't that big of a deal - In fact, I'd say I'm even one of the more patient people out there - but this happens to me a lot, and using it often feels slow and sometimes even frustrating (at least to me).

There were many features, big and small, that I just wished it had.

So I made my own.

It's obviously not perfect but it's certainly way faster to boot up (and to use), and more configurable than Windows file explorer 😎
