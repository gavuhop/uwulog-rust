mod app;
mod ui;

use anyhow::Result;
use app::App;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Write};
use std::sync::Arc;
use std::time::Duration;
use uwu_engine::SystemEngine;
use uwu_sources::{FileSource, ProcessSource};

#[cfg(target_os = "windows")]
use uwu_sources::WinEventSource;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut display_limit: usize = 5_000;
    let mut capacity: usize = 50_000;

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

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(engine, display_limit);
    let res = run_app(&mut terminal, &mut app).await;

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

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    // Vẽ frame khởi tạo ban đầu
    terminal.draw(|f| ui::render(f, app))?;
    app.should_redraw = false;

    loop {
        app.tick();

        // CHỈ VẼ LẠI MÀN HÌNH KHI CÓ SỰ THAY ĐỔI THỰC SỰ (Event-driven Reactive Render)
        if app.should_redraw {
            terminal.draw(|f| ui::render(f, app))?;
            app.should_redraw = false;
        }

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press && app.handle_key(key) {
                    return Ok(());
                }
            }
        }
    }
}
