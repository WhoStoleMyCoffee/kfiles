use std::path::{Path, PathBuf};

use console_engine::pixel;
use console_engine::screen::Screen;
use console_engine::{crossterm::event::KeyEvent, events::Event};
use console_engine::{Color, ConsoleEngine, KeyCode, KeyEventKind, KeyModifiers};

use crate::config::{ColorTheme, Configs, FavoritesList, RecentList};
use crate::filebuffer::FileBuffer;
use crate::search::{self, SelectPanel, SelectPanelState, SearchQuery};
use crate::try_err;
use crate::{
    get_favorites_list_path, get_recent_dirs_path, themevar, APPNAME, CONFIG_PATH,
    SEARCH_PANEL_MARGIN,
};

use crate::{AppError, CONTROL_SHIFT};

pub enum AppState {
    Running,
    Exit(Option<PathBuf>),
    Help,
}

pub struct App {
    engine: ConsoleEngine,
    file_buffer: FileBuffer,
    search_panel: Option<SelectPanel>,
    recent_dirs: RecentList,
    favorites: FavoritesList,
}

impl App {
    pub fn new(at_path: &Path) -> Result<Self, AppError> {
        let cfg: &Configs = Configs::global();
        let engine: ConsoleEngine = ConsoleEngine::init_fill(cfg.update_rate)?;

        // Initialize file buffer
        let (w, h) = FileBuffer::calc_size_from_engine(&engine);
        let mut file_buffer = FileBuffer::new(at_path, Screen::new(w, h));

        try_err!( file_buffer.load_entries() => file_buffer );

        let max_recent_count: usize = cfg.max_recent_count;
        let mut app = Self {
            engine,
            file_buffer,
            search_panel: None,
            recent_dirs: RecentList::load(&get_recent_dirs_path()?, max_recent_count)
                .unwrap_or_else(|_| RecentList::new(max_recent_count)),
            favorites: FavoritesList::load(&get_favorites_list_path()?)
                .unwrap_or_default(),
        };

        app.set_title_to_current();
        Ok(app)
    }

    pub fn run(&mut self) -> AppState {
        if let Some(panel) = &mut self.search_panel {
            if panel.is_running() {
                panel.update();
            }
        }

        let bg_color: Color = themevar!(bg_color);

        match self.engine.poll() {
            Event::Frame => {
                if let Some(panel) = &mut self.search_panel {
                    self.engine.print_screen(
                        SEARCH_PANEL_MARGIN.0 as i32,
                        SEARCH_PANEL_MARGIN.1 as i32,
                        panel.draw((self.engine.frame_count % 8 > 3) as usize),
                    );
                } else {
                    self.engine.fill(pixel::pxl_bg(' ', bg_color));
                    self.engine.print_screen(1, 1, self.file_buffer.draw());
                }

                self.engine.print_fbg(
                    0,
                    0,
                    "Press F1 to show help message, Ctrl-c or Alt-F4 to exit",
                    themevar!(comment_color),
                    bg_color,
                );
                self.file_buffer.status_line.draw(&mut self.engine);

                self.engine.draw();
            }

            Event::Resize(w, h) => {
                self.engine.resize(w as u32, h as u32);
                let (w, h) = FileBuffer::calc_size_from_engine(&self.engine);
                self.file_buffer.resize(w, h);
                self.file_buffer.display_path();
            }

            Event::Mouse(mouse_event) => {
                if let Some(panel) = &mut self.search_panel {
                    panel.handle_mouse_event(mouse_event, SEARCH_PANEL_MARGIN.1 as u16);
                } else {
                    self.file_buffer.handle_mouse_event(mouse_event);
                }
            }

            Event::Key(key_event) => {
                return self.handle_key_event(key_event);
            }

        }

        AppState::Running
    }

    fn handle_key_event(&mut self, event: KeyEvent) -> AppState {
        match event {
            // Exit with Alt-F4
            KeyEvent {
                code: KeyCode::F(4),
                kind: KeyEventKind::Press,
                modifiers: KeyModifiers::ALT,
                ..
            } => {
                return AppState::Exit(Some(self.file_buffer.path.clone()));
            }

            // Exit with Ctrl-c
            // TODO remove?
            KeyEvent {
                code: KeyCode::Char('c'),
                kind: KeyEventKind::Press,
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                return AppState::Exit(Some(self.file_buffer.path.clone()));
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
                // Gets called in drop() anyways (See impl Drop for App)
                // self.add_current_to_recent();
                return AppState::Exit(None);
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
            }

            // Search folders with Ctrl-Shift-p
            KeyEvent {
                code: KeyCode::Char('P'),
                kind: KeyEventKind::Press,
                modifiers,
                ..
            } if modifiers.bits() == CONTROL_SHIFT => {
                todo!()
                // if self.search_panel.is_some() {
                //     return AppState::Running;
                // }

                // let panel: SelectPanel = self
                //     .create_search_panel(SearchQueryMode::Folders(self.file_buffer.path.clone()))
                //     .set_title("Search Folders")
                //     .set_color(themevar!(folder_color));
                // self.search_panel = Some(panel);
            }

            // Search files with Ctrl-p
            KeyEvent {
                code: KeyCode::Char('p'),
                kind: KeyEventKind::Press,
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                todo!()
                // if self.search_panel.is_some() {
                //     return AppState::Running;
                // }

                // let panel: SelectPanel = self
                //     .create_search_panel(SearchQueryMode::Files(self.file_buffer.path.clone()))
                //     .set_title("Search Files");
                // self.search_panel = Some(panel);
            }

            // Recent files with Ctrl-o
            KeyEvent {
                code: KeyCode::Char('o'),
                kind: KeyEventKind::Press,
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                todo!()
                // // If already open in favorites mode, close
                // if let Some(panel) = &self.search_panel {
                //     if let SearchQueryMode::List(_) = panel.get_query_mode() {
                //         self.search_panel = None;
                //     }
                //     return AppState::Running;
                // }

                // let panel: SelectPanel = self
                //     .create_search_panel(SearchQueryMode::List(self.recent_dirs.clone()))
                //     .set_title("Recent")
                //     .set_color(themevar!(folder_color));
                // self.search_panel = Some(panel);
            }

            // Add to favorites with Ctrl-f
            KeyEvent {
                code: KeyCode::Char('f'),
                kind: KeyEventKind::Press,
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                if self.search_panel.is_some() {
                    return AppState::Running;
                }

                self.file_buffer.status_line.normal();
                let added: bool = self.favorites.toggle(self.file_buffer.path.clone());

                if let Err(err) = confy::store(APPNAME, Some(CONFIG_PATH), Configs::global()) {
                    self.file_buffer
                        .status_line
                        .error(err.into(), Some("Error saving configs: \n "));
                } else {
                    self.file_buffer
                        .status_line
                        .set_text(if added {
                            "Added path to favorites"
                        } else {
                            "Removed path from favorites"
                        })
                        .set_color(themevar!(special_color));
                }
            }

            // Open / close favorites with ` or Tab
            KeyEvent {
                code: KeyCode::Char('`') | KeyCode::Tab,
                kind: KeyEventKind::Press,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // If already open, close
                if self.search_panel.is_some() {
                    self.search_panel = None;
                    return AppState::Running;
                }

                // let panel: SelectPanel = self.create_search_panel(SearchQueryMode::List(self.favorites.clone()))
                let mut panel: SelectPanel = self.create_search_panel(
                    Box::new( search::SearchPathList::new(&self.favorites) )
                    )
                    .set_title("Favorites")
                    .set_color(themevar!(special_color));
                
                // panel = panel.on_selected(Box::new(|| {
                // }));

                // panel = panel.on_selected(Box::new(|s: &str| {
                //         self.file_buffer.set_path(&Path::new(r"C:/Users/ddxte/Documents/Projects/kfiles"));
                //     }));
                self.search_panel = Some(panel);
            }

            // Print help with F1
            KeyEvent {
                code: KeyCode::F(1),
                kind: KeyEventKind::Press,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // If already open, close
                if self.search_panel.is_some() {
                    self.search_panel = None;
                    return AppState::Running;
                }

                let panel: SelectPanel = self.create_search_panel(
                    Box::new( search::SearchList::new(&[
                            "Help",
                            "Toggle favorite",
                            "Open configuration file",
                            "Open configuration folder",
                        ]) )
                    )
                    .set_title("Help")
                    .set_color(themevar!(special_color));
                self.search_panel = Some(panel);

                // return AppState::Help;
            }

            // Reload with F5
            KeyEvent {
                code: KeyCode::F(5),
                kind: KeyEventKind::Press,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                try_err!(self.file_buffer.load_entries() => self.file_buffer; else {
                    self.file_buffer.display_path();
                    self.file_buffer.update_scroll();
                });
            },

            key_event => {
                // Try to update search panel first
                if self.search_panel.is_some() {
                    if let Err(err) = self.searchpanel_handle_key_event(key_event) {
                        self.file_buffer
                            .status_line
                            .error(err.as_str().into(), Some("Error opening: \n"));
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

        AppState::Running
    }

    // I put this stuff in its own function because that would've been a disgusting amount of indentation
    fn searchpanel_handle_key_event(&mut self, event: KeyEvent) -> Result<(), String> {
        let search_panel: &mut SelectPanel = self.search_panel.as_mut()
            .expect("SearchPanel not set");

        search_panel.handle_key_event(event);

        match &search_panel.state {
            SelectPanelState::Running => {}

            SelectPanelState::Exit(string_maybe) => {
                let Some(string) = string_maybe.clone() else {
                    self.search_panel = None;
                    return Ok(());
                };

                self.add_current_to_recent();

                // if path.is_dir() {
                //     self.file_buffer.set_path(&path);
                // } else if path.is_file() {
                //     let file_name = path.file_name() .ok_or("Invalid file name")?;
                //     let path: &Path = path.parent() .ok_or("Parent directory not foud")?;

                //     self.file_buffer.set_path(path);
                //     self.file_buffer.select(file_name);
                // }

                self.set_title_to_current();
                self.search_panel = None;
            }
        }

        Ok(())
    }

    fn create_search_panel(&self, query: Box<dyn SearchQuery>) -> SelectPanel {
        SelectPanel::new(
            self.engine.get_width() - SEARCH_PANEL_MARGIN.0 * 2,
            self.engine.get_height() - SEARCH_PANEL_MARGIN.1 * 2,
            query,
        )
    }

    pub fn get_path(&self) -> &PathBuf {
        &self.file_buffer.path
    }

    /// Sets the window's title to `title`
    pub fn set_title(&mut self, title: &str) {
        self.engine.set_title( &format!("KFiles | {}", title) );
    }

    /// Sets the window's title using the current dir
    pub fn set_title_to_current(&mut self) {
        let Some(pathname) = self.file_buffer.path.file_name().and_then(|osstr| osstr.to_str()) else {
            return;
        };
        self.engine.set_title( &format!("KFiles | {}", pathname) );
    }

    pub fn add_to_recent(&mut self, path: PathBuf) {
        self.recent_dirs.push(path);
    }

    pub fn add_current_to_recent(&mut self) {
        self.add_to_recent(self.file_buffer.path.clone());
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

        let theme: &ColorTheme = Configs::theme();
        let bg_color: Color = theme.bg_color.into();
        let mut y: i32 = 0;

        self.engine.fill(pixel::pxl_bg(' ', bg_color));

        if let Err(err) = recent_res {
            self.engine.print_fbg(
                0,
                y,
                &format!("Error while saving recent directories list:\n {err}"),
                theme.error_color.into(),
                bg_color,
            );
            y += 3;
        }

        if let Err(err) = favorites_res {
            self.engine.print_fbg(
                0,
                y,
                &format!("Error while saving favorites list:\n {err}"),
                theme.error_color.into(),
                bg_color,
            );
            y += 3;
        }

        self.engine.print_fbg(
            0,
            y,
            "Press Enter to continue...",
            theme.text_color.into(),
            bg_color,
        );
        self.engine.draw();

        let exit_codes: [KeyCode; 3] = [KeyCode::Enter, KeyCode::Esc, KeyCode::Char(' ')];

        while !exit_codes.iter().any(|c| self.engine.is_key_pressed(*c)) {
            self.engine.wait_frame();
        }
    }
}
