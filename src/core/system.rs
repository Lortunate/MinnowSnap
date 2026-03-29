use gpui::App;
use std::path::Path;

pub fn open_external_url(app: &mut App, url: &str) {
    app.open_url(url);
}

pub fn open_in_file_manager(app: &mut App, path: impl AsRef<Path>) {
    app.open_with_system(path.as_ref());
}
