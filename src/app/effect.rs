#[derive(Debug, Clone)]
pub enum Effect {
    Refresh,
    TogglePause,
    StopSelected,
    DeleteSelectedFiles,
    StartFilePicker { magnet: String, output_folder: String },
    StartDownload {
        magnet: String,
        output_folder: String,
        only_files: Vec<usize>,
    },
}
