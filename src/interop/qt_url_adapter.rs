use cxx_qt_lib::QUrl;

#[must_use]
pub fn qurl_to_local_or_uri(url: &QUrl) -> String {
    if url.is_empty() {
        return String::new();
    }

    if let Some(local) = url.to_local_file() {
        return local.to_string();
    }

    url.to_qstring().to_string()
}
