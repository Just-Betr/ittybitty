use std::{borrow::Cow, path::PathBuf, time::{Duration, Instant}};

use anyhow::{Context, Result, anyhow};
use bytes::Bytes;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use librqbit::{
    AddTorrent, AddTorrentOptions,
    api::{Api, ApiAddTorrentResponse, ApiTorrentListOpts, TorrentDetailsResponse, TorrentStats},
    session_stats::snapshot::SessionStatsSnapshot,
};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    EnterMagnet,
    EnterTorrentDir,
    FilePicker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Torrents,
    Peers,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPanel {
    Filters,
    Torrents,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterKind {
    All,
    Downloading,
    Seeding,
    Paused,
    Stopped,
    Error,
}

const FILTERS: [FilterKind; 6] = [
    FilterKind::All,
    FilterKind::Downloading,
    FilterKind::Seeding,
    FilterKind::Paused,
    FilterKind::Stopped,
    FilterKind::Error,
];

#[derive(Debug)]
pub struct TorrentRow {
    pub id: usize,
    pub name: String,
    pub info_hash: Option<String>,
    pub output_folder: String,
    pub stats: Option<TorrentStats>,
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub length: u64,
    pub included: bool,
}

#[derive(Debug, Clone)]
pub struct FilePickerState {
    pub magnet: String,
    pub output_folder: String,
    pub files: Vec<FileEntry>,
    pub cursor: usize,
}

pub struct App {
    api: Api,
    torrents: Vec<TorrentRow>,
    selected: usize,
    mode: Mode,
    input: String,
    input_cursor: usize,
    suppress_next_input: bool,
    suppress_paste_chars: usize,
    last_char_at: Option<Instant>,
    download_dir: PathBuf,
    status: String,
    last_error: Option<String>,
    file_picker: Option<FilePickerState>,
    session_stats: Option<SessionStatsSnapshot>,
    view: View,
    confirm_delete: bool,
    delete_choice: bool,
    confirm_quit: bool,
    quit_choice: bool,
    focus: FocusPanel,
    filter_index: usize,
    pending_add_input: Option<String>,
    show_help: bool,
    help_scroll: u16,
}

impl App {
    pub fn new(api: Api, download_dir: PathBuf) -> Self {
        Self {
            api,
            torrents: Vec::new(),
            selected: 0,
            mode: Mode::Normal,
            input: String::new(),
            input_cursor: 0,
            suppress_next_input: false,
            suppress_paste_chars: 0,
            last_char_at: None,
            download_dir,
            status: "Ready".to_string(),
            last_error: None,
            file_picker: None,
            session_stats: None,
            view: View::Torrents,
            confirm_delete: false,
            delete_choice: false,
            confirm_quit: false,
            quit_choice: false,
            focus: FocusPanel::Torrents,
            filter_index: 0,
            pending_add_input: None,
            show_help: false,
            help_scroll: 0,
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn torrents(&self) -> &[TorrentRow] {
        &self.torrents
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn input_cursor(&self) -> usize {
        self.input_cursor
    }

    pub fn file_picker(&self) -> Option<&FilePickerState> {
        self.file_picker.as_ref()
    }

    pub fn selected_torrent(&self) -> Option<&TorrentRow> {
        self.torrents.get(self.selected)
    }

    pub fn view(&self) -> View {
        self.view
    }

    pub fn confirm_delete(&self) -> bool {
        self.confirm_delete
    }

    pub fn delete_choice(&self) -> bool {
        self.delete_choice
    }

    pub fn show_help(&self) -> bool {
        self.show_help
    }

    pub fn help_scroll(&self) -> u16 {
        self.help_scroll
    }

    pub fn confirm_quit(&self) -> bool {
        self.confirm_quit
    }

    pub fn quit_choice(&self) -> bool {
        self.quit_choice
    }

    pub fn focus(&self) -> FocusPanel {
        self.focus
    }

    pub fn selected_filter(&self) -> FilterKind {
        FILTERS
            .get(self.filter_index)
            .copied()
            .unwrap_or(FilterKind::All)
    }

    pub fn filtered_indices(&self) -> Vec<usize> {
        self.torrents
            .iter()
            .enumerate()
            .filter_map(|(idx, t)| {
                if self.filter_match(t) {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn session_stats(&self) -> Option<&SessionStatsSnapshot> {
        self.session_stats.as_ref()
    }

    pub fn set_error(&mut self, err: impl ToString) {
        self.last_error = Some(err.to_string());
        self.status = "Error".to_string();
        self.mode = Mode::Normal;
        self.file_picker = None;
        self.confirm_delete = false;
        self.confirm_quit = false;
        self.show_help = false;
        self.help_scroll = 0;
        self.pending_add_input = None;
        self.input.clear();
        self.input_cursor = 0;
        self.suppress_next_input = false;
        self.suppress_paste_chars = 0;
        self.last_char_at = None;
    }

    pub fn clear_error(&mut self) {
        self.last_error = None;
        self.status = "Ready".to_string();
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn refresh(&mut self) {
        let selected_id = self.selected_torrent().map(|t| t.id);
        self.session_stats = Some(self.api.api_session_stats());
        let list = self
            .api
            .api_torrent_list_ext(ApiTorrentListOpts { with_stats: true });
        let rows: Vec<TorrentRow> = list
            .torrents
            .into_iter()
            .filter_map(|t| to_row(t).ok())
            .collect();
        if rows.is_empty() {
            self.selected = 0;
        } else if let Some(id) = selected_id {
            if let Some(idx) = rows.iter().position(|r| r.id == id) {
                self.selected = idx;
            } else {
                self.selected = self.selected.min(rows.len() - 1);
            }
        } else {
            self.selected = self.selected.min(rows.len().saturating_sub(1));
        }
        self.torrents = rows;
        self.ensure_selection_for_filter();
    }

    pub async fn handle_event(&mut self, ev: Event) -> Result<bool> {
        match ev {
            Event::Key(key) => self.handle_key(key).await,
            Event::Paste(text) => {
                if matches!(self.mode, Mode::EnterMagnet | Mode::EnterTorrentDir) {
                    self.insert_text(&text);
                    self.suppress_paste_chars = text.chars().count();
                } else {
                    self.suppress_paste_chars = text.chars().count();
                    self.last_char_at = Some(Instant::now());
                    self.status = "Paste ignored".to_string();
                }
                Ok(false)
            }
            Event::Resize(_, _) => Ok(false),
            _ => Ok(false),
        }
    }

    async fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        if key.kind != KeyEventKind::Press {
            return Ok(false);
        }
        if matches!(self.mode, Mode::Normal) {
            match key.code {
                KeyCode::Char(_) => {
                    if self.should_ignore_paste_char() {
                        return Ok(false);
                    }
                }
                _ => {
                    self.last_char_at = None;
                }
            }
        }
        if self.suppress_paste_chars > 0 {
            self.suppress_paste_chars -= 1;
            return Ok(false);
        }
        if self.show_help {
            match key.code {
                KeyCode::Char('?') | KeyCode::Char('x') | KeyCode::Esc => {
                    self.show_help = false;
                    self.help_scroll = 0;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.help_scroll = self.help_scroll.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.help_scroll = self.help_scroll.saturating_add(1);
                }
                KeyCode::PageUp => {
                    self.help_scroll = self.help_scroll.saturating_sub(5);
                }
                KeyCode::PageDown => {
                    self.help_scroll = self.help_scroll.saturating_add(5);
                }
                KeyCode::Home => {
                    self.help_scroll = 0;
                }
                KeyCode::End => {
                    self.help_scroll = self.help_scroll.saturating_add(100);
                }
                _ => {}
            }
            return Ok(false);
        }
        if self.last_error.is_some() {
            if matches!(key.code, KeyCode::Char('x') | KeyCode::Esc) {
                self.clear_error();
            }
            return Ok(false);
        }
        if self.confirm_delete {
            match key.code {
                KeyCode::Left => {
                    self.delete_choice = true;
                }
                KeyCode::Right => {
                    self.delete_choice = false;
                }
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.delete_choice = true;
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.delete_choice = false;
                }
                KeyCode::Esc => {
                    self.confirm_delete = false;
                    self.delete_choice = false;
                    self.status = "Delete cancelled".to_string();
                }
                KeyCode::Enter => {
                    if self.delete_choice {
                        self.confirm_delete = false;
                        self.delete_choice = false;
                        self.delete_selected_files().await?;
                    } else {
                        self.confirm_delete = false;
                        self.delete_choice = false;
                        self.stop_selected().await?;
                    }
                }
                _ => {}
            }
            return Ok(false);
        }
        if self.confirm_quit {
            match key.code {
                KeyCode::Left => {
                    self.quit_choice = true;
                }
                KeyCode::Right => {
                    self.quit_choice = false;
                }
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.quit_choice = true;
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.quit_choice = false;
                }
                KeyCode::Esc => {
                    self.confirm_quit = false;
                    self.quit_choice = false;
                    self.status = "Quit cancelled".to_string();
                }
                KeyCode::Enter => {
                    if self.quit_choice {
                        return Ok(true);
                    }
                    self.confirm_quit = false;
                    self.quit_choice = false;
                    self.status = "Quit cancelled".to_string();
                }
                _ => {}
            }
            return Ok(false);
        }
        if matches!(self.mode, Mode::Normal) {
            match key.code {
                KeyCode::Char('f') => {
                    self.view = View::Torrents;
                    return Ok(false);
                }
                KeyCode::Char('i') => {
                    self.view = View::Info;
                    return Ok(false);
                }
                KeyCode::Char('v') => {
                    self.view = View::Peers;
                    return Ok(false);
                }
                _ => {}
            }
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c') | KeyCode::Char('q') => {
                    self.confirm_quit = true;
                    self.quit_choice = false;
                    return Ok(false);
                }
                _ => {}
            }
        }

        match self.mode {
            Mode::Normal => self.handle_key_normal(key).await,
            Mode::EnterMagnet => self.handle_key_input(key, Mode::EnterMagnet).await,
            Mode::EnterTorrentDir => self.handle_key_input(key, Mode::EnterTorrentDir).await,
            Mode::FilePicker => self.handle_key_file_picker(key).await,
        }
    }

    async fn handle_key_normal(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Tab | KeyCode::BackTab | KeyCode::Char('\t') => {
                self.focus = match self.focus {
                    FocusPanel::Filters => FocusPanel::Torrents,
                    FocusPanel::Torrents => FocusPanel::Filters,
                };
            }
            KeyCode::Char('?') => {
                self.show_help = true;
                self.help_scroll = 0;
            }
            KeyCode::Char('t') => self.focus = FocusPanel::Torrents,
            KeyCode::Char('g') => self.focus = FocusPanel::Filters,
            KeyCode::Char('p') => {
                self.toggle_pause().await?;
            }
            KeyCode::Char('a') => {
                self.mode = Mode::EnterMagnet;
                self.input.clear();
                self.input_cursor = 0;
                self.suppress_next_input = true;
                self.suppress_paste_chars = 0;
                self.status = "Paste magnet/URL/path and press Enter".to_string();
            }
            KeyCode::Char('d') => {
                if self.selected_torrent().is_some() {
                    self.confirm_delete = true;
                    self.delete_choice = false;
                }
            }
            KeyCode::Char('q') => {
                self.confirm_quit = true;
                self.quit_choice = false;
            }
            KeyCode::Char('1') => {
                self.filter_index = 0;
                self.ensure_selection_for_filter();
            }
            KeyCode::Char('2') => {
                self.filter_index = 1;
                self.ensure_selection_for_filter();
            }
            KeyCode::Char('3') => {
                self.filter_index = 2;
                self.ensure_selection_for_filter();
            }
            KeyCode::Char('4') => {
                self.filter_index = 3;
                self.ensure_selection_for_filter();
            }
            KeyCode::Char('5') => {
                self.filter_index = 4;
                self.ensure_selection_for_filter();
            }
            KeyCode::Char('6') => {
                self.filter_index = 5;
                self.ensure_selection_for_filter();
            }
            KeyCode::Char('r') => {
                self.refresh();
            }
            KeyCode::Down | KeyCode::Char('j') => match self.focus {
                FocusPanel::Torrents => {
                    self.move_selection(1);
                }
                FocusPanel::Filters => {
                    self.filter_index = (self.filter_index + 1).min(FILTERS.len() - 1);
                    self.ensure_selection_for_filter();
                }
            },
            KeyCode::Up | KeyCode::Char('k') => match self.focus {
                FocusPanel::Torrents => {
                    self.move_selection(-1);
                }
                FocusPanel::Filters => {
                    self.filter_index = self.filter_index.saturating_sub(1);
                    self.ensure_selection_for_filter();
                }
            },
            _ => {}
        }
        Ok(false)
    }

    async fn handle_key_input(&mut self, key: KeyEvent, mode: Mode) -> Result<bool> {
        match key.code {
            KeyCode::Esc => {
                if mode == Mode::EnterTorrentDir {
                    if let Some(add_input) = self.pending_add_input.take() {
                        let output_folder = self.download_dir.to_string_lossy().into_owned();
                        self.input.clear();
                        self.input_cursor = 0;
                        self.suppress_next_input = false;
                        self.suppress_paste_chars = 0;
                        self.status = "Fetching metadata...".to_string();
                        self.last_error = None;
                        self.start_file_picker_with_dir(add_input, output_folder)
                            .await?;
                        return Ok(false);
                    }
                }
                self.mode = Mode::Normal;
                self.input.clear();
                self.input_cursor = 0;
                self.suppress_next_input = false;
                self.suppress_paste_chars = 0;
                self.status = "Cancelled".to_string();
            }
            KeyCode::Enter => {
                let value = self.input.trim().to_string();
                self.input.clear();
                self.input_cursor = 0;
                self.suppress_next_input = false;
                self.suppress_paste_chars = 0;
                match mode {
                    Mode::EnterMagnet => {
                        if value.is_empty() {
                            self.status = "Magnet cannot be empty".to_string();
                            self.last_error = Some("Empty magnet".to_string());
                        } else {
                            self.pending_add_input = Some(value);
                            self.mode = Mode::EnterTorrentDir;
                            self.input = self.download_dir.to_string_lossy().into_owned();
                            self.input_cursor = self.input.chars().count();
                            self.status = "Set download dir for this torrent".to_string();
                        }
                    }
                    Mode::EnterTorrentDir => {
                        let add_input = self
                            .pending_add_input
                            .take()
                            .ok_or_else(|| anyhow!("missing pending torrent input"))?;
                        let output_folder = if value.is_empty() {
                            self.download_dir.to_string_lossy().into_owned()
                        } else {
                            value
                        };
                        self.status = "Fetching metadata...".to_string();
                        self.last_error = None;
                        self.start_file_picker_with_dir(add_input, output_folder)
                            .await?;
                    }
                    _ => {}
                }
                if !matches!(self.mode, Mode::FilePicker | Mode::EnterTorrentDir) {
                    self.mode = Mode::Normal;
                }
            }
            KeyCode::Backspace => {
                self.backspace();
            }
            KeyCode::Delete => {
                self.delete();
            }
            KeyCode::Left => {
                self.move_cursor_left();
            }
            KeyCode::Right => {
                self.move_cursor_right();
            }
            KeyCode::Home => {
                self.input_cursor = 0;
            }
            KeyCode::End => {
                self.input_cursor = self.input.chars().count();
            }
            KeyCode::Char(c) => {
                if self.suppress_next_input {
                    self.suppress_next_input = false;
                    self.suppress_paste_chars = 0;
                    self.insert_char(c);
                } else if self.suppress_paste_chars > 0 {
                    self.suppress_paste_chars -= 1;
                } else {
                    self.insert_char(c);
                }
            }
            _ => {}
        }
        Ok(false)
    }

    async fn handle_key_file_picker(&mut self, key: KeyEvent) -> Result<bool> {
        let Some(picker) = &mut self.file_picker else {
            self.mode = Mode::Normal;
            return Ok(false);
        };
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.file_picker = None;
                self.status = "Cancelled file selection".to_string();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                picker.cursor = picker.cursor.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !picker.files.is_empty() {
                    picker.cursor = (picker.cursor + 1).min(picker.files.len() - 1);
                }
            }
            KeyCode::Char(' ') => {
                if let Some(file) = picker.files.get_mut(picker.cursor) {
                    file.included = !file.included;
                }
            }
            KeyCode::Char('a') => {
                for file in &mut picker.files {
                    file.included = true;
                }
            }
            KeyCode::Char('n') => {
                for file in &mut picker.files {
                    file.included = false;
                }
            }
            KeyCode::Enter => {
                let magnet = picker.magnet.clone();
                let output_folder = picker.output_folder.clone();
                let only_files: Vec<usize> = picker
                    .files
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, file)| if file.included { Some(idx) } else { None })
                    .collect();
                self.status = "Starting download...".to_string();
                self.last_error = None;
                self.start_download(magnet, output_folder, only_files)
                    .await?;
                self.file_picker = None;
                self.mode = Mode::Normal;
            }
            _ => {}
        }
        Ok(false)
    }

    async fn start_file_picker_with_dir(
        &mut self,
        magnet: String,
        output_folder: String,
    ) -> Result<()> {
        let add = build_add_torrent(&magnet)?;
        let response = self
            .api
            .api_add_torrent(
                add,
                Some(AddTorrentOptions {
                    list_only: true,
                    output_folder: Some(output_folder.clone()),
                    ..Default::default()
                }),
            )
            .await
            .context("error listing files")?;
        let info_hash = response.details.info_hash.as_str();
        if self.has_same_destination(info_hash, &output_folder) {
            return Err(anyhow!(
                "Torrent already added for this download directory"
            ));
        }
        let suffix = derive_folder_suffix(&response);
        let final_output = PathBuf::from(output_folder).join(sanitize_path_component(&suffix));
        if final_output.exists() {
            return Err(anyhow!("Destination folder already exists"));
        }
        std::fs::create_dir_all(&final_output).context("failed to create download folder")?;
        let output_folder = final_output.to_string_lossy().into_owned();
        let picker = build_picker(magnet, output_folder, response)?;
        self.file_picker = Some(picker);
        self.mode = Mode::FilePicker;
        self.status = "Select files and press Enter".to_string();
        Ok(())
    }

    async fn start_download(
        &mut self,
        magnet: String,
        output_folder: String,
        only_files: Vec<usize>,
    ) -> Result<()> {
        let add = build_add_torrent(&magnet)?;
        let response = self
            .api
            .api_add_torrent(
                add,
                Some(AddTorrentOptions {
                    only_files: Some(only_files),
                    output_folder: Some(output_folder),
                    overwrite: true,
                    ..Default::default()
                }),
            )
            .await
            .context("error adding torrent")?;
        if response.id.is_none() {
            return Err(anyhow!("torrent was not added"));
        }
        self.status = "Torrent added".to_string();
        Ok(())
    }

    fn has_same_destination(&self, info_hash: &str, output_folder: &str) -> bool {
        self.torrents.iter().any(|t| {
            t.info_hash
                .as_ref()
                .map(|h| h == info_hash)
                .unwrap_or(false)
                && t.output_folder == output_folder
        })
    }

    async fn toggle_pause(&mut self) -> Result<()> {
        let Some(t) = self.selected_torrent() else {
            return Ok(());
        };
        let Some(stats) = t.stats.as_ref() else {
            return Ok(());
        };
        match stats.state {
            librqbit::TorrentStatsState::Paused => {
                self.api
                    .api_torrent_action_start(t.id.into())
                    .await
                    .context("error resuming torrent")?;
                self.status = "Resumed".to_string();
            }
            librqbit::TorrentStatsState::Live | librqbit::TorrentStatsState::Initializing => {
                self.api
                    .api_torrent_action_pause(t.id.into())
                    .await
                    .context("error pausing torrent")?;
                self.status = "Paused".to_string();
            }
            librqbit::TorrentStatsState::Error => {
                self.status = "Cannot pause: torrent error".to_string();
            }
        }
        Ok(())
    }

    async fn stop_selected(&mut self) -> Result<()> {
        let Some(t) = self.selected_torrent() else {
            return Ok(());
        };
        self.api
            .api_torrent_action_forget(t.id.into())
            .await
            .context("error stopping torrent")?;
        self.status = "Stopped (forgotten)".to_string();
        Ok(())
    }

    async fn delete_selected_files(&mut self) -> Result<()> {
        let Some(t) = self.selected_torrent() else {
            return Ok(());
        };
        self.api
            .api_torrent_action_delete(t.id.into())
            .await
            .context("error deleting torrent and files")?;
        self.status = "Deleted torrent and files".to_string();
        Ok(())
    }

}

impl App {
    fn should_ignore_paste_char(&mut self) -> bool {
        if self.confirm_delete || self.show_help || self.last_error.is_some() {
            return false;
        }
        let now = Instant::now();
        if let Some(last) = self.last_char_at {
            if now.duration_since(last) <= Duration::from_millis(200) {
                self.last_char_at = Some(now);
                self.status = "Paste ignored".to_string();
                return true;
            }
        }
        self.last_char_at = Some(now);
        false
    }

    fn insert_char(&mut self, c: char) {
        let idx = cursor_to_byte_index(&self.input, self.input_cursor);
        self.input.insert(idx, c);
        self.input_cursor += 1;
    }

    fn insert_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let idx = cursor_to_byte_index(&self.input, self.input_cursor);
        self.input.insert_str(idx, text);
        self.input_cursor += text.chars().count();
    }

    fn backspace(&mut self) {
        if self.input_cursor == 0 {
            return;
        }
        let end = cursor_to_byte_index(&self.input, self.input_cursor);
        let start = cursor_to_byte_index(&self.input, self.input_cursor - 1);
        self.input.replace_range(start..end, "");
        self.input_cursor -= 1;
    }

    fn delete(&mut self) {
        let len = self.input.chars().count();
        if self.input_cursor >= len {
            return;
        }
        let start = cursor_to_byte_index(&self.input, self.input_cursor);
        let end = cursor_to_byte_index(&self.input, self.input_cursor + 1);
        self.input.replace_range(start..end, "");
    }

    fn move_cursor_left(&mut self) {
        self.input_cursor = self.input_cursor.saturating_sub(1);
    }

    fn move_cursor_right(&mut self) {
        let len = self.input.chars().count();
        self.input_cursor = (self.input_cursor + 1).min(len);
    }

    fn filter_match(&self, t: &TorrentRow) -> bool {
        use FilterKind::*;
        let Some(stats) = t.stats.as_ref() else {
            return matches!(self.selected_filter(), All | Stopped);
        };
        let is_seeding = stats.finished
            || (stats.total_bytes > 0
                && stats.progress_bytes >= stats.total_bytes
                && matches!(stats.state, librqbit::TorrentStatsState::Live));
        match self.selected_filter() {
            All => true,
            Downloading => {
                matches!(stats.state, librqbit::TorrentStatsState::Live) && !stats.finished
            }
            Seeding => is_seeding,
            Paused => matches!(stats.state, librqbit::TorrentStatsState::Paused),
            Stopped => false,
            Error => matches!(stats.state, librqbit::TorrentStatsState::Error),
        }
    }

    fn ensure_selection_for_filter(&mut self) {
        if self.torrents.is_empty() {
            self.selected = 0;
            return;
        }
        if self.selected < self.torrents.len() && self.filter_match(&self.torrents[self.selected]) {
            return;
        }
        if let Some(idx) = self.torrents.iter().enumerate().find_map(|(idx, t)| {
            if self.filter_match(t) {
                Some(idx)
            } else {
                None
            }
        }) {
            self.selected = idx;
        } else {
            self.selected = 0;
        }
    }

    fn move_selection(&mut self, delta: isize) {
        let indices = self.filtered_indices();
        if indices.is_empty() {
            return;
        }
        let current_pos = indices
            .iter()
            .position(|&idx| idx == self.selected)
            .unwrap_or(0) as isize;
        let next_pos = (current_pos + delta).clamp(0, indices.len() as isize - 1) as usize;
        self.selected = indices[next_pos];
    }
}

fn cursor_to_byte_index(s: &str, cursor: usize) -> usize {
    if cursor == 0 {
        return 0;
    }
    let mut count = 0;
    for (idx, _) in s.char_indices() {
        if count == cursor {
            return idx;
        }
        count += 1;
    }
    s.len()
}

fn build_picker(
    magnet: String,
    output_folder: String,
    response: ApiAddTorrentResponse,
) -> Result<FilePickerState> {
    let files = response
        .details
        .files
        .unwrap_or_default()
        .into_iter()
        .map(|f| FileEntry {
            name: f.name,
            length: f.length,
            included: f.included,
        })
        .collect();
    Ok(FilePickerState {
        magnet,
        output_folder,
        files,
        cursor: 0,
    })
}

fn to_row(details: TorrentDetailsResponse) -> Result<TorrentRow> {
    let id = details.id.ok_or_else(|| anyhow!("missing torrent id"))?;
    let name = details
        .name
        .clone()
        .unwrap_or_else(|| details.info_hash.clone());
    Ok(TorrentRow {
        id,
        name,
        info_hash: Some(details.info_hash),
        output_folder: details.output_folder,
        stats: details.stats,
    })
}

fn derive_folder_suffix(response: &ApiAddTorrentResponse) -> String {
    if let Some(name) = response.details.name.as_ref() {
        if !name.trim().is_empty() {
            return name.trim().to_string();
        }
    }
    if let Some(first) = response.details.files.as_ref().and_then(|f| f.first()) {
        if !first.name.trim().is_empty() {
            return first.name.trim().to_string();
        }
    }
    "download".to_string()
}

fn sanitize_path_component(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch == '/' || ch == '\\' {
            out.push('-');
        } else {
            out.push(ch);
        }
    }
    out.trim().to_string()
}

pub fn start_event_thread() -> mpsc::UnboundedReceiver<Event> {
    let (tx, rx) = mpsc::unbounded_channel();
    std::thread::spawn(move || {
        loop {
            if let Ok(ready) = crossterm::event::poll(Duration::from_millis(200)) {
                if !ready {
                    continue;
                }
                if let Ok(ev) = crossterm::event::read() {
                    let _ = tx.send(ev);
                }
            }
        }
    });
    rx
}

fn build_add_torrent(input: &str) -> Result<AddTorrent<'static>> {
    let trimmed = input.trim();
    if trimmed.starts_with("magnet:")
        || trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
    {
        return Ok(AddTorrent::Url(Cow::Owned(trimmed.to_string())));
    }
    let path = PathBuf::from(trimmed);
    if path.exists() {
        let data = std::fs::read(&path).context("failed to read .torrent file")?;
        return Ok(AddTorrent::TorrentFileBytes(Bytes::from(data)));
    }
    Err(anyhow!(
        "Input must be a magnet, URL, or an existing .torrent file path"
    ))
}
