#[derive(Debug, Clone)]
pub enum Effect {
    Refresh,
    TogglePause,
    StopSelected,
    DeleteSelectedFiles,
    PreflightAdd {
        magnet: String,
    },
    StartFilePicker { magnet: String, output_folder: String },
    StartDownload {
        magnet: String,
        output_folder: String,
        only_files: Vec<usize>,
    },
}

