#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyUnicode {
    Shift,
}

pub fn key_symbol(key: KeyUnicode) -> &'static str {
    match key {
        KeyUnicode::Shift => "⇧",
    }
}
