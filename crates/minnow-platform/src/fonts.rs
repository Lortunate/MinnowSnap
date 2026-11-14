use ab_glyph::{FontArc, FontVec};
use font_kit::family_name::FamilyName;
use font_kit::handle::Handle;
use font_kit::properties::Properties;
use font_kit::source::SystemSource;
use minnow_application::ports::{ErrorKind, OperationError, OperationResult};
use std::sync::Mutex;

#[derive(Default)]
pub struct SystemFonts {
    families: Mutex<Option<Vec<String>>>,
}

impl SystemFonts {
    pub fn families(&self) -> OperationResult<Vec<String>> {
        let mut cached = self
            .families
            .lock()
            .map_err(|_| OperationError::new(ErrorKind::Task, "font family cache was poisoned"))?;
        if let Some(families) = cached.as_ref() {
            return Ok(families.clone());
        }
        let mut families = SystemSource::new()
            .all_families()
            .map_err(|error| OperationError::new(ErrorKind::Native, format!("failed to enumerate fonts: {error}")))?;
        families.retain(|family| is_selectable_family(family));
        families.sort();
        families.dedup();
        *cached = Some(families.clone());
        Ok(families)
    }

    /// Resolve font bytes on the platform side; the editor receives an owned
    /// font and never consults system directories or family-name conventions.
    pub fn load(&self, preferred_families: &[&str]) -> OperationResult<FontArc> {
        let source = SystemSource::new();
        for family in preferred_families {
            let Ok(handle) = source.select_best_match(&[FamilyName::Title((*family).to_string())], &Properties::new()) else {
                continue;
            };
            let (bytes, index) = match handle {
                Handle::Path { path, font_index } => {
                    let Ok(bytes) = std::fs::read(path) else { continue };
                    (bytes, font_index)
                }
                Handle::Memory { bytes, font_index } => (bytes.as_ref().clone(), font_index),
            };
            if let Ok(font) = FontVec::try_from_vec_and_index(bytes, index) {
                return Ok(font.into());
            }
        }
        Err(OperationError::new(
            ErrorKind::Unavailable,
            "no usable font was found for annotation text",
        ))
    }

    pub fn annotation_font(&self) -> OperationResult<FontArc> {
        self.load(&[
            "Microsoft YaHei",
            "PingFang SC",
            "Noto Sans CJK SC",
            "Source Han Sans SC",
            "Segoe UI",
            "Arial",
            "Helvetica",
        ])
    }
}

fn is_selectable_family(family: &str) -> bool {
    if family.starts_with('.') {
        return false;
    }
    let lower = family.to_lowercase();
    let blocked = [
        "emoji",
        "dingbats",
        "symbol",
        "webdings",
        "wingdings",
        "nerd",
        "extra",
        "system",
        "braille",
        "private",
        "bitmap",
        "lastresort",
        "fallback",
        "stix",
        "math",
        "music",
        "general",
        "icon",
        "gb18030",
        "zalgo",
        "sans",
        "serif",
        "mono",
        "fixed",
        "terminal",
        "nisc",
        "kacst",
        "lohit",
        "tibetan",
    ];
    !blocked.iter().any(|block| lower.contains(block)) && (!lower.contains("ui") || lower.contains("segoe") || lower.contains("san francisco"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_filter_keeps_existing_system_family_exceptions() {
        assert!(is_selectable_family("Segoe UI"));
        assert!(is_selectable_family("Arial"));
        assert!(!is_selectable_family(".AppleSystemUIFont"));
        assert!(!is_selectable_family("Apple Color Emoji"));
    }
}
