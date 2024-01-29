use std::path::{Path, PathBuf};

use console_engine::KeyModifiers;
use console_engine::{
    pixel, rect_style::BorderStyle, screen::Screen, Color, KeyCode, KeyEventKind,
};

use console_engine::crossterm::event::{KeyEvent, MouseEvent, MouseEventKind};
use console_engine::events::Event;
use console_engine::forms::{Form, FormField, FormOptions, FormStyle, FormValue, Text};

use crate::config::{ColorTheme, Configs};
use crate::{themevar, util::*};
use query::*;



const SEARCH_LIST_OFFSET: (u8, u8) = (2, 4);


#[derive(Debug, Clone)]
pub struct IndexedString (usize, String);

impl IndexedString {
    pub fn index(&self) -> usize {
        self.0
    }
}

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

impl From<(usize, &Path)> for IndexedString {
    #[inline]
    fn from(value: (usize, &Path)) -> Self {
        Self (value.0, value.1.display().to_string())
    }
}

impl From<(usize, &PathBuf)> for IndexedString {
    #[inline]
    fn from(value: (usize, &PathBuf)) -> Self {
        Self (value.0, value.1.display().to_string())
    }
}

impl From<(usize, &String)> for IndexedString {
    #[inline]
    fn from(value: (usize, &String)) -> Self {
        Self (value.0,value.1.to_string())
    }
}

impl ToString for IndexedString {
    #[inline]
    fn to_string(&self) -> String {
        format!("{}: {}", self.0, self.1)
    }
}

impl PartialEq for IndexedString {
    fn eq(&self, other: &Self) -> bool {
        self.1 == other.1
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
            state: SelectPanelState::Running,
        }
    }

    pub fn calc_list_height(panel_height: u32) -> u32 {
        panel_height - SEARCH_LIST_OFFSET.1 as u32 - 1
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
        self.query.update();
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
                code: KeyCode::Esc,
                ..
            } => {
                self.state = SelectPanelState::Exit;
                return;
            }

            KeyEvent {
                code: KeyCode::Backspace,
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.form.reset();
            },

            // Form input
            _ => {
                self.form.handle_event(Event::Key(key_event));

                if self.form.is_finished() {
                    let Some(selected) = self.query.get_results().get(self.selected_index) else {
                        // Search unsuccessful
                        self.form.reset();
                        self.query.search("");
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

        let theme: &ColorTheme = Configs::theme();
        let bg_color = Color::from(theme.bg_color);

        // Highlight selected line
        self.screen.h_line(
            SEARCH_LIST_OFFSET.0 as i32,
            self.selected_index as i32 + SEARCH_LIST_OFFSET.1 as i32,
            self.screen.get_width() as i32 - 2,
            pixel::pxl_bg(' ', theme.comment_color.into()),
        );

        // Entries
        let width: usize = self.screen.get_width() as usize - 3;
        // for (i, entry) in self.query.get_results() .iter().enumerate() {
        for (i, entry) in self.query.iter_display().enumerate() {
            // Reached the bottom
            if i as u32 + SEARCH_LIST_OFFSET.1 as u32 >= self.screen.get_height() - 1 {
                break;
            }

            let bg: Color = if i == self.selected_index {
                theme.comment_color.into()
            } else {
                bg_color
            };

            let s: String = entry.to_string()
                .trunc_back(width);

            self.screen.print_fbg(
                SEARCH_LIST_OFFSET.0 as i32,
                i as i32 + SEARCH_LIST_OFFSET.1 as i32,
                &s,
                self.color,
                bg
            );
        }

    }

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
        self.screen.print_fbg(2, 0, &text, bg_color.into(), bg_inverted.into());

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




pub mod query {
    use std::path::{Path, PathBuf};
    use std::collections::VecDeque;
    use std::sync::mpsc::{Receiver, self, Sender, SendError};
    use std::sync::{ Arc, Mutex };
    use std::thread;
    use std::time::Duration;

    use super::IndexedString;
    use crate::util::{
        get_folders_at,
        get_files_and_folders_at,
        path2string,
        get_all_folders_at,
    };
    use crate::config::Configs; // TODO decouple query & Configs


    pub trait SearchQuery {
        fn get_list(&self) -> Vec<IndexedString>;

        fn search(&mut self, query: &str);

        fn update(&mut self) {}

        fn get_results(&self) -> &Vec<IndexedString>;

        fn iter_display(&self) -> Box<dyn Iterator<Item = String> + '_> {
            Box::new(self.get_results().iter()
                .map(|is| is.to_string())
            )
        }
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

        fn get_results(&self) -> &Vec<IndexedString> {
            &self.results
        }

        fn iter_display(&self) -> Box<dyn Iterator<Item = String> + '_> {
            Box::new(self.get_results().iter()
                .map(|is| is.to_string().replace('\\', "/"))
            )
        }
    }





    pub struct SearchList<S: ToString> {
        pub items: Vec<S>,
        pub results: Vec<IndexedString>,
    }

    impl<S: ToString + Clone> SearchList<S> {
        pub fn new(items: &[S]) -> Self {
            let mut l = Self {
                items: items.to_vec(),
                results: Vec::new(),
            };
            l.results = l.get_list();
            l
        }
    }

    impl<S: ToString + Clone> SearchQuery for SearchList<S> {
        fn get_list(&self) -> Vec<IndexedString> {
            self.items.iter().enumerate()
                .map( |(i, s)| IndexedString(i, s.to_string()) )
                .collect()
        }

        fn search(&mut self, query: &str) {
            if query.is_empty() {
                self.results = self.get_list();
                return;
            }

            let q: String = query.to_lowercase();
            self.results = self.items.iter().enumerate()
                .map( |(i, s)| IndexedString(i, s.to_string()) )
                .filter(|s| s.to_string().to_lowercase() .contains(&q))
                .collect();
        }

        fn get_results(&self) -> &Vec<IndexedString> {
            &self.results
        }

        fn iter_display(&self) -> Box<dyn Iterator<Item = String> + '_> {
            Box::new(self.get_results().iter()
                .map(|IndexedString(_, s)| s.clone())
            )
        }
    }



    enum QueryResults {
        ConnectionCheck,
        Results {
            results: Vec<PathBuf>,
            new_queue: Vec<PathBuf>,
            query_len: usize,
        }
    }




    struct ThreadedSearchBundle {
        receiver: Receiver<QueryResults>,
        queue: Arc<Mutex< VecDeque<PathBuf> >>,
        query: String,
        query_change_notifiers: Vec< Sender<String> >
    }

    impl ThreadedSearchBundle {
        fn new(query: &str, queue: &[PathBuf]) -> (Self, Sender<QueryResults>) {
            let (tx, rx) = mpsc::channel::<QueryResults>();
            let tsb = Self {
                receiver: rx,
                queue: Arc::new(Mutex::new( VecDeque::from(queue.to_vec()) )),
                query: query.to_string(),
                query_change_notifiers: Vec::new(),
            };
            (tsb, tx)
        }

        fn register_query_notifier(&mut self) -> Receiver<String> {
            let (tx, rx) = mpsc::channel::<String>();
            self.query_change_notifiers.push(tx);
            rx
        }

        fn change_query(&mut self, new_query: &str) -> Result<(), SendError<String>> {
            self.query = new_query.to_string();
            // Broadcast query change
            for sender in self.query_change_notifiers.iter() {
                sender.send(new_query.to_string())?;
            }
            Ok(())
        }
    }


    enum TryUpdateQueryError {
        /// No bundle
        NoBundle,
        /// New query is not a subset (extension) of the current query
        QueryNotSubset,
        /// Error while sending update query notification to threads
        NotifyError( SendError<String> ),
    }







    pub struct SearchFolders {
        root: PathBuf,
        results: Vec<IndexedString>,
        thread_count: u8,
        bundle: Option<ThreadedSearchBundle>,
        max_result_count: usize,
        max_queue_len: Option<usize>,
    }

    impl SearchFolders {
        pub fn new(root: &Path) -> Self {
            Self {
                root: root.to_path_buf(),
                results: Vec::new(),
                thread_count: 1,
                bundle: None,
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

        pub fn with_max_results(mut self, max_result_count: usize) -> Self {
            self.max_result_count = max_result_count;
            self
        }

        pub fn list(mut self) -> Self {
            self.results = self.get_list();
            self
        }

        fn new_query(&mut self, new_query: &str) {
            let (mut bundle, result_sender) = ThreadedSearchBundle::new(
                new_query,
                &[ self.root.to_path_buf() ]
            );

            self.results.retain(|IndexedString(_, s)| s.to_lowercase() .contains(new_query));

            // Spawn threads
            let (fast_loop_ms, slow_loop_ms) = {
                let perf = Configs::performance();
                (perf.thread_fast_ms, perf.thread_slow_ms)
            };

            for _ in 0..self.thread_count {
                let queue = Arc::clone(&bundle.queue);
                let result_sender = result_sender.clone();
                let query_receiver = bundle.register_query_notifier();
                let mut query: String = new_query.to_lowercase();
                let root = self.root.clone();

                thread::spawn(move || {
                    let mut wait_ms: u64 = ( (fast_loop_ms + slow_loop_ms) / 2 ) as u64; // Time between loops to prevent spam
                    loop {
                        thread::sleep(Duration::from_millis(wait_ms));

                        // Check connection
                        if result_sender.send( QueryResults::ConnectionCheck ).is_err() { break; }

                        // Check for query changes
                        if let Some(new_query) = query_receiver.try_iter().last() {
                            query = new_query;
                        }

                        // Get search path
                        let Ok(mut q) = queue.lock() else { break; };
                        let Some(search_path) = q.pop_front() else {
                            wait_ms = (wait_ms + slow_loop_ms as u64) / 2; // Slow down
                            continue;
                        };
                        drop(q); // Unlock mutex

                        let Ok(folders) = get_all_folders_at(search_path) else { continue; };

                        let results: Vec<PathBuf> = folders.iter()
                            .filter_map(|pb| pb.strip_prefix(&root).ok() .map(PathBuf::from) )
                            .filter(|pb| path2string(pb).to_lowercase() .contains(&query))
                            .collect();

                        let new_queue: Vec<PathBuf> = folders.into_iter()
                             // Don't search inside folders that start with "." (like .git/)
                             .filter(|pb| !path2string(pb.file_name().unwrap_or_default()) .starts_with('.') )
                             .collect();

                        if result_sender.send(QueryResults::Results { results, new_queue, query_len: query.len() })
                            .is_err() {
                            break;
                        }

                        // wait_ms = fast_loop_ms as u64; // Speed up
                        wait_ms = (wait_ms + fast_loop_ms as u64) / 2; // Speed up
                    }
                });
            }

            self.bundle = Some(bundle);
        }

        fn try_update_query(&mut self, new_query: &str) -> Result<(), TryUpdateQueryError> {
            // No query
            let Some(bundle) = &mut self.bundle else {
                return Err(TryUpdateQueryError::NoBundle);
            };
            // new_query is not an extension of current query
            if new_query.len() <= bundle.query.len() {
                return Err(TryUpdateQueryError::QueryNotSubset);
            }

            // Notify threads of query change
            if let Err(err) = bundle.change_query(new_query) {
                return Err(TryUpdateQueryError::NotifyError( err ));
            }

            // Filter results
            self.results.retain(|IndexedString(_, s)| s.to_lowercase() .contains(new_query));

            Ok(())
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

            results.truncate(max);
            results.iter()
                .filter_map(|pb| pb.strip_prefix(&self.root).ok())
                .enumerate()
                .map(IndexedString::from)
                .collect()
        }

        fn search(&mut self, query: &str) {
            if query.is_empty() {
                self.results = self.get_list();
                self.bundle = None;
                return;
            }

            /* Epic plan for not wasting search results when the user makes a query that's just the current query but extended like "my search que" => "my search quer" which narrows the search results but also when they shorten the query e.g. "my search que" => "my search qu" that widens the search results thing so we wanna reset and restart the query because yes just trust me bro
             * ... or "epic plan" for short.
             * if new > old   => filter results & notify threads
             * if new == old  => reset queue + results & new channel + threads?
             * if new < old   => reset queue + results & new channel + threads
             * if old == None => reset queue + results & new channel + threads
             */

            // Try to filter & notify, if that fails, reset & new
            match self.try_update_query(query) {
                Ok(()) => {},
                Err(_) => self.new_query(query),
            }
        }

        fn update(&mut self) {
            let Some(bundle) = &mut self.bundle else { return; };
            let rx = &bundle.receiver;
            let current_query_len: usize = bundle.query.len();

            let max: usize = self.max_result_count;
            for res in rx.try_iter() {
                match res {
                    QueryResults::ConnectionCheck => {},
                    QueryResults::Results { results, new_queue, query_len } => {
                        // Append queue
                        if !new_queue.is_empty() {
                            let Ok(mut q) = bundle.queue.lock() else { continue; };

                            q.append(&mut VecDeque::from(new_queue));
                            if let Some(max_queue_len) = self.max_queue_len {
                                q.truncate(max_queue_len);
                            }
                        }

                        // Append results
                        let l: usize = self.results.len();
                        if results.is_empty() { continue; }
                        let r = results.into_iter().enumerate()
                            .map(|(i, pb)| IndexedString::from( (l + i, &pb) ));

                        // Same query
                        if query_len == current_query_len {
                            self.results.append(&mut r
                                .filter(|is| !self.results.contains(is)) // Check for duplicates
                                .collect())
                        // Probably outdated
                        } else {
                            self.results.append(&mut r
                                .filter(|is| !self.results.contains(is) && is.as_ref().contains(&bundle.query)) //  Also check for query
                                .collect())
                        }
                        self.results.truncate(max);
                    },
                }
            }

            if self.results.len() >= max {
                self.bundle = None;
            }
        }

        fn get_results(&self) -> &Vec<IndexedString> {
            &self.results
        }

        fn iter_display(&self) -> Box<dyn Iterator<Item = String> + '_> {
            Box::new(self.get_results().iter()
                .map(|IndexedString(_, s)| s.replace('\\', "/"))
            )
        }

    }




    pub struct SearchFiles {
        root: PathBuf,
        results: Vec<IndexedString>,
        thread_count: u8,
        bundle: Option<ThreadedSearchBundle>,
        max_result_count: usize,
        max_queue_len: Option<usize>,
        /// File extension types to ignore
        ignore_extensions: Vec<String>,
    }

    impl SearchFiles {
        pub fn new(root: &Path) -> Self {
            Self {
                root: root.to_path_buf(),
                results: Vec::new(),
                thread_count: 1,
                bundle: None,
                max_result_count: usize::MAX,
                max_queue_len: None,
                ignore_extensions: Vec::new(),
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

        pub fn with_max_results(mut self, max_result_count: usize) -> Self {
            self.max_result_count = max_result_count;
            self
        }

        pub fn ignore_extensions(mut self, extensions_to_ignore: &[String]) -> Self {
            self.ignore_extensions = extensions_to_ignore.to_vec();
            self
        }

        pub fn list(mut self) -> Self {
            self.results = self.get_list();
            self
        }

        fn new_query(&mut self, new_query: &str) {
            let (mut bundle, result_sender) = ThreadedSearchBundle::new(
                new_query,
                &[ self.root.to_path_buf() ]
            );

            self.results.retain(|IndexedString(_, s)| s.to_lowercase() .contains(new_query));

            // Spawn threads
            let (fast_loop_ms, slow_loop_ms) = {
                let perf = Configs::performance();
                (perf.thread_fast_ms, perf.thread_slow_ms)
            };

            for _ in  0..self.thread_count {
                let queue = Arc::clone(&bundle.queue);
                let result_sender = result_sender.clone();
                let query_receiver = bundle.register_query_notifier();
                let mut query: String = new_query.to_lowercase();
                let root = self.root.clone();
                let ignore_types: String = self.ignore_extensions.join(",");

                thread::spawn(move || {
                    let mut wait_ms: u64 = ( (fast_loop_ms + slow_loop_ms) / 2 ) as u64; // Time between loops to prevent spam
                    loop {
                        thread::sleep(Duration::from_millis(wait_ms));

                        // Check connection
                        if result_sender.send( QueryResults::ConnectionCheck ).is_err() { break; }

                        // Check for query changes
                        if let Some(new_query) = query_receiver.try_iter().last() {
                            query = new_query;
                        }

                        // Get search path
                        let Ok(mut q) = queue.lock() else { break; };
                        let Some(search_path) = q.pop_front() else {
                            wait_ms = (wait_ms + slow_loop_ms as u64) / 2; // Slow down
                            continue;
                        };
                        drop(q); // Unlock mutex
                                 
                        let Ok((files, folders)) = get_files_and_folders_at(search_path) else {
                            return;
                        };

                        let new_queue: Vec<PathBuf> = folders.into_iter()
                             // Don't search inside folders that start with "." (like .git/)
                             .filter(|pb| !path2string(pb.file_name().unwrap_or_default()) .starts_with('.') )
                             .collect();

                        let results: Vec<PathBuf> = files.into_iter()
                            .filter_map(|pb| pb.strip_prefix(&root).ok() .map(PathBuf::from) )
                            .filter(|pb| {
                                !ignore_types.contains(&path2string(pb.extension().unwrap_or_default()))
                                && path2string(pb).to_lowercase().contains(&query)
                            })
                            .collect();

                        if result_sender.send(QueryResults::Results { results, new_queue, query_len: query.len() })
                            .is_err() {
                            break;
                        }

                        // wait_ms = fast_loop_ms as u64; // Speed up
                        wait_ms = (wait_ms + fast_loop_ms as u64) / 2; // Speed up
                    }
                });
            }

            self.bundle = Some(bundle);
        }

        fn try_update_query(&mut self, new_query: &str) -> Result<(), TryUpdateQueryError> {
            // No query
            let Some(bundle) = &mut self.bundle else {
                return Err(TryUpdateQueryError::NoBundle);
            };
            // new_query is not an extension of current query
            if new_query.len() <= bundle.query.len() {
                return Err(TryUpdateQueryError::QueryNotSubset);
            }

            // Notify threads of query change
            if let Err(err) = bundle.change_query(new_query) {
                return Err(TryUpdateQueryError::NotifyError( err ));
            }

            // Filter results
            self.results.retain(|IndexedString(_, s)| s.to_lowercase() .contains(new_query));

            Ok(())
        }
    }

    impl SearchQuery for SearchFiles {
        fn get_list(&self) -> Vec<IndexedString> {
            let max: usize = self.max_result_count;
            let mut results: Vec<PathBuf> = Vec::new();
            let mut queue: VecDeque<PathBuf> = VecDeque::from([ self.root.clone() ]);

            while results.len() < max {
                let Some(search_path) = queue.pop_front() else { break; };
                let Ok((mut files, folders)) = get_files_and_folders_at(search_path) else {
                    continue;
                };

                results.append(&mut files);
                queue.append(&mut folders.into());
                if let Some(max_queue_len) = self.max_queue_len {
                    queue.truncate(max_queue_len);
                }
            }

            results.iter()
                .take(max)
                .enumerate()
                .map(IndexedString::from)
                .collect()
        }

        fn search(&mut self, query: &str) {
            if query.is_empty() {
                self.results = self.get_list();
                self.bundle = None;
                return;
            }

            // Epic plan, once again

            // Try to filter & notify, if that fails, reset & new
            match self.try_update_query(query) {
                Ok(()) => {},
                Err(_) => self.new_query(query),
            }
        }

        fn update(&mut self) {
            let Some(bundle) = &mut self.bundle else { return; };
            let rx = &bundle.receiver;
            let current_query_len: usize = bundle.query.len();

            let max: usize = self.max_result_count;
            for res in rx.try_iter() {
                match res {
                    QueryResults::ConnectionCheck => {},
                    QueryResults::Results { results, new_queue, query_len } => {
                        // Append queue
                        if !new_queue.is_empty() {
                            let Ok(mut q) = bundle.queue.lock() else { continue; };

                            q.append(&mut VecDeque::from(new_queue));
                            if let Some(max_queue_len) = self.max_queue_len {
                                q.truncate(max_queue_len);
                            }
                        }

                        // Append results
                        let l: usize = self.results.len();
                        if results.is_empty() || l >= max { continue; }
                        let r = results.into_iter().enumerate()
                            .map(|(i, pb)| IndexedString::from( (l + i, &pb) ));
                        // Same query
                        if query_len == current_query_len {
                            self.results.append(&mut r
                                .filter(|is| !self.results.contains(is)) // Check for duplicates
                                .collect())
                        // Probably outdated
                        } else {
                            self.results.append(&mut r
                                .filter(|is| !self.results.contains(is) && is.as_ref().contains(&bundle.query)) //  Also check for query
                                .collect())
                        }
                        self.results.truncate(max);
                    },
                }
            }

            if self.results.len() >= max {
                self.bundle = None;
            }
        }

        fn get_results(&self) -> &Vec<IndexedString> {
            &self.results
        }

        fn iter_display(&self) -> Box<dyn Iterator<Item = String> + '_> {
            Box::new(self.get_results().iter()
                .map(|IndexedString(_, s)| s.replace('\\', "/"))
            )
        }
    }



}



