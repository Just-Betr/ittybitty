mod app;
mod ui;

use std::{io, path::PathBuf, time::Duration};

use anyhow::{Context, Result};
use crossterm::{
    event::{DisableBracketedPaste, EnableBracketedPaste},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use directories::UserDirs;
use librqbit::{Api, Session, SessionOptions, SessionPersistenceConfig};
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::select;

use crate::app::{App, start_event_thread};

#[tokio::main]
async fn main() -> Result<()> {
    let download_dir = default_download_dir();
    let session = Session::new_with_opts(
        download_dir.clone(),
        SessionOptions {
            fastresume: true,
            persistence: Some(SessionPersistenceConfig::Json { folder: None }),
            ..Default::default()
        },
    )
    .await
    .context("failed to create rqbit session")?;
    let api = Api::new(session.clone(), None);

    let mut app = App::new(api, download_dir);
    app.refresh();

    setup_terminal()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    let mut events = start_event_thread();
    let mut tick = tokio::time::interval(Duration::from_millis(500));

    let mut should_quit = false;

    while !should_quit {
        terminal.draw(|frame| ui::draw(frame, &app))?;

        select! {
            _ = tick.tick() => {
                app.refresh();
            }
            Some(ev) = events.recv() => {
                match app.handle_event(ev).await {
                    Ok(quit) => should_quit = quit,
                    Err(err) => app.set_error(err),
                }
            }
        }
    }

    restore_terminal()?;
    Ok(())
}

fn setup_terminal() -> Result<()> {
    enable_raw_mode().context("failed to enable raw mode")?;
    execute!(io::stdout(), EnterAlternateScreen, EnableBracketedPaste)
        .context("failed to enter alt screen")?;
    Ok(())
}

fn restore_terminal() -> Result<()> {
    disable_raw_mode().context("failed to disable raw mode")?;
    execute!(io::stdout(), DisableBracketedPaste, LeaveAlternateScreen)
        .context("failed to leave alt screen")?;
    Ok(())
}

fn default_download_dir() -> PathBuf {
    UserDirs::new()
        .and_then(|dirs| dirs.download_dir().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}
