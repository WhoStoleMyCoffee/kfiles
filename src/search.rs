use std::collections::VecDeque;
use std::fmt::Display;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Sender, Receiver, self};
use std::sync::{ Arc, Mutex };
use std::thread::{self, JoinHandle};

use console_engine::{
    pixel, rect_style::BorderStyle, screen::Screen, Color, KeyCode, KeyEventKind,
};

use console_engine::crossterm::event::{KeyEvent, MouseEvent, MouseEventKind};
use console_engine::events::Event;
use console_engine::forms::{Form, FormField, FormOptions, FormStyle, FormValue, Text};

use threads_pool::ThreadPool;

use crate::config::{ColorTheme, Configs};
use crate::{themevar, util::*};


#[derive(Debug, Clone)]
pub struct IndexedString (usize, String);

impl AsRef<String> for IndexedString {
    fn as_ref(&self) -> &String {
        &self.1
    }
}

impl AsMut<String> for IndexedString {
    fn as_mut(&mut self) -> &mut String {
        &mut self.1
    }
}

impl From<(usize, &PathBuf)> for IndexedString {
    fn from(value: (usize, &PathBuf)) -> Self {
        Self (value.0, value.1.display().to_string())
    }
}

impl From<(usize, &String)> for IndexedString {
    fn from(value: (usize, &String)) -> Self {
        Self (value.0,value.1.to_string())
    }
}

impl ToString for IndexedString {
    fn to_string(&self) -> String {
        format!("{}: {}", self.0, self.1)
    }
}


#[derive(Debug, PartialEq)]
pub enum SelectPanelState {
    Running,
    Exit,
}

type Callback = Box< dyn FnMut(&IndexedString) >;

// Used for quick-searching files, folders, and favorites
// The actual searching happens in SearchQuery
pub struct SelectPanel {
    screen: Screen,
    form: Form, // Input box
    title: String,
    color: Color,
    selected_index: usize,
    query: Box<dyn SearchQuery>,
    callback: Option<Callback>,
    pub state: SelectPanelState,
}

impl SelectPanel {
    pub fn new(
        width: u32,
        height: u32,
        query: Box<dyn SearchQuery>
    ) -> Self {
        let cfg: &Configs = Configs::global();

        Self {
            screen: Screen::new(width, height),
            form: SelectPanel::build_form(width - 2, cfg.theme.bg_color.into()),
            title: "Search".to_string(),
            color: cfg.theme.file_color.into(),
            selected_index: 0,
            query,
            callback: None,
            // query: SearchQuery::new(
            //     mode,
            //     max_result_count,
            //     cfg.max_search_queue_len,
            //     cfg.search_thread_count,
            //     cfg.search_ignore_types.clone(),
            // ),
            state: SelectPanelState::Running,
        }
    }

    pub fn with_title(mut self, title: &str) -> Self {
        self.title = title.to_string();
        self
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn set_query(&mut self, query: Box<dyn SearchQuery>) {
        self.query = query;
    }

    fn build_form(width: u32, bg_color: Color) -> Form {
        let theme = FormStyle {
            border: Some(BorderStyle::new_light().with_colors(themevar!(text_color), bg_color)),
            bg: bg_color,
            ..Default::default()
        };

        let mut form = Form::new(
            width,
            3,
            FormOptions {
                style: theme,
                ..Default::default()
            },
        );
        form.build_field::<Text>(
            "query",
            FormOptions {
                style: theme,
                ..Default::default()
            },
        );

        form.set_active(true);
        form
    }

    pub fn update(&mut self) {
        // self.query.update();
        let count: usize = self.query.get_results().len();
        self.selected_index = self.selected_index.min(count.max(1) - 1);
    }

    pub fn is_running(&self) -> bool {
        self.state == SelectPanelState::Running
    }

    pub fn handle_mouse_event(&mut self, event: MouseEvent, y_offset: u16) {
        if let MouseEvent {
            kind: MouseEventKind::Down(_) | MouseEventKind::Drag(_),
            row,
            ..
        } = event
        {
            let offset: u16 = y_offset + 4;
            let urow: u16 = row.max(offset) - offset;
            let count: usize = self.query.get_results().len();
            self.selected_index = (urow as usize).min(count.max(1) - 1);
        }
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) {
        if key_event.kind != KeyEventKind::Press {
            return;
        }

        // No need to check for KeyEvent.kind because we only allowed KeyEventKind::Press up there
        match key_event {
            // Move cursor up
            KeyEvent {
                code: KeyCode::Up, ..
            } => {
                let count: usize = self.query.get_results().len();
                self.selected_index = self.selected_index
                    .checked_sub(1)
                    .or_else(|| count.checked_sub(1))
                    .unwrap_or_default();
                return;
            }

            // Move cursor down
            KeyEvent {
                code: KeyCode::Down,
                ..
            } => {
                let len: usize = self.query.get_results().len();
                if len == 0 { return; }
                self.selected_index = (self.selected_index + 1) % len;
                return;
            }

            // Esc to exit
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                self.state = SelectPanelState::Exit;
                return;
            }

            // Form input
            _ => {
                self.form.handle_event(Event::Key(key_event));

                if self.form.is_finished() {
                    let Some(selected) = self.query.get_results().get(self.selected_index) else {
                        return;
                    };

                    if let Some(cb) = &mut self.callback {
                        cb(selected);
                    }

                    self.state = SelectPanelState::Exit;
                    return;
                }
            }
        }

        // Handle query
        let value: String = match self.form.get_field_output("query") {
            Some(FormValue::String(value)) => value.replace('/', r"\"),
            _ => return,
        };

        self.query.search(&value);
    }

    fn display_results(&mut self) {
        if self.query.get_results().is_empty() { return; }

        let offset: (i32, i32) = (2, 4);
        let theme: &ColorTheme = Configs::theme();
        let bg_color = Color::from(theme.bg_color);

        // Highlight selected line
        self.screen.h_line(
            offset.0,
            self.selected_index as i32 + offset.1,
            self.screen.get_width() as i32 - 2,
            pixel::pxl_bg(' ', theme.comment_color.into()),
        );

        // Entries
        let width: usize = self.screen.get_width() as usize - 3;
        for (i, entry) in self.query.get_results().iter().enumerate() {
            // Reached the bottom
            if i as u32 + offset.1 as u32 >= self.screen.get_height() - 1 {
                break;
            }

            let bg: Color = if i == self.selected_index {
                theme.comment_color.into()
            } else {
                bg_color
            };

            let s: String = entry.to_string()
                .replace('\\', "/")
                .trunc_back(width);
            self.screen.print_fbg(offset.0, i as i32 + offset.1, &s, self.color, bg);
        }

        /*
        let strip_root: Option<&Path> = match &self.query.mode {
            SearchQueryMode::Files(root) | SearchQueryMode::Folders(root) => Some(root),
            _ => None,
        };
        for (i, entry) in self.query.results.iter().enumerate() {
            // Reached the bottom
            if i as u32 + offset.1 as u32 >= self.screen.get_height() - 1 {
                break;
            }

            let bg: Color = if i == self.selected_index {
                theme.comment_color.into()
            } else {
                bg_color
            };

            let path: &Path = if let Some(root) = strip_root {
                entry.strip_prefix(root).unwrap_or(entry)
            } else {
                entry
            };

            let mut path_string: String = path2string(path)
                .replace('\\', "/")
                .trunc_back(width);

            if let Some(prefix) = &entry.prefix {
                path_string = format!("{prefix} {path_string}");
            }

            self.screen
                .print_fbg(offset.0, i as i32 + offset.1, &path_string, self.color, bg);
        }
        */

    }

    // TODO optimize maybe
    pub fn draw(&mut self, tick: usize) -> &Screen {
        let theme: &ColorTheme = Configs::theme();
        let bg_color = theme.bg_color;
        let bg_inverted = theme.bg_color.inv();

        self.screen.fill(pixel::pxl_bg(' ', bg_color.into()));

        self.screen.rect_border(
            0,
            0,
            self.screen.get_width() as i32 - 1,
            self.screen.get_height() as i32 - 1,
            BorderStyle::new_light().with_colors(bg_inverted.into(), bg_color.into()),
        );

        self.screen.print_screen(1, 1, self.form.draw(tick));
        self.display_results();

        let text: String = format!(" {} ", &self.title);
        self.screen
            .print_fbg(2, 0, &text, bg_color.into(), bg_inverted.into());

        &self.screen
    }

    pub fn on_selected<F>(mut self, f: F) -> Self
    where F: FnMut(&IndexedString) + 'static {
        self.callback = Some(Box::new(f));
        self
    }

    pub fn get_list_height(&self) -> usize {
        (self.screen.get_height() - 5) as usize
    }

}




pub trait SearchQuery {
    fn get_list(&self) -> Vec<IndexedString>;

    fn search(&mut self, query: &str);

    fn get_results(&mut self) -> &Vec<IndexedString>;
}


/// TODO remove
pub struct EmptySearchList ( Vec<IndexedString> );

impl EmptySearchList {
    pub fn new() -> Self {
        Self ( Vec::new() )
    }
}

impl SearchQuery for EmptySearchList {
    fn get_list(&self) -> Vec<IndexedString> {
        self.0.iter()
            .cloned()
            .collect()
    }

    fn get_results(&mut self) -> &Vec<IndexedString> {
        &self.0
    }

    fn search(&mut self, _query: &str) {}
}





pub struct SearchPathList {
    pub items: Vec<PathBuf>,
    pub results: Vec<IndexedString>,
}

impl SearchPathList {
    pub fn new(items: &[PathBuf]) -> Self {
        let mut l = Self {
            items: items.to_vec(),
            results: Vec::new(),
        };
        l.results = l.get_list();
        l
    }
}

impl SearchQuery for SearchPathList {
    fn get_list(&self) -> Vec<IndexedString> {
        self.items.iter().enumerate()
            .map(IndexedString::from)
            .collect()
    }

    fn search(&mut self, query: &str) {
        if query.is_empty() {
            self.results = self.get_list();
            return;
        }

        let q: String = query.to_lowercase();
        self.results = self.items.iter().enumerate()
            .map(IndexedString::from)
            .filter(|s| s.to_string().to_lowercase().contains(&q))
            .collect();
    }

    fn get_results(&mut self) -> &Vec<IndexedString> {
        &self.results
    }
}





pub struct SearchList {
    pub items: Vec<String>,
    pub results: Vec<IndexedString>,
}

impl SearchList {
    pub fn new(items: &[&str]) -> Self {
        let mut l = Self {
            items: items.iter()
                .map(|s| s.to_string())
                .collect(),
            results: Vec::new(),
        };
        l.results = l.get_list();
        l
    }
}

impl SearchQuery for SearchList {
    fn get_list(&self) -> Vec<IndexedString> {
        self.items.iter().enumerate()
            .map(IndexedString::from)
            .collect()
    }

    fn search(&mut self, query: &str) {
        if query.is_empty() {
            self.results = self.get_list();
            return;
        }
        let q: String = query.to_lowercase();
        self.results = self.items.iter().enumerate()
            .map(IndexedString::from)
            .filter(|s| s.to_string().to_lowercase().contains(&q))
            .collect();
    }

    fn get_results(&mut self) -> &Vec<IndexedString> {
        &self.results
    }
}



struct QueryResults {
    results: Vec<PathBuf>,
    new_queue: Vec<PathBuf>,
}

impl QueryResults {
    fn empty() -> Self {
        Self {
            results: Vec::new(),
            new_queue: Vec::new(),
        }
    }
}



pub struct SearchFolders {
    root: PathBuf,
    results: Vec<IndexedString>,
    thread_count: u8,
    threads: Vec<JoinHandle<()>>,
    queue: Arc<Mutex< VecDeque<PathBuf> >>,
    receiver: Option<Receiver< QueryResults >>,
    max_result_count: usize,
    max_queue_len: Option<usize>,
}

impl SearchFolders {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            results: Vec::new(),
            thread_count: 1,
            threads: Vec::new(),
            queue: Arc::new(Mutex::new( VecDeque::new() )),
            receiver: None,
            max_result_count: usize::MAX,
            max_queue_len: None,
        }
    }

    pub fn with_queue_len(mut self, max_len: Option<usize>) -> Self {
        self.max_queue_len = max_len;
        self
    }

    pub fn with_threads(mut self, thread_count: u8) -> Self {
        self.thread_count = thread_count;
        self
    }

    pub fn with_max_result(mut self, max_result_count: usize) -> Self {
        self.max_result_count = max_result_count;
        self
    }

    pub fn list(mut self) -> Self {
        self.results = self.get_list();
        self
    }
}

impl SearchQuery for SearchFolders {
    fn get_list(&self) -> Vec<IndexedString> {
        let max: usize = self.max_result_count;
        let Ok(mut results) = get_folders_at(&self.root, max) else {
            return Vec::new();
        };
        let mut idx: usize = 0;

        while results.len() < max {
            let Some(search_path) = results.get(idx) else { break; };
            let limit: usize = max - results.len();
            let Ok(mut folders) = get_folders_at(search_path, limit) else {
                idx += 1;
                continue;
            };
            results.append(&mut folders);
            idx += 1;
        }

        results.iter().enumerate()
            .take(max)
            .map(IndexedString::from)
            .collect()
    }

    fn search(&mut self, query: &str) {
        if query.is_empty() {
            self.results = self.get_list();
            self.receiver = None;
            return;
        }

        let (tx, rx) = mpsc::channel::<QueryResults>();
        self.receiver = Some(rx);

        // TODO make optional?
        for handle in self.threads.drain(..) {
            let _ = handle.join();
        }

        self.results.clear();

        if let Ok(mut q) = self.queue.lock() {
            *q = VecDeque::from( [self.root.to_path_buf()] );
        } else {
            return;
        }

        for _ in 0..self.thread_count {
            let queue = Arc::clone(&self.queue);
            let tx = tx.clone();
            let query: String = query.to_lowercase();
            let root = self.root.clone();

            let handle = thread::spawn(move || loop {
                // Check connection
                if tx.send( QueryResults::empty() ).is_err() { break; }

                let Ok(mut q) = queue.lock() else { break; };
                let Some(search_path) = q.pop_front() else {
                    continue;
                };
                drop(q); // Unlock mutex

                let Ok(folders) = get_all_folders_at(search_path) else { continue; };

                let results: Vec<PathBuf> = folders.iter()
                    .filter(|pb|
                        path2string( pb.strip_prefix(&root).unwrap_or(pb) ) .to_lowercase() .contains(&query)
                    )
                    .cloned()
                    .collect();

                let new_queue: Vec<PathBuf> = folders.into_iter()
                     .filter(|pb| !path2string(pb.file_name().unwrap_or_default()) .starts_with('.') )
                     .collect();

                if tx.send(QueryResults { results, new_queue, })
                    .is_err() {
                    break;
                }
            });

            self.threads.push(handle);
        }

    }

    fn get_results(&mut self) -> &Vec<IndexedString> {
        let Some(rx) = &mut self.receiver else {
            return &self.results;
        };

        let max: usize = self.max_result_count;
        for QueryResults { results, mut new_queue } in rx.try_iter() {

            if !results.is_empty() {
                let l: usize = self.results.len();
                for (i, pathbuf) in results.iter().enumerate() .take(max - l) {
                    let is: IndexedString = IndexedString::from( (l + i, pathbuf) );
                    self.results.push(is);
                }
            }

            if !new_queue.is_empty() {
                if let Some(max_queue_len) = self.max_queue_len {
                    new_queue.truncate(max_queue_len);
                }

                self.queue.lock().unwrap()
                    .append(&mut VecDeque::from(new_queue));
            }

        }

        if self.results.len() >= max {
            self.receiver = None;
            self.queue.lock().unwrap()
                .clear();
        }

        &self.results

    }
}


