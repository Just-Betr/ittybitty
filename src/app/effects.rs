use std::{collections::HashSet, path::PathBuf};

use anyhow::{Context, Result, anyhow};
use librqbit::{AddTorrentOptions, api::ApiTorrentListOpts};
use tokio::time::sleep;

use super::{
    action::Action,
    effect::Effect,
    state::{App, Dialog, FilePickerState, TorrentRow},
    util::{build_add_torrent, build_picker, derive_folder_suffix, sanitize_path_component, to_row},
};

impl App {
    pub async fn run_effect(&mut self, effect: Effect) -> Result<Vec<Action>> {
        match effect {
            Effect::Refresh => {
                self.refresh();
            }
            Effect::TogglePause => {
                self.toggle_pause().await?;
            }
            Effect::StopSelected => {
                self.stop_selected().await?;
            }
            Effect::DeleteSelectedFiles => {
                self.delete_selected_files().await?;
            }
            Effect::PreflightAdd { magnet } => {
                let next = self.preflight_add(magnet).await?;
                return Ok(next);
            }
            Effect::StartFilePicker {
                magnet,
                output_folder,
            } => {
                self.start_file_picker_with_dir(magnet, output_folder).await?;
            }
            Effect::StartDownload {
                magnet,
                output_folder,
                only_files,
            } => {
                self.status = "Starting download...".to_string();
                self.last_error = None;
                self.start_download(magnet, output_folder, only_files).await?;
                self.file_picker = None;
                self.mode = super::Mode::Normal;
                self.dialog = Dialog::None;
            }
        }
        Ok(Vec::new())
    }

    async fn preflight_add(&mut self, magnet: String) -> Result<Vec<Action>> {
        let add = build_add_torrent(&magnet)?;
        let response = self
            .api
            .api_add_torrent(
                add,
                Some(AddTorrentOptions {
                    list_only: true,
                    output_folder: Some(self.download_dir.to_string_lossy().into_owned()),
                    ..Default::default()
                }),
            )
            .await
            .context("error listing files")?;
        let info_hash = response.details.info_hash.as_str();
        let existing = self
            .api
            .api_torrent_list_ext(ApiTorrentListOpts { with_stats: false })
            .torrents
            .iter()
            .any(|t| t.info_hash == info_hash);
        if existing {
            return Err(anyhow!(
                "Torrent already added; duplicate locations are not supported"
            ));
        }
        Ok(vec![Action::PreflightAddResult { magnet }])
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

    async fn start_file_picker_with_dir(
        &mut self,
        magnet: String,
        output_folder: String,
    ) -> Result<()> {
        let response = {
            let mut resp = None;
            for attempt in 0..3 {
                let add = build_add_torrent(&magnet)?;
                match self
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
                {
                    Ok(ok) => {
                        resp = Some(ok);
                        break;
                    }
                    Err(err) => {
                        if attempt < 2 {
                            sleep(std::time::Duration::from_millis(500)).await;
                        } else {
                            return Err(anyhow!("error listing files: {err}"));
                        }
                    }
                }
            }
            resp.ok_or_else(|| anyhow!("error listing files: unknown"))?
        };
        let info_hash = response.details.info_hash.as_str();
        let suffix = derive_folder_suffix(&response);
        let base = PathBuf::from(&output_folder);
        let mut folder_name = sanitize_path_component(&suffix);
        let mut final_output = base.join(&folder_name);
        if self.has_same_destination(info_hash, final_output.to_string_lossy().as_ref()) {
            return Err(anyhow!(
                "Torrent already added for this download directory"
            ));
        }
        if final_output.exists() {
            let short_hash: String = info_hash.chars().take(8).collect();
            folder_name = format!("{folder_name}-{short_hash}");
            final_output = base.join(&folder_name);
            if final_output.exists() {
                return Err(anyhow!("Destination folder already exists"));
            }
        }
        std::fs::create_dir_all(&final_output).context("failed to create download folder")?;
        let output_folder = final_output.to_string_lossy().into_owned();
        let picker: FilePickerState = build_picker(magnet, output_folder, response)?;
        self.file_picker = Some(picker);
        self.mode = super::Mode::FilePicker;
        self.status = "Select files and press Enter".to_string();
        self.dialog = Dialog::FilePicker;
        Ok(())
    }

    async fn start_download(
        &mut self,
        magnet: String,
        output_folder: String,
        only_files: Vec<usize>,
    ) -> Result<()> {
        if only_files.is_empty() {
            return Err(anyhow!("No files selected"));
        }
        let expected: HashSet<usize> = only_files.iter().copied().collect();
        let add = build_add_torrent(&magnet)?;
        let response = self
            .api
            .api_add_torrent(
                add,
                Some(AddTorrentOptions {
                    paused: true,
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
        if let Some(id) = response.id {
            let details = self
                .api
                .api_torrent_details(id.into())
                .context("error verifying file selection")?;
            let files = details
                .files
                .ok_or_else(|| anyhow!("torrent details missing files"))?;
            let actual: HashSet<usize> = files
                .into_iter()
                .enumerate()
                .filter_map(|(idx, file)| if file.included { Some(idx) } else { None })
                .collect();
            if actual != expected {
                let _ = self.api.api_torrent_action_delete(id.into()).await;
                return Err(anyhow!(
                    "File selection was not honored; torrent was removed"
                ));
            }
            self.api
                .api_torrent_action_start(id.into())
                .await
                .context("error starting torrent")?;
        }
        self.status = "Torrent added".to_string();
        Ok(())
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
