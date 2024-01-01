use std::collections::VecDeque;
use std::fmt::Display;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Sender, Receiver, self};
use std::sync::{ Arc, Mutex };
use std::thread;

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

    /*
    // TODO maybe
    fn update(&mut self) {}
    */
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



type QueryResults = Vec<PathBuf>;

pub struct SearchFolders {
    root: PathBuf,
    results: Vec<IndexedString>,
    receiver: Option< Receiver<QueryResults> >,
    max_queue_size: usize,
    thread_count: usize,
    max_result_count: usize,
}

impl SearchFolders {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            results: Vec::new(),
            receiver: None,
            max_queue_size: 512,
            thread_count: 2,
            max_result_count: usize::MAX,
        }
    }

    pub fn with_queue_size(mut self, max_size: usize) -> Self {
        self.max_queue_size = max_size;
        self
    }

    pub fn with_threads(mut self, thread_count: usize) -> Self {
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
            return;
        }

        self.results.clear();
        let query: String = query.to_lowercase();

        let (tx, rx) = mpsc::channel::<QueryResults>();
        self.receiver = Some(rx);
        let path: PathBuf = self.root.to_path_buf();
        let max_queue_size: usize = self.max_queue_size;
        let thread_count: usize = self.thread_count;

        // TODO hmm i dont really like this
        thread::spawn(move || {
            let pool: ThreadPool = ThreadPool::new(thread_count);
            let queue = Arc::new(Mutex::new( VecDeque::from( [path] ) ));

            loop {
                // Pop search path
                let Ok(mut q) = queue.lock() else { break; };
                let Some(search_path) = q.pop_front() else {
                    if Arc::strong_count(&queue) == 1 { break; } // All work is done
                    continue; // Queue is empty but we might still be searching in other threads
                };
                drop(q); // Unlock mutex

                // Check connection
                if tx.send(Vec::new()).is_err() { break; }

                let queue = Arc::clone(&queue);
                let search_query: String = query.clone();
                let tx = tx.clone();
                if pool.execute(move || query_search_folders(
                        search_path,
                        search_query,
                        queue,
                        max_queue_size,
                        tx,
                )).is_err() {
                    break;
                }
            }
        });
    }

    fn get_results(&mut self) -> &Vec<IndexedString> {
        let Some(rx) = &mut self.receiver else {
            return &self.results;
        };

        let max: usize = self.max_result_count;
        for received in rx.try_iter() {
            let l: usize = self.results.len();
            for (i, pathbuf) in received.iter().enumerate() .take(max - l) {
                let is: IndexedString = IndexedString::from( (l + i, pathbuf) );
                self.results.push(is);
            }
        }

        &self.results
    }
}




fn query_search_folders(
    search_path: PathBuf,
    query: String,
    queue: Arc<Mutex< VecDeque<PathBuf> >>,
    max_queue_size: usize,
    sender: Sender<QueryResults>,
) {
    let Ok(folders) = get_all_folders_at(search_path) else { return; };

    let Ok(mut q) = queue.lock() else { return; };
    let take_count: usize = max_queue_size - q.len();
    q.append(&mut folders.iter()
             // Don't search inside folders that start with "." (like .git/)
             .filter(|pathbuf| !path2string(pathbuf.file_name().unwrap_or_default()) .starts_with('.') )
             .take(take_count)
             .cloned()
             .collect(),
             );
    drop(q); // Unlock mutex

    let folders: QueryResults = folders.into_iter()
        .filter(|pathbuf| {
            pathbuf.display().to_string().to_lowercase().contains(&query)
        })
        .collect();
    if folders.is_empty() { return; }
    let _ = sender.send(folders);

}


/*

struct SearchQuery {
    query: String,
    results: Vec<FileEntry>,
    receiver: Option<Receiver<Vec<FileEntry>>>,
    max_result_count: usize,
    max_queue_size: usize,
    thread_count: usize,
    ignore_types: String,
}

impl SearchQuery {
    // max_result_count, max_queue_size, and ignore_types are values taked from Configs
    fn new(
        mode: SearchQueryMode,
        max_result_count: usize,
        max_queue_size: usize,
        thread_count: usize,
        ignore_types: String,
    ) -> Self {
        let mut q = Self {
            query: String::new(),
            mode,
            results: Vec::new(),
            receiver: None,
            max_result_count,
            max_queue_size,
            thread_count,
            ignore_types,
        };

        q.results = q.list();
        q
    }

    // Get a list of dirs. Mainly used when there's no query but you don't wanna leave the user with an empty screen, yknow
    fn list(&self) -> Vec<FileEntry> {
        match &self.mode {
            SearchQueryMode::List(paths) => paths
                .iter()
                .enumerate()
                .map(|(i, pathbuf)| FileEntry::from(pathbuf.as_path()).prefix_idx(i))
                .collect(),

            SearchQueryMode::Files(path) => {
                let mut results: Vec<PathBuf> = Vec::new();
                let mut queue: Vec<PathBuf> = vec![path.clone()];

                while results.len() < self.max_result_count {
                    let Some(search_path) = queue.pop() else { break; };
                    let Ok((mut files, folders)) = get_files_and_folders_at(search_path) else { continue; };

                    results.append(&mut files);
                    queue.append(&mut folders.iter()
                            .take(self.max_queue_size - queue.len())
                            .cloned()
                            .collect(),
                    );
                }

                results.iter()
                    .take(self.max_result_count)
                    .map(|pathbuf| FileEntry::from(pathbuf.as_path()))
                    .collect()
            }

            SearchQueryMode::Folders(path) => {
                let Ok(mut results) = get_folders_at(path, self.max_result_count) else {
                    return Vec::new();
                };
                let mut idx: usize = 0;

                while results.len() < self.max_result_count {
                    let Some(search_path) = results.get(idx) else { break; };

                    let limit: usize = self.max_result_count - results.len();
                    let Ok(mut folders) = get_folders_at(search_path, limit) else {
                        continue;
                    };
                    results.append(&mut folders);
                    idx += 1;
                }

                results.iter()
                    .take(self.max_result_count)
                    .map(|pathbuf| FileEntry::from(pathbuf.as_path()))
                    .collect()
            }
        }
    }

    fn update(&mut self) {
        let Some(rx) = &mut self.receiver else { return; };

        let max: usize = self.max_result_count;

        for received in rx.try_iter() {
            for entry in received.into_iter().take(max - self.results.len()) {
                self.results.push(entry);
            }
        }

        if self.results.len() >= max {
            self.receiver = None;
        }
    }

    fn search(&mut self, query: String) {
        if query == self.query { return; } // Query hasn't changed
        self.query = query;

        if self.query.is_empty() {
            self.results = self.list();
            return;
        }

        self.results.clear();
        let search_query: String = self.query.to_lowercase();

        // Files and Folders are done on threads. List isn't.
        // We use mpsc to send results back for this specific query
        // A new set of sender and receiver is created for each query, replacing the old self.reveiver
        // If that happens while the thread is running, stop searching (obviously)

        // Man, that's a disgusting amount of indentation lol
        match &self.mode {
            SearchQueryMode::List(paths) => {
                self.receiver = None;
                self.results = paths.iter().enumerate()
                    .map(|(i, pathbuf)| FileEntry::from(pathbuf.as_path()) .prefix_idx(i))
                    .filter(|entry| entry.to_string().to_lowercase().contains(&search_query))
                    .collect();
            }

            SearchQueryMode::Files(path) => {
                let (tx, rx) = mpsc::channel::<Vec<FileEntry>>();
                self.receiver = Some(rx);
                let path: PathBuf = path.clone();
                let max_queue_size: usize = self.max_queue_size;
                let thread_count: usize = self.thread_count;
                let ignore_types: String = self.ignore_types.clone();

                thread::spawn(move || {
                    let pool: ThreadPool = ThreadPool::new(thread_count);
                    let queue = Arc::new(Mutex::new( VecDeque::from( [path] ) ));

                    loop {
                        // Pop search path
                        let Ok(mut q) = queue.lock() else { break; };
                        let Some(search_path) = q.pop_front() else {
                            if Arc::strong_count(&queue) == 1 { break; } // All work is done
                            continue; // Queue is empty but we might still be searching in other threads
                        };
                        drop(q); // Unlock mutex

                        // Check connection
                        if tx.send(Vec::new()).is_err() { break; }

                        let queue = Arc::clone(&queue);
                        let search_query: String = search_query.clone();
                        let ignore_types: String = ignore_types.clone();
                        let tx = tx.clone();
                        if pool.execute(move || query_search_files(
                            search_path,
                            search_query,
                            ignore_types,
                            queue,
                            max_queue_size,
                            tx,
                        )).is_err() {
                            break;
                        }
                    }
                });
            } // end SearchQueryMode::Files(path)

            SearchQueryMode::Folders(path) => {
                let (tx, rx) = mpsc::channel::<Vec<FileEntry>>();
                self.receiver = Some(rx);
                let path: PathBuf = path.clone();
                let max_queue_size: usize = self.max_queue_size;
                let thread_count: usize = self.thread_count;

                thread::spawn(move || {
                    let pool: ThreadPool = ThreadPool::new(thread_count);
                    let queue = Arc::new(Mutex::new( VecDeque::from( [path] ) ));

                    loop {
                        // Pop search path
                        let Ok(mut q) = queue.lock() else { break; };
                        let Some(search_path) = q.pop_front() else {
                            if Arc::strong_count(&queue) == 1 { break; } // All work is done
                            continue; // Queue is empty but we might still be searching in other threads
                        };
                        drop(q); // Unlock mutex

                        // Check connection
                        if tx.send(Vec::new()).is_err() { break; }

                        let queue = Arc::clone(&queue);
                        let search_query: String = search_query.clone();
                        let tx = tx.clone();
                        if pool.execute(move || query_search_folders(
                                search_path,
                                search_query,
                                queue,
                                max_queue_size,
                                tx,
                        )).is_err() {
                            break;
                        }
                    }
                });

            } // end SearchQueryMode::Folders()
        }
    }
}
// Beautiful



fn query_search_files(
    search_path: PathBuf,
    query: String,
    ignore_types: String,
    queue: Arc<Mutex< VecDeque<PathBuf> >>,
    max_queue_size: usize,
    sender: Sender<Vec<FileEntry>>,
) {
    let Ok((files, folders)) = get_files_and_folders_at(search_path) else {
        return;
    };

    let Ok(mut q) = queue.lock() else { return; };
    let take_count: usize = max_queue_size - q.len();
    q.append(&mut folders.into_iter()
         // Don't search inside folders that start with "." (like .git/)
         .filter(|pathbuf| !path2string(pathbuf.file_name().unwrap_or_default()) .starts_with('.'))
         .take(take_count)
         .collect(),
         );
    drop(q); // Unlock mutex

    let files: Vec<FileEntry> = files.into_iter()
        .filter(|pathbuf| {
            !ignore_types.contains(&path2string(pathbuf.extension().unwrap_or_default()))
                && path2string(pathbuf).to_lowercase().contains(&query)
        })
        .map(FileEntry::from)
        .collect();
    if files.is_empty() { return; }
    let _ = sender.send(files);
}



*/
