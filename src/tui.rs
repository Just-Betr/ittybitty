use std::io;

use anyhow::{Context, Result};
use crossterm::{
    event::{DisableBracketedPaste, EnableBracketedPaste},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};

pub fn setup_terminal() -> Result<()> {
    enable_raw_mode().context("failed to enable raw mode")?;
    execute!(io::stdout(), EnterAlternateScreen, EnableBracketedPaste)
        .context("failed to enter alt screen")?;
    Ok(())
}

pub fn restore_terminal() -> Result<()> {
    disable_raw_mode().context("failed to disable raw mode")?;
    execute!(io::stdout(), DisableBracketedPaste, LeaveAlternateScreen)
        .context("failed to leave alt screen")?;
    Ok(())
}
