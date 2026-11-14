use minnow_application::ports::{ErrorKind, OperationError, OperationResult};
use minnow_application::settings::OcrModelType;
use std::path::{Component, Path};

/// A content identity supplied before a transfer, never inferred from its result.
#[derive(Clone, Debug)]
pub struct ArtifactSpec {
    name: String,
    url: String,
    bytes: u64,
    sha256: String,
}

impl ArtifactSpec {
    pub fn new(name: impl Into<String>, url: impl Into<String>, bytes: u64, sha256: impl Into<String>) -> OperationResult<Self> {
        let artifact = Self {
            name: name.into(),
            url: url.into(),
            bytes,
            sha256: sha256.into().to_ascii_lowercase(),
        };
        let mut components = Path::new(&artifact.name).components();
        if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
            return Err(invalid("model filenames must be single path components"));
        }
        if artifact.url.trim().is_empty()
            || artifact.bytes == 0
            || artifact.sha256.len() != 64
            || !artifact.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(invalid("artifacts require a source, a positive expected size and a SHA256 digest"));
        }
        Ok(artifact)
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn url(&self) -> &str {
        &self.url
    }
    pub fn expected_bytes(&self) -> u64 {
        self.bytes
    }
    pub fn expected_sha256(&self) -> &str {
        &self.sha256
    }
}

/// One compatible detector, recognizer, and exact dictionary.
///
/// The supported convention is Paddle CTC: blank class zero, every UTF-8
/// dictionary line in file order, then one ASCII space. No line is skipped.
/// Custom sources must supply all three pre-established artifact identities.
/// The recognizer must expose the same ordered dictionary in its ONNX
/// `character` metadata and output `dictionary_entries + 2` classes.
#[derive(Clone, Debug)]
pub struct ModelSources {
    identity: String,
    detector: ArtifactSpec,
    recognizer: ArtifactSpec,
    dictionary: ArtifactSpec,
    dictionary_entries: usize,
}

impl ModelSources {
    pub fn verified(
        identity: impl Into<String>,
        detector: ArtifactSpec,
        recognizer: ArtifactSpec,
        dictionary: ArtifactSpec,
        dictionary_entries: usize,
    ) -> OperationResult<Self> {
        let sources = Self {
            identity: identity.into(),
            detector,
            recognizer,
            dictionary,
            dictionary_entries,
        };
        sources.validate()?;
        Ok(sources)
    }

    pub fn for_model(model: OcrModelType) -> Self {
        crate::catalog::for_model(model)
    }
    pub fn identity(&self) -> &str {
        &self.identity
    }
    pub fn detector(&self) -> &ArtifactSpec {
        &self.detector
    }
    pub fn recognizer(&self) -> &ArtifactSpec {
        &self.recognizer
    }
    pub fn dictionary(&self) -> &ArtifactSpec {
        &self.dictionary
    }
    pub fn dictionary_entries(&self) -> usize {
        self.dictionary_entries
    }
    pub fn recognition_classes(&self) -> usize {
        self.dictionary_entries + 2
    }

    #[cfg(any(test, feature = "local-ocr"))]
    pub(crate) fn artifacts(&self) -> [&ArtifactSpec; 3] {
        [&self.detector, &self.recognizer, &self.dictionary]
    }

    pub(crate) fn validate(&self) -> OperationResult<()> {
        if self.identity.trim().is_empty() || self.dictionary_entries == 0 || self.dictionary_entries.checked_add(2).is_none() {
            return Err(invalid(
                "a model spec requires an identity and a nonempty dictionary with a bounded class count",
            ));
        }
        let names = [&self.detector.name, &self.recognizer.name, &self.dictionary.name];
        if names[0] == names[1] || names[0] == names[2] || names[1] == names[2] {
            return Err(invalid("model filenames must be distinct"));
        }
        Ok(())
    }
}

fn invalid(message: &str) -> OperationError {
    OperationError::new(ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_specs_cannot_omit_content_identity() {
        for (name, size, digest) in [
            ("../model", 1, "a".repeat(64)),
            ("model", 0, "a".repeat(64)),
            ("model", 1, "unknown".into()),
        ] {
            assert!(ArtifactSpec::new(name, "source", size, digest).is_err());
        }
    }
}
