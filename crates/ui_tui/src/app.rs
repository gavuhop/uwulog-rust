use crossterm::event::{KeyCode, KeyEvent};
use ratatui::widgets::ListState;
use std::sync::Arc;
use std::time::{Duration, Instant};
use uwu_core_engine::SystemEngine;
use uwu_core_schema::LogEvent;

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
    pub should_redraw: bool,
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
            should_redraw: true,
        }
    }

    pub fn refresh_search(&mut self) {
        let (matched, logs) = self
            .engine
            .search_with_count(&self.query, self.display_limit);
        self.total_matched = matched;
        self.cached_logs = logs;
        self.last_processed_count = self.engine.total_processed();
        self.last_search_time = Instant::now();

        if self.is_auto_scroll && !self.cached_logs.is_empty() {
            self.list_state.select(Some(self.cached_logs.len() - 1));
        }
    }

    pub fn tick(&mut self) {
        let total_processed = self.engine.total_processed();
        let now = Instant::now();

        let query_changed = self.query != self.last_query;
        let new_logs_arrived = total_processed != self.last_processed_count
            && now.duration_since(self.last_search_time) > Duration::from_millis(200);

        if query_changed {
            // 1. Khi từ khóa thay đổi (đang gõ): Chạy Full Search 1 lần trên dữ liệu RingBuffer
            self.last_query = self.query.clone();
            self.refresh_search();
            self.should_redraw = true;
        } else if self.is_auto_scroll && new_logs_arrived {
            // 2. Khi log mới streaming về (từ khóa không đổi): Lọc TĂNG TIẾN (Incremental) CHỈ trên log mới về!
            let (new_matched_count, new_matching_logs) = self
                .engine
                .filter_incremental(&self.query, self.last_processed_count);

            if new_matched_count > 0 {
                self.total_matched += new_matched_count;
                self.cached_logs.extend(new_matching_logs);

                if self.cached_logs.len() > self.display_limit {
                    let overflow = self.cached_logs.len() - self.display_limit;
                    self.cached_logs.drain(0..overflow);
                }

                if !self.cached_logs.is_empty() {
                    self.list_state.select(Some(self.cached_logs.len() - 1));
                }
                self.should_redraw = true;
            }

            self.last_processed_count = total_processed;
            self.last_search_time = now;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        self.should_redraw = true;
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
                        self.refresh_search();
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
                    self.refresh_search();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventState, KeyModifiers};
    use uwu_core_schema::{RawLogEntry, RawPayload};

    fn make_key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    async fn create_test_tui_app(item_count: usize) -> App {
        let engine = Arc::new(SystemEngine::new(100));
        let tx = engine.get_channel();

        for i in 0..item_count {
            tx.send(RawLogEntry {
                payload: RawPayload::Text(format!("[INFO] Log row {}", i)),
            })
            .await
            .unwrap();
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
        App::new(engine, 50)
    }

    #[tokio::test]
    async fn test_tui_app_init() {
        let app = create_test_tui_app(5).await;
        assert_eq!(app.total_matched, 5);
        assert_eq!(app.cached_logs.len(), 5);
        assert!(app.is_auto_scroll);
        assert_eq!(app.list_state.selected(), Some(4));
    }

    #[tokio::test]
    async fn test_tui_app_navigation_keys() {
        let mut app = create_test_tui_app(10).await;
        assert_eq!(app.list_state.selected(), Some(9));

        // Up: moves to 8, turns off auto_scroll
        app.handle_key(make_key(KeyCode::Up));
        assert_eq!(app.list_state.selected(), Some(8));
        assert!(!app.is_auto_scroll);

        // Home: moves to 0
        app.handle_key(make_key(KeyCode::Home));
        assert_eq!(app.list_state.selected(), Some(0));

        // Down: moves to 1
        app.handle_key(make_key(KeyCode::Down));
        assert_eq!(app.list_state.selected(), Some(1));

        // End: moves to bottom (9), re-enables auto_scroll
        app.handle_key(make_key(KeyCode::End));
        assert_eq!(app.list_state.selected(), Some(9));
        assert!(app.is_auto_scroll);
    }

    #[tokio::test]
    async fn test_tui_app_editing_mode_keys() {
        let mut app = create_test_tui_app(5).await;

        // Press '/' -> enter editing mode
        app.handle_key(make_key(KeyCode::Char('/')));
        assert!(matches!(app.input_mode, InputMode::Editing));

        // Type query 'e', 'r', 'r'
        app.handle_key(make_key(KeyCode::Char('e')));
        app.handle_key(make_key(KeyCode::Char('r')));
        app.handle_key(make_key(KeyCode::Char('r')));
        assert_eq!(app.query, "err");

        // Backspace -> 'er'
        app.handle_key(make_key(KeyCode::Backspace));
        assert_eq!(app.query, "er");

        // Enter -> return to normal mode
        app.handle_key(make_key(KeyCode::Enter));
        assert!(matches!(app.input_mode, InputMode::Normal));
    }

    #[tokio::test]
    async fn test_tui_app_latch_toggle_keys() {
        let mut app = create_test_tui_app(5).await;
        assert!(app.is_auto_scroll);

        // Press Space -> pause
        app.handle_key(make_key(KeyCode::Char(' ')));
        assert!(!app.is_auto_scroll);

        // Press 'p' -> resume
        app.handle_key(make_key(KeyCode::Char('p')));
        assert!(app.is_auto_scroll);
    }

    #[tokio::test]
    async fn test_tui_app_tick_query_changed() {
        let mut app = create_test_tui_app(5).await;
        app.query = "level:warn".to_string();

        app.tick();
        assert_eq!(app.last_query, "level:warn");
        assert_eq!(app.total_matched, 0);
    }

    #[tokio::test]
    async fn test_tui_app_exit_key() {
        let mut app = create_test_tui_app(5).await;
        let should_exit = app.handle_key(make_key(KeyCode::Char('q')));
        assert!(should_exit);
    }
}
