use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};
use std::io::{self, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};
use uwu_engine::SystemEngine;
use uwu_schema::LogLevel;
#[cfg(target_os = "windows")]
use uwu_sources::WinEventSource;
use uwu_sources::{FileSource, ProcessSource};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut display_limit: usize = 5_000;
    let mut capacity: usize = 50_000;

    // Lấy capacity và display_limit từ tham số dòng lệnh nếu có
    let mut i = 1;
    while i < args.len() {
        if (args[i] == "-n" || args[i] == "--limit") && i + 1 < args.len() {
            if let Ok(val) = args[i + 1].parse::<usize>() {
                display_limit = val;
            }
        } else if (args[i] == "-cap" || args[i] == "--capacity") && i + 1 < args.len() {
            if let Ok(val) = args[i + 1].parse::<usize>() {
                capacity = val;
            }
        }
        i += 1;
    }

    let engine = Arc::new(SystemEngine::new(capacity));
    let mut added_custom_source = false;

    // Parser tham số dòng lệnh nguồn log
    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        if arg == "-r" || arg == "--run" || arg == "-c" || arg == "--cmd" {
            if i + 1 < args.len() {
                let cmd_str = &args[i + 1];
                let cmd_args: Vec<String> = args[i + 2..].to_vec();

                let (prog, proc_args) = if cmd_args.is_empty() && cmd_str.contains(' ') {
                    let parts: Vec<&str> = cmd_str.split_whitespace().collect();
                    (
                        parts[0].to_string(),
                        parts[1..].iter().map(|s| s.to_string()).collect(),
                    )
                } else {
                    (cmd_str.clone(), cmd_args)
                };

                let _ = engine
                    .add_source(Box::new(ProcessSource::new(prog, proc_args)))
                    .await;
                added_custom_source = true;
                break;
            }
        } else if arg == "-f" || arg == "--file" {
            if i + 1 < args.len() {
                let file_path = &args[i + 1];
                let _ = engine
                    .add_source(Box::new(FileSource::new(file_path)))
                    .await;
                added_custom_source = true;
                i += 1;
            }
        } else if !arg.starts_with('-')
            && i > 0
            && args[i - 1] != "-n"
            && args[i - 1] != "--limit"
            && args[i - 1] != "-cap"
            && args[i - 1] != "--capacity"
        {
            let _ = engine.add_source(Box::new(FileSource::new(arg))).await;
            added_custom_source = true;
        }
        i += 1;
    }

    if !added_custom_source {
        #[cfg(target_os = "windows")]
        {
            let _ = engine
                .add_source(Box::new(WinEventSource::new("System")))
                .await;
            let _ = engine
                .add_source(Box::new(WinEventSource::new("Application")))
                .await;
        }
    }

    // Cấu hình Terminal raw mode cho Ratatui
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Vòng lặp giao diện TUI siêu mượt
    let res = run_tui(&mut terminal, engine, display_limit).await;

    // Khôi phục hoàn toàn trạng thái Terminal chuẩn xác
    let _ = disable_raw_mode();
    let _ = execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    );
    let _ = terminal.show_cursor();
    let _ = io::stdout().flush();

    if let Err(err) = res {
        eprintln!("TUI Error: {:?}", err);
    }

    std::process::exit(0);
}

enum InputMode {
    Normal,
    Editing,
}

async fn run_tui<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    engine: Arc<SystemEngine>,
    display_limit: usize,
) -> io::Result<()> {
    let mut query = String::new();
    let mut input_mode = InputMode::Normal;
    let mut list_state = ListState::default();

    // Chế độ Auto Scroll Tail mode (mặc định true: tự động cuộn theo log mới ở dưới cùng)
    let mut is_auto_scroll = true;

    let (mut total_matched, mut cached_logs) = engine.search_with_count(&query, display_limit);
    let mut last_query = String::new();
    let mut last_processed_count = engine.total_processed();
    let mut last_search_time = Instant::now();

    if !cached_logs.is_empty() {
        list_state.select(Some(cached_logs.len() - 1));
    }

    loop {
        let total_logs = engine.total_logs();
        let total_processed = engine.total_processed();
        let now = Instant::now();

        let query_changed = query != last_query;
        let new_logs_arrived = total_processed != last_processed_count
            && now.duration_since(last_search_time) > Duration::from_millis(200);

        // ĐỐI VỚI CHẾ ĐỘ QUAN SÁT (is_auto_scroll == false): KHÔNG BAO GIỜ RE-SEARCH HAY THAY ĐỔI CACHED_LOGS!
        // Giúp đóng băng 100% giao diện, không bị trôi kể cả khi RAM tràn 50,000 log.
        let need_search = query_changed || (is_auto_scroll && new_logs_arrived);

        if need_search {
            let (matched, logs) = engine.search_with_count(&query, display_limit);
            total_matched = matched;
            cached_logs = logs;
            last_query = query.clone();
            last_processed_count = total_processed;
            last_search_time = now;

            // Nếu đang ở Auto-scroll mode, tự động chọn dòng dưới cùng (mới nhất)
            if is_auto_scroll && !cached_logs.is_empty() {
                list_state.select(Some(cached_logs.len() - 1));
            }
        }

        let displayed_count = cached_logs.len();
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints(
                    [
                        Constraint::Length(3),
                        Constraint::Min(5),
                        Constraint::Length(1),
                    ]
                    .as_ref(),
                )
                .split(f.area());

            let input_style = match input_mode {
                InputMode::Normal => Style::default().fg(Color::Gray),
                InputMode::Editing => Style::default().fg(Color::Yellow),
            };

            let input_title = match input_mode {
                InputMode::Normal => " Filter Query (Nhấn '/' để lọc, Esc để thoát) ",
                InputMode::Editing => " Editing Filter Query (Nhấn Enter/Esc để hoàn tất) ",
            };

            let input_widget = Paragraph::new(query.as_str())
                .style(input_style)
                .block(Block::default().borders(Borders::ALL).title(input_title));
            f.render_widget(input_widget, chunks[0]);

            let items: Vec<ListItem> = cached_logs
                .iter()
                .map(|log| {
                    let level_color = match log.level {
                        LogLevel::Error | LogLevel::Fatal => Color::Red,
                        LogLevel::Warn => Color::Yellow,
                        LogLevel::Info => Color::Green,
                        LogLevel::Debug => Color::Cyan,
                        LogLevel::Trace => Color::Magenta,
                        LogLevel::Unknown => Color::Gray,
                    };

                    let time_str = log.timestamp.format("%H:%M:%S%.3f").to_string();
                    let line_content = format!(
                        "[{}] [{:<5}] [{}] {}",
                        time_str,
                        log.level.to_string(),
                        log.source_id,
                        log.message
                    );

                    ListItem::new(line_content).style(Style::default().fg(level_color))
                })
                .collect();

            let mode_tag = if is_auto_scroll {
                " [LIVE AUTO-SCROLL (TAIL -F)] "
            } else {
                " [ĐÃ ĐÓNG BĂNG MÀN HÌNH QUAN SÁT - FROZEN] "
            };

            let title_text = if total_matched > displayed_count {
                format!(
                    " Live Logs (Khớp: {} | Hiển thị: {} log mới nhất | RAM: {}){} ",
                    total_matched, displayed_count, total_logs, mode_tag
                )
            } else {
                format!(" Live Logs (Khớp: {} | RAM: {}){} ", total_matched, total_logs, mode_tag)
            };

            let log_list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(title_text))
                .highlight_style(Style::default().add_modifier(Modifier::BOLD).bg(Color::Rgb(40, 40, 70)));
            f.render_stateful_widget(log_list, chunks[1], &mut list_state);

            let status_text = match input_mode {
                InputMode::Normal => {
                    if is_auto_scroll {
                        " [LIVE TAIL] Tự động cuộn theo log mới ở dưới cùng | [Up/Dn]: Cuộn quan sát log cũ | [/]: Tìm kiếm | [Q]: Thoát "
                    } else {
                        " [FROZEN] ĐÃ ĐÓNG BĂNG MÀN HÌNH ĐỂ QUAN SÁT | Nhấn [End/G/Space] để bật lại Live Tail | [Up/Dn]: Di chuyển "
                    }
                }
                InputMode::Editing => " [Enter/Esc]: Đóng ô nhập từ khóa ",
            };
            let status_bar = Paragraph::new(status_text)
                .style(Style::default().bg(if is_auto_scroll { Color::Blue } else { Color::Rgb(180, 100, 0) }).fg(Color::White));
            f.render_widget(status_bar, chunks[2]);
        })?;

        if event::poll(Duration::from_millis(30))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match input_mode {
                    InputMode::Normal => match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('/') | KeyCode::Char('i') => {
                            input_mode = InputMode::Editing;
                        }
                        KeyCode::Char(' ') | KeyCode::Char('p') => {
                            // Phím Space hoặc p để Bật/Tắt Auto-scroll
                            is_auto_scroll = !is_auto_scroll;
                            if is_auto_scroll && !cached_logs.is_empty() {
                                let (matched, logs) =
                                    engine.search_with_count(&query, display_limit);
                                total_matched = matched;
                                cached_logs = logs;
                                last_processed_count = total_processed;
                                last_search_time = Instant::now();
                                list_state.select(Some(cached_logs.len() - 1));
                            }
                        }
                        KeyCode::Up => {
                            let current_idx = list_state.selected().unwrap_or(0);
                            if current_idx > 0 {
                                list_state.select(Some(current_idx - 1));
                                is_auto_scroll = false; // Đóng băng view ngay khi cuộn lên xem log cũ
                            }
                        }
                        KeyCode::Down => {
                            let current_idx = list_state.selected().unwrap_or(0);
                            if current_idx < displayed_count.saturating_sub(1) {
                                let new_idx = current_idx + 1;
                                list_state.select(Some(new_idx));
                                if new_idx == displayed_count.saturating_sub(1) {
                                    is_auto_scroll = true; // Xuống tới tận cùng -> tự động bật lại Live Auto-scroll
                                } else {
                                    is_auto_scroll = false;
                                }
                            }
                        }
                        KeyCode::Home | KeyCode::Char('g') if displayed_count > 0 => {
                            list_state.select(Some(0));
                            is_auto_scroll = false; // Nhảy lên đầu (log cũ nhất) -> Đóng băng view
                        }
                        KeyCode::End | KeyCode::Char('G') if displayed_count > 0 => {
                            is_auto_scroll = true; // Nhảy xuống cuối (log mới nhất) -> Bật lại Live Tail
                            let (matched, logs) = engine.search_with_count(&query, display_limit);
                            total_matched = matched;
                            cached_logs = logs;
                            last_processed_count = total_processed;
                            last_search_time = Instant::now();
                            list_state.select(Some(cached_logs.len() - 1));
                        }
                        KeyCode::PageUp => {
                            let current_idx = list_state.selected().unwrap_or(0);
                            let new_idx = current_idx.saturating_sub(20);
                            list_state.select(Some(new_idx));
                            is_auto_scroll = false;
                        }
                        KeyCode::PageDown => {
                            let current_idx = list_state.selected().unwrap_or(0);
                            let max_idx = displayed_count.saturating_sub(1);
                            let new_idx = (current_idx + 20).min(max_idx);
                            list_state.select(Some(new_idx));
                            is_auto_scroll = new_idx == max_idx;
                        }
                        _ => {}
                    },
                    InputMode::Editing => match key.code {
                        KeyCode::Enter | KeyCode::Esc => {
                            input_mode = InputMode::Normal;
                        }
                        KeyCode::Char(c) => {
                            query.push(c);
                            is_auto_scroll = true;
                        }
                        KeyCode::Backspace => {
                            query.pop();
                            is_auto_scroll = true;
                        }
                        _ => {}
                    },
                }
            }
        }
    }
}
