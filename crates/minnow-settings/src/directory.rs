use serde::{Deserialize, Serialize};
use std::path::Path;

/// A default output directory or a custom path that can be stored without
/// changing its native spelling. Existence and permissions are checked on export.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct SaveDirectory(Option<String>);

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SaveDirectoryError {
    #[error("the save directory cannot be stored because its path is not valid UTF-8")]
    NonUtf8,
    #[error("the save directory cannot contain a NUL character")]
    ContainsNul,
}

impl SaveDirectory {
    /// Empty and entirely whitespace paths select the system default. Custom
    /// paths retain all whitespace and components exactly as supplied.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, SaveDirectoryError> {
        let path = path.as_ref().to_str().ok_or(SaveDirectoryError::NonUtf8)?;
        if path.contains('\0') {
            return Err(SaveDirectoryError::ContainsNul);
        }
        Ok(Self((!path.trim().is_empty()).then(|| path.to_owned())))
    }

    pub fn is_default(&self) -> bool {
        self.0.is_none()
    }

    pub fn as_custom_str(&self) -> Option<&str> {
        self.0.as_deref()
    }

    pub fn as_custom_path(&self) -> Option<&Path> {
        self.as_custom_str().map(Path::new)
    }
}

impl<'de> Deserialize<'de> for SaveDirectory {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Option::<String>::deserialize(deserializer)? {
            Some(path) => Self::new(path).map_err(serde::de::Error::custom),
            None => Ok(Self::default()),
        }
    }
}
