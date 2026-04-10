#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualCaptureSource {
    Preview,
    Scroll,
}

pub const PREVIEW_SOURCE: &str = "image://minnow/preview";
pub const SCROLL_SOURCE: &str = "image://minnow/scroll";
pub const PROVIDER_ID_PREVIEW: &str = "preview";
pub const PROVIDER_ID_SCROLL: &str = "scroll";

fn strip_query_fragment(input: &str) -> &str {
    input.split(['?', '#']).next().unwrap_or(input)
}

pub fn normalize_provider_id(id: &str) -> &str {
    strip_query_fragment(id).trim_matches('/')
}

pub fn normalize_virtual_source(source: &str) -> &str {
    strip_query_fragment(source)
}

pub fn parse_provider_source(id: &str) -> Option<VirtualCaptureSource> {
    match normalize_provider_id(id) {
        PROVIDER_ID_PREVIEW => Some(VirtualCaptureSource::Preview),
        PROVIDER_ID_SCROLL => Some(VirtualCaptureSource::Scroll),
        _ => None,
    }
}

pub fn parse_virtual_source(source: &str) -> Option<VirtualCaptureSource> {
    match normalize_virtual_source(source) {
        PREVIEW_SOURCE => Some(VirtualCaptureSource::Preview),
        SCROLL_SOURCE => Some(VirtualCaptureSource::Scroll),
        _ => None,
    }
}
