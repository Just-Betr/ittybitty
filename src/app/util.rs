use std::{borrow::Cow, path::PathBuf};

use anyhow::{Context, Result, anyhow};
use bytes::Bytes;
use librqbit::{AddTorrent, api::{ApiAddTorrentResponse, TorrentDetailsResponse}};

use super::{FileEntry, FilePickerState, TorrentRow};

pub fn build_add_torrent(input: &str) -> Result<AddTorrent<'static>> {
    let trimmed = input.trim();
    let cleaned: String = trimmed
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_control())
        .collect();
    if cleaned.starts_with("magnet:")
        || cleaned.starts_with("http://")
        || cleaned.starts_with("https://")
    {
        return Ok(AddTorrent::Url(Cow::Owned(cleaned)));
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

pub fn build_picker(
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

pub fn to_row(details: TorrentDetailsResponse) -> Result<TorrentRow> {
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

pub fn derive_folder_suffix(response: &ApiAddTorrentResponse) -> String {
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

pub fn sanitize_path_component(input: &str) -> String {
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

pub fn cursor_to_byte_index(s: &str, cursor: usize) -> usize {
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
