# kfiles
A keybind-heavy 'OK' file explorer

<img width="674" alt="kfiles_screenshot" src="https://github.com/WhoStoleMyCoffee/kfiles/assets/79783809/ae7c54c6-d2d5-49cb-8244-46e84d7c3cfb">

## Motivation
I was once working on a project, doing my thing, and was looking for a folder stuffed *somewhere* in a directory I often used. So, I opened Windows file explorer and... was stuck waiting for *several seconds* for it to open. 

Now, I know waiting several seconds for an app to open isn't that big of a deal - In fact, I'd say I'm even one of the more patient people out there - but I do use Windows file explorer a lot, and using it often feels slow and sloppy (at least to me).

There were many features, big and small, that I just wished it had.

So I made my own.

It's obviously not perfect but it's good enough for me as a navigation tool; and it's certainly faster at booting up, and more configurable than Windows file explorer 😎

## Usage
You can run the .exe like a normal app, or through the terminal (if you set the PATH environment variable to it) with `kfiles`. 

E.g. `kfiles` (run at default directory), `kfiles .` (run at path), `kfiles --help` (run with options)

### Run options
`--help` or `h` Shows a help message

`--favorites <query>` or `-f <query>` Run the app with the first result that matches `<query>` in your favorites

### Keybinds
`j`, `down arrow` Move cursor down

`k`, `up arrow` Move cursor up

`Ctrl-c` Exit the program

`Enter` Open selected folder, file, or program

\` Search favorites (Esc or \` to cancel)

`Ctrl-p` Search files (Esc to cancel)

`Ctrl-Shift-p` Search folders (Esc to cancel)

`Ctrl-f` Toggle current directory as favorite

`Ctrl-e` Reveal current directory in default file explorer

When in search panel:
`up & down arrows` Move cursor

`Enter` Confirm

### Configs:
You can find your config file at your `AppData/Roaming/kfiles/config/configs.toml`

`scroll_margin` Minimum spacing between cursor and upper/lower edge

`max_search_stack` How "deep" to search in search panel

`favorites` List of favorite directories (you can re-order them here)

`default_dir` Default directory when the program is run

`target_fps` The frames per second to run the program at. It's not really significant but it could be nice sometimes to be able to configure it

`folder_color` The RGB color values for displaying folders

`file_color` The RGB color values for displaying files

`special_color` The RGB color values for displaying special text

`bg_color` The RGB color values for the background

## Features to come
... if I have the motivation..

- ignoring certain files and folders

- creating, renaming, and deleting files / folders (Yeah, I haven't implemented those yet 😬)
