use std::{path::PathBuf, time::{Duration, Instant}};

use librqbit::{
    api::Api,
    session_stats::snapshot::SessionStatsSnapshot,
    TorrentStats,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    EnterMagnet,
    EnterTorrentDir,
    FilePicker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialog {
    None,
    AddTorrent,
    ConfirmDelete,
    ConfirmQuit,
    Help,
    FilePicker,
    Error,
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

pub const FILTERS: [FilterKind; 6] = [
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
    pub(crate) api: Api,
    pub(crate) torrents: Vec<TorrentRow>,
    pub(crate) selected: usize,
    pub(crate) mode: Mode,
    pub(crate) input: String,
    pub(crate) input_cursor: usize,
    pub(crate) last_char_at: Option<Instant>,
    pub(crate) download_dir: PathBuf,
    pub(crate) status: String,
    pub(crate) last_error: Option<String>,
    pub(crate) file_picker: Option<FilePickerState>,
    pub(crate) session_stats: Option<SessionStatsSnapshot>,
    pub(crate) view: View,
    pub(crate) confirm_delete: bool,
    pub(crate) delete_choice: bool,
    pub(crate) confirm_quit: bool,
    pub(crate) quit_choice: bool,
    pub(crate) focus: FocusPanel,
    pub(crate) filter_index: usize,
    pub(crate) pending_add_input: Option<String>,
    pub(crate) show_help: bool,
    pub(crate) help_scroll: u16,
    pub(crate) dialog: Dialog,
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
            dialog: Dialog::None,
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

    pub fn dialog(&self) -> Dialog {
        self.dialog
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
        self.last_char_at = None;
        self.dialog = Dialog::Error;
    }

    pub fn clear_error(&mut self) {
        self.last_error = None;
        self.status = "Ready".to_string();
        self.dialog = Dialog::None;
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub(crate) fn has_same_destination(&self, info_hash: &str, output_folder: &str) -> bool {
        self.torrents.iter().any(|t| {
            t.info_hash
                .as_ref()
                .map(|h| h == info_hash)
                .unwrap_or(false)
                && t.output_folder == output_folder
        })
    }

    pub(crate) fn should_ignore_paste_char(&mut self) -> bool {
        if self.dialog != Dialog::None {
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

    pub(crate) fn insert_char(&mut self, c: char) {
        let idx = super::util::cursor_to_byte_index(&self.input, self.input_cursor);
        self.input.insert(idx, c);
        self.input_cursor += 1;
    }

    pub(crate) fn backspace(&mut self) {
        if self.input_cursor == 0 {
            return;
        }
        let end = super::util::cursor_to_byte_index(&self.input, self.input_cursor);
        let start = super::util::cursor_to_byte_index(&self.input, self.input_cursor - 1);
        self.input.replace_range(start..end, "");
        self.input_cursor -= 1;
    }

    pub(crate) fn delete(&mut self) {
        let len = self.input.chars().count();
        if self.input_cursor >= len {
            return;
        }
        let start = super::util::cursor_to_byte_index(&self.input, self.input_cursor);
        let end = super::util::cursor_to_byte_index(&self.input, self.input_cursor + 1);
        self.input.replace_range(start..end, "");
    }

    pub(crate) fn move_cursor_left(&mut self) {
        self.input_cursor = self.input_cursor.saturating_sub(1);
    }

    pub(crate) fn move_cursor_right(&mut self) {
        let len = self.input.chars().count();
        self.input_cursor = (self.input_cursor + 1).min(len);
    }

    pub(crate) fn filter_match(&self, t: &TorrentRow) -> bool {
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

    pub(crate) fn ensure_selection_for_filter(&mut self) {
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

    pub(crate) fn move_selection(&mut self, delta: isize) {
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
