use crossterm::event::{KeyCode, KeyEvent};
use ratatui::widgets::ListState;
use std::sync::Arc;
use std::time::{Duration, Instant};
use uwu_engine::SystemEngine;
use uwu_schema::LogEvent;

pub enum InputMode {
    Normal,
    Editing,
}

pub struct App {
    pub engine: Arc<SystemEngine>,
    pub display_limit: usize,
    pub query: String,
    pub input_mode: InputMode,
    pub list_state: ListState,
    pub is_auto_scroll: bool,
    pub total_matched: usize,
    pub cached_logs: Vec<LogEvent>,
    pub last_query: String,
    pub last_processed_count: u64,
    pub last_search_time: Instant,
}

impl App {
    pub fn new(engine: Arc<SystemEngine>, display_limit: usize) -> Self {
        let (total_matched, cached_logs) = engine.search_with_count("", display_limit);
        let mut list_state = ListState::default();
        if !cached_logs.is_empty() {
            list_state.select(Some(cached_logs.len() - 1));
        }

        Self {
            engine,
            display_limit,
            query: String::new(),
            input_mode: InputMode::Normal,
            list_state,
            is_auto_scroll: true,
            total_matched,
            cached_logs,
            last_query: String::new(),
            last_processed_count: 0,
            last_search_time: Instant::now(),
        }
    }

    pub fn tick(&mut self) {
        let total_processed = self.engine.total_processed();
        let now = Instant::now();

        let query_changed = self.query != self.last_query;
        let new_logs_arrived = total_processed != self.last_processed_count
            && now.duration_since(self.last_search_time) > Duration::from_millis(200);

        let need_search = query_changed || (self.is_auto_scroll && new_logs_arrived);

        if need_search {
            let (matched, logs) = self
                .engine
                .search_with_count(&self.query, self.display_limit);
            self.total_matched = matched;
            self.cached_logs = logs;
            self.last_query = self.query.clone();
            self.last_processed_count = total_processed;
            self.last_search_time = now;

            if self.is_auto_scroll && !self.cached_logs.is_empty() {
                self.list_state.select(Some(self.cached_logs.len() - 1));
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        let displayed_count = self.cached_logs.len();

        match self.input_mode {
            InputMode::Normal => match key.code {
                KeyCode::Char('q') => return true, // Exit signal
                KeyCode::Char('/') | KeyCode::Char('i') => {
                    self.input_mode = InputMode::Editing;
                }
                KeyCode::Char(' ') | KeyCode::Char('p') => {
                    self.is_auto_scroll = !self.is_auto_scroll;
                    if self.is_auto_scroll && !self.cached_logs.is_empty() {
                        let (matched, logs) = self
                            .engine
                            .search_with_count(&self.query, self.display_limit);
                        self.total_matched = matched;
                        self.cached_logs = logs;
                        self.last_processed_count = self.engine.total_processed();
                        self.last_search_time = Instant::now();
                        self.list_state.select(Some(self.cached_logs.len() - 1));
                    }
                }
                KeyCode::Up => {
                    let current_idx = self.list_state.selected().unwrap_or(0);
                    if current_idx > 0 {
                        self.list_state.select(Some(current_idx - 1));
                        self.is_auto_scroll = false;
                    }
                }
                KeyCode::Down => {
                    let current_idx = self.list_state.selected().unwrap_or(0);
                    if current_idx < displayed_count.saturating_sub(1) {
                        let new_idx = current_idx + 1;
                        self.list_state.select(Some(new_idx));
                        self.is_auto_scroll = new_idx == displayed_count.saturating_sub(1);
                    }
                }
                KeyCode::Home | KeyCode::Char('g') if displayed_count > 0 => {
                    self.list_state.select(Some(0));
                    self.is_auto_scroll = false;
                }
                KeyCode::End | KeyCode::Char('G') if displayed_count > 0 => {
                    self.is_auto_scroll = true;
                    let (matched, logs) = self
                        .engine
                        .search_with_count(&self.query, self.display_limit);
                    self.total_matched = matched;
                    self.cached_logs = logs;
                    self.last_processed_count = self.engine.total_processed();
                    self.last_search_time = Instant::now();
                    self.list_state.select(Some(self.cached_logs.len() - 1));
                }
                KeyCode::PageUp => {
                    let current_idx = self.list_state.selected().unwrap_or(0);
                    let new_idx = current_idx.saturating_sub(20);
                    self.list_state.select(Some(new_idx));
                    self.is_auto_scroll = false;
                }
                KeyCode::PageDown => {
                    let current_idx = self.list_state.selected().unwrap_or(0);
                    let max_idx = displayed_count.saturating_sub(1);
                    let new_idx = (current_idx + 20).min(max_idx);
                    self.list_state.select(Some(new_idx));
                    self.is_auto_scroll = new_idx == max_idx;
                }
                _ => {}
            },
            InputMode::Editing => match key.code {
                KeyCode::Enter | KeyCode::Esc => {
                    self.input_mode = InputMode::Normal;
                }
                KeyCode::Char(c) => {
                    self.query.push(c);
                    self.is_auto_scroll = true;
                }
                KeyCode::Backspace => {
                    self.query.pop();
                    self.is_auto_scroll = true;
                }
                _ => {}
            },
        }

        false
    }
}
