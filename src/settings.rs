use std::path::PathBuf;

#[derive(Clone, Debug, Default)]
pub struct AppSettings {
    pub default_output_folder: Option<PathBuf>,
}
