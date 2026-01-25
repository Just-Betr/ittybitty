use std::{io, path::PathBuf, time::Duration};

use anyhow::{Context, Result};
use directories::UserDirs;
use librqbit::{Api, Session, SessionOptions, SessionPersistenceConfig};
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::select;

use ittybitty::{app::App, events::start_event_thread, tui};

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

    tui::setup_terminal()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    let mut events = start_event_thread();
    let mut tick = tokio::time::interval(Duration::from_millis(500));

    let mut should_quit = false;

    while !should_quit {
        terminal.draw(|frame| ittybitty::ui::draw(frame, &app))?;

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

    tui::restore_terminal()?;
    Ok(())
}

fn default_download_dir() -> PathBuf {
    UserDirs::new()
        .and_then(|dirs| dirs.download_dir().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}
