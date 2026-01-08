use log::{error, info};
use std::sync::{LazyLock, Mutex};
use std::time::Instant;

static FONTS_CACHE: LazyLock<Mutex<Option<Vec<String>>>> = LazyLock::new(|| Mutex::new(None));

pub fn get_system_fonts() -> Vec<String> {
    let mut cache_guard = FONTS_CACHE.lock().unwrap();

    if let Some(cached) = cache_guard.as_ref() {
        return cached.clone();
    }

    let start = Instant::now();
    let source = font_kit::source::SystemSource::new();
    let all_families = source.all_families().unwrap_or_else(|e| {
        error!("Failed to enumerate system fonts: {e}");
        vec![]
    });

    let mut filtered = Vec::new();
    let blocklist = [
        "Emoji",
        "Dingbats",
        "Symbol",
        "Webdings",
        "Wingdings",
        "Nerd",
        "Extra",
        "System",
        "Braille",
        "Private",
        "Bitmap",
        "LastResort",
        "Fallback",
        "STIX",
        "Math",
        "Music",
        "UI",
        "General",
        "icon",
        "GB18030",
        "Zalgo",
        "SANS",
        "SERIF",
        "MONO",
        "Fixed",
        "Terminal",
        "NISC",
        "Kacst",
        "Lohit",
        "Tibetan",
    ];

    for family in all_families {
        if family.starts_with('.') {
            continue;
        }

        let name_lower = family.to_lowercase();
        let mut allowed = true;
        for &block in blocklist.iter() {
            if name_lower.contains(&block.to_lowercase()) {
                if block == "UI" && (name_lower.contains("segoe") || name_lower.contains("san francisco")) {
                    continue;
                }
                allowed = false;
                break;
            }
        }

        if allowed {
            filtered.push(family);
        }
    }

    filtered.sort();

    info!("Loaded and filtered fonts in {:?}. Found {} fonts.", start.elapsed(), filtered.len());

    *cache_guard = Some(filtered.clone());
    filtered
}
