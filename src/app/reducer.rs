use std::collections::VecDeque;

use anyhow::{Result, anyhow};

use super::{
    action::Action,
    effect::Effect,
    state::Dialog,
    FocusPanel,
    Mode,
};
use super::state::App;

impl App {
    pub async fn handle_event(&mut self, ev: crossterm::event::Event) -> Result<bool> {
        let mut queue: VecDeque<Action> = self.actions_from_event(ev).into();
        while let Some(action) = queue.pop_front() {
            if let Some(quit) = self.apply_action(action, &mut queue).await? {
                return Ok(quit);
            }
        }
        Ok(false)
    }

    async fn apply_action(
        &mut self,
        action: Action,
        queue: &mut VecDeque<Action>,
    ) -> Result<Option<bool>> {
        match action {
            Action::Paste(text) => {
                if matches!(self.mode, Mode::EnterMagnet | Mode::EnterTorrentDir) {
                    self.input = text;
                    self.input_cursor = self.input.chars().count();
                } else {
                    self.last_char_at = Some(std::time::Instant::now());
                    self.status = "Paste ignored".to_string();
                }
            }
            Action::HelpOpen => {
                self.show_help = true;
                self.help_scroll = 0;
                self.dialog = Dialog::Help;
            }
            Action::HelpClose => {
                self.show_help = false;
                self.help_scroll = 0;
                self.dialog = Dialog::None;
            }
            Action::HelpScroll(delta) => {
                if delta.is_negative() {
                    self.help_scroll = self.help_scroll.saturating_sub(delta.unsigned_abs());
                } else {
                    self.help_scroll = self.help_scroll.saturating_add(delta as u16);
                }
            }
            Action::ErrorClear => {
                self.clear_error();
            }
            Action::ConfirmDeleteOpen => {
                if self.selected_torrent().is_some() {
                    self.confirm_delete = true;
                    self.delete_choice = false;
                    self.dialog = Dialog::ConfirmDelete;
                }
            }
            Action::ConfirmDeleteSelect(choice) => {
                self.delete_choice = choice;
            }
            Action::ConfirmDeleteConfirm => {
                if self.delete_choice {
                    self.confirm_delete = false;
                    self.delete_choice = false;
                    self.dialog = Dialog::None;
                    queue.push_back(Action::RunEffect(Effect::DeleteSelectedFiles));
                } else {
                    self.confirm_delete = false;
                    self.delete_choice = false;
                    self.dialog = Dialog::None;
                    queue.push_back(Action::RunEffect(Effect::StopSelected));
                }
            }
            Action::ConfirmDeleteCancel => {
                self.confirm_delete = false;
                self.delete_choice = false;
                self.status = "Delete cancelled".to_string();
                self.dialog = Dialog::None;
            }
            Action::ConfirmQuitOpen => {
                self.confirm_quit = true;
                self.quit_choice = false;
                self.dialog = Dialog::ConfirmQuit;
            }
            Action::ConfirmQuitSelect(choice) => {
                self.quit_choice = choice;
            }
            Action::ConfirmQuitConfirm => {
                if self.quit_choice {
                    return Ok(Some(true));
                }
                self.confirm_quit = false;
                self.quit_choice = false;
                self.status = "Quit cancelled".to_string();
                self.dialog = Dialog::None;
            }
            Action::ConfirmQuitCancel => {
                self.confirm_quit = false;
                self.quit_choice = false;
                self.status = "Quit cancelled".to_string();
                self.dialog = Dialog::None;
            }
            Action::ViewSet(view) => {
                self.view = view;
            }
            Action::FocusToggle => {
                self.focus = match self.focus {
                    FocusPanel::Filters => FocusPanel::Torrents,
                    FocusPanel::Torrents => FocusPanel::Filters,
                };
            }
            Action::FocusSet(panel) => {
                self.focus = panel;
            }
            Action::MoveSelection(delta) => {
                self.move_selection(delta);
            }
            Action::MoveFilter(delta) => {
                if delta.is_negative() {
                    self.filter_index = self.filter_index.saturating_sub(delta.unsigned_abs());
                } else {
                    self.filter_index =
                        (self.filter_index + delta as usize).min(super::state::FILTERS.len() - 1);
                }
                self.ensure_selection_for_filter();
            }
            Action::SetFilter(index) => {
                self.filter_index = index.min(super::state::FILTERS.len() - 1);
                self.ensure_selection_for_filter();
            }
            Action::TogglePause => {
                queue.push_back(Action::RunEffect(Effect::TogglePause));
            }
            Action::StartAdd => {
                self.mode = Mode::EnterMagnet;
                self.input.clear();
                self.input_cursor = 0;
                self.status = "Paste magnet/URL/path and press Enter".to_string();
                self.dialog = Dialog::AddTorrent;
            }
            Action::InputChar(c) => {
                self.insert_char(c);
            }
            Action::InputBackspace => {
                self.backspace();
            }
            Action::InputDelete => {
                self.delete();
            }
            Action::InputLeft => {
                self.move_cursor_left();
            }
            Action::InputRight => {
                self.move_cursor_right();
            }
            Action::InputHome => {
                self.input_cursor = 0;
            }
            Action::InputEnd => {
                self.input_cursor = self.input.chars().count();
            }
            Action::InputEnter => {
                if self.mode == Mode::EnterMagnet {
                    self.status = "Fetching metadata...".to_string();
                }
                let value = self.input.trim().to_string();
                self.input.clear();
                self.input_cursor = 0;
                match self.mode {
                    Mode::EnterMagnet => {
                        if value.is_empty() {
                            self.set_error("Magnet cannot be empty");
                        } else if let Err(err) = super::util::build_add_torrent(&value) {
                            self.set_error(err);
                        } else {
                            self.status = "Checking torrent...".to_string();
                            self.dialog = Dialog::None;
                            queue.push_back(Action::RunEffect(Effect::PreflightAdd {
                                magnet: value,
                            }));
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
                        queue.push_back(Action::RunEffect(Effect::StartFilePicker {
                            magnet: add_input,
                            output_folder,
                        }));
                    }
                    _ => {}
                }
                if !matches!(self.mode, Mode::FilePicker | Mode::EnterTorrentDir) {
                    self.mode = Mode::Normal;
                }
            }
            Action::InputCancel => {
                if self.mode == Mode::EnterTorrentDir {
                    if let Some(add_input) = self.pending_add_input.take() {
                        let output_folder = self.download_dir.to_string_lossy().into_owned();
                        self.input.clear();
                        self.input_cursor = 0;
                        self.status = "Fetching metadata...".to_string();
                        self.last_error = None;
                        self.dialog = Dialog::AddTorrent;
                        queue.push_back(Action::RunEffect(Effect::StartFilePicker {
                            magnet: add_input,
                            output_folder,
                        }));
                        return Ok(None);
                    }
                }
                self.mode = Mode::Normal;
                self.input.clear();
                self.input_cursor = 0;
                self.status = "Cancelled".to_string();
                self.dialog = Dialog::None;
            }
            Action::FilePickerCancel => {
                self.mode = Mode::Normal;
                self.file_picker = None;
                self.status = "Cancelled file selection".to_string();
                self.dialog = Dialog::None;
            }
            Action::FilePickerUp => {
                if let Some(picker) = &mut self.file_picker {
                    picker.cursor = picker.cursor.saturating_sub(1);
                }
            }
            Action::FilePickerDown => {
                if let Some(picker) = &mut self.file_picker {
                    if !picker.files.is_empty() {
                        picker.cursor = (picker.cursor + 1).min(picker.files.len() - 1);
                    }
                }
            }
            Action::FilePickerToggle => {
                if let Some(picker) = &mut self.file_picker {
                    if let Some(file) = picker.files.get_mut(picker.cursor) {
                        file.included = !file.included;
                    }
                }
            }
            Action::FilePickerAll => {
                if let Some(picker) = &mut self.file_picker {
                    for file in &mut picker.files {
                        file.included = true;
                    }
                }
            }
            Action::FilePickerNone => {
                if let Some(picker) = &mut self.file_picker {
                    for file in &mut picker.files {
                        file.included = false;
                    }
                }
            }
            Action::FilePickerConfirm => {
                if let Some(picker) = &self.file_picker {
                    let magnet = picker.magnet.clone();
                    let output_folder = picker.output_folder.clone();
                    let only_files: Vec<usize> = picker
                        .files
                        .iter()
                        .enumerate()
                        .filter_map(|(idx, file)| if file.included { Some(idx) } else { None })
                        .collect();
                    queue.push_back(Action::RunEffect(Effect::StartDownload {
                        magnet,
                        output_folder,
                        only_files,
                    }));
                }
            }
            Action::Refresh => {
                queue.push_back(Action::RunEffect(Effect::Refresh));
            }
            Action::RunEffect(effect) => {
                let next = self.run_effect(effect).await?;
                for action in next {
                    queue.push_back(action);
                }
            }
            Action::PreflightAddResult { magnet } => {
                self.pending_add_input = Some(magnet);
                self.mode = Mode::EnterTorrentDir;
                self.input = self.download_dir.to_string_lossy().into_owned();
                self.input_cursor = self.input.chars().count();
                self.status = "Set download dir for this torrent".to_string();
                self.dialog = Dialog::AddTorrent;
            }
        }
        Ok(None)
    }
}

