use std::path::{Path, PathBuf};

use console_engine::pixel;
use console_engine::screen::Screen;
use console_engine::{crossterm::event::KeyEvent, events::Event};
use console_engine::{Color, ConsoleEngine, KeyCode, KeyEventKind, KeyModifiers};

use crate::config::{ColorTheme, Configs, FavoritesList, RecentList, PerformanceConfigs};
use crate::filebuffer::{FileBuffer, StatusLine};
use crate::search::{
    SelectPanel,
    SelectPanelState,
    query,
};
use crate::util::start_terminal;
use crate::try_err;
use crate::{
    get_favorites_list_path, get_recent_dirs_path, themevar,
    SEARCH_PANEL_MARGIN,
    Action,
};

use crate::{AppError, CONTROL_SHIFT};


macro_rules! screen_size {
    ($engine:expr) => {
        ($engine.get_width(), $engine.get_height())
    };
}


macro_rules! select_panel {
    ($sw:expr, $sh:expr, $q:expr) => {
        SelectPanel::new(
            $sw - SEARCH_PANEL_MARGIN.0 * 2,
            $sh - SEARCH_PANEL_MARGIN.1 * 2,
            Box::new($q),
        )
    };
}


#[derive(Debug, PartialEq, Eq)]
pub enum SelectPanelMode {
    Help,
    Recent,
    Favorites,
    SearchFiles,
    SearchFolders,
}



pub enum AppState {
    Running,
    Exit(Option<PathBuf>),
    Action(Action),
}

pub struct App {
    pub engine: Option<ConsoleEngine>,
    file_buffer: FileBuffer,
    select_panel: Option<(SelectPanelMode, SelectPanel)>,
    recent_dirs: RecentList,
    favorites: FavoritesList,
    pub state: AppState,
}

impl App {
    pub fn new(at_path: &Path) -> Result<Self, AppError> {
        let cfg: &Configs = Configs::global();
        let engine: ConsoleEngine = App::init_engine()?;

        // Initialize file buffer
        let (w, h) = FileBuffer::calc_size_from_engine(&engine);
        let mut file_buffer = FileBuffer::new(at_path, Screen::new(w, h));

        try_err!( file_buffer.load_entries() => file_buffer );

        let max_recent_count: usize = cfg.max_recent_count;
        let mut app = Self {
            engine: Some(engine),
            file_buffer,
            select_panel: None,
            recent_dirs: RecentList::load(&get_recent_dirs_path()?, max_recent_count)
                .unwrap_or_else(|_| RecentList::new(max_recent_count)),
            favorites: FavoritesList::load(&get_favorites_list_path()?)
                .unwrap_or_default(),
            state: AppState::Running,
        };

        app.set_title_to_current();
        Ok(app)
    }

    pub fn init_engine() -> std::io::Result<ConsoleEngine> {
        ConsoleEngine::init_fill(Configs::performance().update_rate)
    }

    pub fn run(&mut self) -> &AppState {
        if let AppState::Running = self.state {} else {
            return &self.state;
        }



        if let Some((_, panel)) = &mut self.select_panel {
            if panel.is_running() {
                panel.update();
            }
        }

        let Some(engine) = &mut self.engine else {
            return &self.state;
        };
        let bg_color: Color = themevar!(bg_color);

        match engine.poll() {
            Event::Frame => {
                if let Some((_, panel)) = &mut self.select_panel {
                    engine.print_screen(
                        SEARCH_PANEL_MARGIN.0 as i32,
                        SEARCH_PANEL_MARGIN.1 as i32,
                        panel.draw((engine.frame_count % 8 > 3) as usize),
                    );
                } else {
                    engine.fill(pixel::pxl_bg(' ', bg_color));
                    engine.print_screen(1, 1, self.file_buffer.draw());
                }

                engine.print_fbg(
                    0,
                    0,
                    "Press F1 to show help message, Ctrl-c or Alt-F4 to exit",
                    themevar!(comment_color),
                    bg_color,
                );
                self.file_buffer.status_line.draw(engine);

                engine.draw();
            }

            Event::Resize(w, h) => {
                engine.resize(w as u32, h as u32);
                let (w, h) = FileBuffer::calc_size_from_engine(engine);
                self.file_buffer.resize(w, h);
                self.file_buffer.display_path();
            }

            Event::Mouse(mouse_event) => {
                if let Some((_, panel)) = &mut self.select_panel {
                    panel.handle_mouse_event(mouse_event, SEARCH_PANEL_MARGIN.1 as u16);
                } else {
                    self.file_buffer.handle_mouse_event(mouse_event);
                }
            }

            Event::Key(key_event) => {
                self.handle_key_event(key_event);
                return &self.state;
            }

        }

        self.state = AppState::Running;
        &self.state
    }

    pub fn exit(&mut self) {
        self.state = AppState::Exit( Some(self.file_buffer.path.clone()) );
    }

    fn handle_key_event(&mut self, event: KeyEvent) {
        if self.engine.is_none() { return; }

        match event {
            // Exit with Alt-F4
            KeyEvent {
                code: KeyCode::F(4),
                kind: KeyEventKind::Press,
                modifiers: KeyModifiers::ALT,
                ..
            } => {
                self.exit();
            }

            // Exit with Ctrl-c
            // TODO remove?
            KeyEvent {
                code: KeyCode::Char('c'),
                kind: KeyEventKind::Press,
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.exit();
            }

            // Reveal in file explorer and close with Ctrl-Shift-e
            KeyEvent {
                code: KeyCode::Char('E'),
                kind: KeyEventKind::Press,
                modifiers,
                ..
            } if modifiers.bits() == CONTROL_SHIFT => {
                self.file_buffer
                    .reveal()
                    .expect("Failed to reveal current directory");
                
                // self.add_current_to_recent(); // Gets called in drop() anyways (See impl Drop for App)
                self.state = AppState::Exit(None);
            }

            // Reveal in file explorer with Ctrl-e
            KeyEvent {
                code: KeyCode::Char('e'),
                kind: KeyEventKind::Press,
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.file_buffer
                    .reveal()
                    .expect("Failed to reveal current directory");
                self.add_current_to_recent();
            },

            // Search folders with Ctrl-Shift-p
            KeyEvent {
                code: KeyCode::Char('P'),
                kind: KeyEventKind::Press,
                modifiers,
                ..
            } if modifiers.bits() == CONTROL_SHIFT => {
                // If already open, close
                if let Some((SelectPanelMode::SearchFolders, _)) = &self.select_panel {
                    self.select_panel = None;
                    self.state = AppState::Running;
                    return;
                }

                let (sw, sh) = screen_size!(self.engine.as_ref().unwrap());
                let perf: &PerformanceConfigs = Configs::performance();
                let app = self as *mut App;
                let root_path = self.file_buffer.path .clone();
                let panel: SelectPanel = select_panel!(sw, sh,
                        query::SearchFolders::new(&self.file_buffer.path)
                            .with_queue_len(perf.max_search_queue_len)
                            .with_threads(perf.search_thread_count)
                            .with_max_results( SelectPanel::calc_list_height(sh) as usize )
                            .list()
                    )
                    .with_title("Search Folders")
                    .with_color(themevar!(folder_color))
                    .on_selected(move |s| {
                        let path: PathBuf = root_path.join( Path::new(s.as_ref()) );
                        let app = unsafe { &mut *app };
                        app.add_current_to_recent();
                        app.file_buffer.set_path(&path);
                    });
                self.select_panel = Some((SelectPanelMode::SearchFolders, panel));
            },

            // Search files with Ctrl-p
            KeyEvent {
                code: KeyCode::Char('p'),
                kind: KeyEventKind::Press,
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                // If already open, close
                if let Some((SelectPanelMode::SearchFiles, _)) = &self.select_panel {
                    self.select_panel = None;
                    self.state = AppState::Running;
                    return;
                }

                let (sw, sh) = screen_size!(self.engine.as_ref().unwrap());
                let cfg: &Configs = Configs::global();
                let app = self as *mut App;
                let root_path = self.file_buffer.path.clone();
                let panel: SelectPanel = select_panel!(sw, sh,
                        query::SearchFiles::new(&self.file_buffer.path)
                            .with_queue_len(cfg.performance .max_search_queue_len)
                            .with_threads(cfg.performance .search_thread_count)
                            .with_max_results( SelectPanel::calc_list_height(sh) as usize )
                            .ignore_extensions( &cfg.search_ignore_extensions )
                            .list()
                    )
                    .with_title("Search Files")
                    .with_color(themevar!(file_color))
                    .on_selected(move |s| {
                        let path: PathBuf = root_path.join( Path::new(s.as_ref()) );
                        let app = unsafe { &mut *app };
                        app.add_current_to_recent();
                        app.file_buffer.set_path(&path);
                    });
                self.select_panel = Some((SelectPanelMode::SearchFiles, panel));
            },

            // Recent files with Ctrl-o
            KeyEvent {
                code: KeyCode::Char('o'),
                kind: KeyEventKind::Press,
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                // If already open, close
                if let Some((SelectPanelMode::Recent, _)) = &self.select_panel {
                    self.select_panel = None;
                    self.state = AppState::Running;
                    return;
                }

                let (sw, sh) = screen_size!(self.engine.as_ref().unwrap());
                let app = self as *mut App;
                let panel: SelectPanel = select_panel!(sw, sh,
                    query::SearchPathList::new(&self.recent_dirs)
                    )
                    .with_title("Recent")
                    .with_color(themevar!(folder_color))
                    .on_selected(move |s| {
                        let path: &Path = Path::new(s.as_ref());
                        let app = unsafe { &mut *app };
                        app.add_current_to_recent();
                        app.file_buffer.set_path(path);
                    });

                self.select_panel = Some((SelectPanelMode::Recent, panel));
            },

            // Open / close favorites with ` or Tab
            KeyEvent {
                code: KeyCode::Char('`') | KeyCode::Tab,
                kind: KeyEventKind::Press,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // If already open, close
                if let Some((SelectPanelMode::Favorites, _)) = &self.select_panel {
                    self.select_panel = None;
                    self.state = AppState::Running;
                    return;
                }

                let (sw, sh) = screen_size!(self.engine.as_ref().unwrap());
                let app = self as *mut App;
                let panel: SelectPanel = select_panel!(sw, sh,
                     query::SearchPathList::new(&self.favorites)
                     )
                    .with_title("Favorites")
                    .with_color(themevar!(special_color))
                    .on_selected(move |s| {
                        let path: &Path = Path::new(s.as_ref());
                        let app = unsafe { &mut *app };
                        app.add_current_to_recent();
                        app.file_buffer.set_path(path);
                    });
                self.select_panel = Some((SelectPanelMode::Favorites, panel));
            },

            // Open help list with F1
            KeyEvent {
                code: KeyCode::F(1),
                kind: KeyEventKind::Press,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // If already open, close
                if let Some((SelectPanelMode::Help, _)) = &self.select_panel {
                    self.select_panel = None;
                    self.state = AppState::Running;
                    return;
                }

                let (sw, sh) = screen_size!(self.engine.as_ref().unwrap());
                let app = self as *mut App;
                let panel: SelectPanel = select_panel!(sw, sh,
                    query::SearchList::new( Action::display_list() )
                    )
                    .with_title("Command Palette")
                    .with_color(themevar!(special_color))
                    .on_selected(move |is| {
                        let app = unsafe { &mut *app };
                        let Some(action) = Action::display_list().get( is.index() ) else {
                            app.file_buffer.status_line.error("Could not find specified action".into(), None);
                            return;
                        };

                        app.state = AppState::Action(*action);
                    });
                self.select_panel = Some((SelectPanelMode::Help, panel));
            }

            // Reload with F5
            KeyEvent {
                code: KeyCode::F(5),
                kind: KeyEventKind::Press,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // Reload entries
                try_err!(self.file_buffer.load_entries() => self.file_buffer; else {
                    self.file_buffer.display_path();
                    self.file_buffer.update_scroll();
                });
            },

            // Open terminal with Ctrl-t
            KeyEvent {
                code: KeyCode::Char('t'),
                kind: KeyEventKind::Press,
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                try_err!( start_terminal().map_err(AppError::from) => self.file_buffer );
            },

            key_event => {
                // Try to update search panel first
                if let Some((_, panel)) = &mut self.select_panel {
                    panel.handle_key_event(event);
                    if panel.state == SelectPanelState::Exit {
                        self.set_title_to_current();
                        self.select_panel = None;
                    }

                // File buffer
                } else {
                    let prev_path: &Path = &self.get_path().clone();
                    self.file_buffer.handle_key_event(key_event);

                    if self.get_path() != prev_path {
                        self.set_title_to_current();
                    }

                }
            }
        }
    }

    pub fn get_path(&self) -> &PathBuf {
        &self.file_buffer.path
    }

    pub fn status_line_mut(&mut self) -> &mut StatusLine {
        &mut self.file_buffer.status_line
    }

    /// Sets the window's title to `title`
    pub fn set_title(&mut self, title: &str) {
        if let Some(engine) = &mut self.engine {
            engine.set_title( &format!("KFiles | {}", title) );
        }
    }

    /// Sets the window's title using the current dir
    pub fn set_title_to_current(&mut self) {
        let Some(pathname) = self.file_buffer.path.file_name().and_then(|osstr| osstr.to_str()) else {
            return;
        };
        let Some(engine) = &mut self.engine else {
            return;
        };
        engine.set_title( &format!("KFiles | {}", pathname) );
    }

    pub fn add_to_recent(&mut self, path: PathBuf) {
        self.recent_dirs.push(path);
    }

    pub fn add_current_to_recent(&mut self) {
        self.add_to_recent(self.file_buffer.path.clone());
    }

    pub fn clear_recent_list(&mut self) {
        self.recent_dirs.clear();
    }

    pub fn toggle_current_as_favorite(&mut self) -> Result<bool, AppError> {
        let added: bool = self.favorites.toggle(&self.file_buffer.path);
        self.favorites.save( &get_favorites_list_path()? )?;

        Ok(added)
    }
}

impl Drop for App {
    fn drop(&mut self) {
        let recent_res = get_recent_dirs_path().and_then(|path| {
            self.add_current_to_recent();
            self.recent_dirs.save(&path).map_err(AppError::from)
        });

        let favorites_res = get_favorites_list_path()
            .and_then(|path| self.favorites.save(&path).map_err(AppError::from));

        // FYI: guard clause or whatever it's called
        if recent_res.is_ok() && favorites_res.is_ok() {
            return;
        }

        let Some(engine) = &mut self.engine else {
            return;
        };

        let theme: &ColorTheme = Configs::theme();
        let bg_color: Color = theme.bg_color.into();
        let mut y: i32 = 0;

        engine.fill(pixel::pxl_bg(' ', bg_color));

        if let Err(err) = recent_res {
            engine.print_fbg(
                0,
                y,
                &format!("Error while saving recent directories list:\n {err}"),
                theme.error_color.into(),
                bg_color,
            );
            y += 3;
        }

        if let Err(err) = favorites_res {
            engine.print_fbg(
                0,
                y,
                &format!("Error while saving favorites list:\n {err}"),
                theme.error_color.into(),
                bg_color,
            );
            y += 3;
        }

        engine.print_fbg(
            0,
            y,
            "Press Enter to continue...",
            theme.text_color.into(),
            bg_color,
        );
        engine.draw();

        let exit_codes: [KeyCode; 3] = [KeyCode::Enter, KeyCode::Esc, KeyCode::Char(' ')];

        while !exit_codes.iter().any(|c| engine.is_key_pressed(*c)) {
            engine.wait_frame();
        }
    }
}
