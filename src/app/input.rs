use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};

use super::{FocusPanel, Mode, View, action::Action, state::App};

impl App {
    pub fn actions_from_event(&mut self, ev: Event) -> Vec<Action> {
        match ev {
            Event::Key(key) => self.actions_from_key(key),
            Event::Paste(text) => vec![Action::Paste(text)],
            Event::Resize(_, _) => Vec::new(),
            _ => Vec::new(),
        }
    }

    pub fn actions_from_key(&mut self, key: KeyEvent) -> Vec<Action> {
        if key.kind == KeyEventKind::Repeat {
            let repeat_ok = matches!(
                key.code,
                KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right
            );
            let input_repeat_ok = matches!(self.mode, Mode::EnterMagnet | Mode::EnterTorrentDir)
                && matches!(key.code, KeyCode::Char(_));
            if !repeat_ok && !input_repeat_ok {
                return Vec::new();
            }
        } else if key.kind != KeyEventKind::Press {
            return Vec::new();
        }
        if matches!(self.mode, Mode::Normal) {
            match key.code {
                KeyCode::Char(c) => {
                    let nav_char = matches!(c, 'h' | 'j' | 'k' | 'l');
                    if !nav_char && self.should_ignore_paste_char() {
                        return Vec::new();
                    }
                }
                _ => {
                    self.last_char_at = None;
                }
            }
        }
        if self.show_help {
            return match key.code {
                KeyCode::Char('?') | KeyCode::Char('x') | KeyCode::Esc => {
                    vec![Action::HelpClose]
                }
                KeyCode::Up | KeyCode::Char('k') => vec![Action::HelpScroll(-1)],
                KeyCode::Down | KeyCode::Char('j') => vec![Action::HelpScroll(1)],
                _ => Vec::new(),
            };
        }
        if self.last_error.is_some() {
            if matches!(key.code, KeyCode::Char('x') | KeyCode::Esc) {
                return vec![Action::ErrorClear];
            }
            return Vec::new();
        }
        if self.confirm_delete {
            return match key.code {
                KeyCode::Left | KeyCode::Char('h') => vec![Action::ConfirmDeleteSelect(true)],
                KeyCode::Right | KeyCode::Char('l') => vec![Action::ConfirmDeleteSelect(false)],
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    vec![Action::ConfirmDeleteSelect(true)]
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    vec![Action::ConfirmDeleteSelect(false)]
                }
                KeyCode::Esc => vec![Action::ConfirmDeleteCancel],
                KeyCode::Enter => vec![Action::ConfirmDeleteConfirm],
                _ => Vec::new(),
            };
        }
        if self.confirm_quit {
            return match key.code {
                KeyCode::Left | KeyCode::Char('h') => vec![Action::ConfirmQuitSelect(true)],
                KeyCode::Right | KeyCode::Char('l') => vec![Action::ConfirmQuitSelect(false)],
                KeyCode::Char('y') | KeyCode::Char('Y') => vec![Action::ConfirmQuitSelect(true)],
                KeyCode::Char('n') | KeyCode::Char('N') => vec![Action::ConfirmQuitSelect(false)],
                KeyCode::Esc => vec![Action::ConfirmQuitCancel],
                KeyCode::Enter => vec![Action::ConfirmQuitConfirm],
                _ => Vec::new(),
            };
        }
        if matches!(self.mode, Mode::Normal) {
            return match key.code {
                KeyCode::Char('f') => vec![Action::ViewSet(View::Torrents)],
                KeyCode::Char('i') => vec![Action::ViewSet(View::Info)],
                KeyCode::Char('v') => vec![Action::ViewSet(View::Peers)],
                KeyCode::Tab | KeyCode::BackTab | KeyCode::Char('\t') => vec![Action::FocusToggle],
                KeyCode::Char('?') => vec![Action::HelpOpen],
                KeyCode::Char('t') => vec![Action::FocusSet(FocusPanel::Torrents)],
                KeyCode::Char('g') => vec![Action::FocusSet(FocusPanel::Filters)],
                KeyCode::Char('p') => vec![Action::TogglePause],
                KeyCode::Char('a') => vec![Action::StartAdd],
                KeyCode::Char('d') => vec![Action::ConfirmDeleteOpen],
                KeyCode::Char('q') => vec![Action::ConfirmQuitOpen],
                KeyCode::Char('1') => vec![Action::SetFilter(0)],
                KeyCode::Char('2') => vec![Action::SetFilter(1)],
                KeyCode::Char('3') => vec![Action::SetFilter(2)],
                KeyCode::Char('4') => vec![Action::SetFilter(3)],
                KeyCode::Char('5') => vec![Action::SetFilter(4)],
                KeyCode::Char('6') => vec![Action::SetFilter(5)],
                KeyCode::Char('r') => vec![Action::Refresh],
                KeyCode::Down | KeyCode::Char('j') => match self.focus {
                    FocusPanel::Torrents => vec![Action::MoveSelection(1)],
                    FocusPanel::Filters => vec![Action::MoveFilter(1)],
                },
                KeyCode::Up | KeyCode::Char('k') => match self.focus {
                    FocusPanel::Torrents => vec![Action::MoveSelection(-1)],
                    FocusPanel::Filters => vec![Action::MoveFilter(-1)],
                },
                _ => Vec::new(),
            };
        }
        match self.mode {
            Mode::EnterMagnet | Mode::EnterTorrentDir => match key.code {
                KeyCode::Esc => vec![Action::InputCancel],
                KeyCode::Enter => vec![Action::InputEnter],
                KeyCode::Backspace => vec![Action::InputBackspace],
                KeyCode::Delete => vec![Action::InputDelete],
                KeyCode::Left => vec![Action::InputLeft],
                KeyCode::Right => vec![Action::InputRight],
                KeyCode::Home => vec![Action::InputHome],
                KeyCode::End => vec![Action::InputEnd],
                KeyCode::Char(c) => vec![Action::InputChar(c)],
                _ => Vec::new(),
            },
            Mode::FilePicker => match key.code {
                KeyCode::Esc => vec![Action::FilePickerCancel],
                KeyCode::Up | KeyCode::Char('k') => vec![Action::FilePickerUp],
                KeyCode::Down | KeyCode::Char('j') => vec![Action::FilePickerDown],
                KeyCode::Char(' ') => vec![Action::FilePickerToggle],
                KeyCode::Char('a') => vec![Action::FilePickerAll],
                KeyCode::Char('n') => vec![Action::FilePickerNone],
                KeyCode::Enter => vec![Action::FilePickerConfirm],
                _ => Vec::new(),
            },
            Mode::Normal => Vec::new(),
        }
    }
}
