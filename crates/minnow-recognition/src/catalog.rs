//! Pinned RapidOCR PP-OCRv5 catalog, verified on 2026-11-14.
//!
//! ModelScope git refs/tags/v3.10.0 peels to the full commit below. Expected
//! bytes and SHA256 are from its official repository file API (LFS object
//! identities for ONNX); all five payloads were independently downloaded and
//! hashed in temporary directories. The dictionary lives beside the matching
//! Paddle recognizer in the same commit, rather than on a moving GitHub branch.
//!
//! Provenance: https://modelscope.cn/api/v1/models/RapidAI/RapidOCR/repo/files?Revision=7d0781614ca1a83d5ad9603f713acb2e74855d72&Recursive=true
//! CTC convention: https://github.com/RapidAI/RapidOCR/blob/bd941dae09da793e76c6ba15c7aaf40f8424232a/python/rapidocr/ch_ppocr_rec/utils.py
use crate::sources::{ArtifactSpec, ModelSources};
use minnow_application::settings::OcrModelType;

const REVISION: &str = "7d0781614ca1a83d5ad9603f713acb2e74855d72";
const DICTIONARY_ENTRIES: usize = 18_383;

pub(crate) fn for_model(model: OcrModelType) -> ModelSources {
    let (kind, detector_bytes, detector_sha, recognizer_bytes, recognizer_sha) = match model {
        OcrModelType::Mobile => (
            "mobile",
            4_819_576,
            "4d97c44a20d30a81aad087d6a396b08f786c4635742afc391f6621f5c6ae78ae",
            16_631_306,
            "5825fc7ebf84ae7a412be049820b4d86d77620f204a041697b0494669b1742c5",
        ),
        OcrModelType::Server => (
            "server",
            88_118_768,
            "0f8846b1d4bba223a2a2f9d9b44022fbc22cc019051a602b41a7fda9667e4cad",
            84_577_022,
            "e09385400eaaaef34ceff54aeb7c4f0f1fe014c27fa8b9905d4709b65746562a",
        ),
    };
    let base = format!("https://modelscope.cn/models/RapidAI/RapidOCR/resolve/{REVISION}");
    let artifact = |name: String, path: String, bytes, digest| {
        ArtifactSpec::new(name, format!("{base}/{path}"), bytes, digest).expect("the built-in artifact catalog is valid")
    };
    ModelSources::verified(
        format!("RapidAI/RapidOCR@{REVISION}/PP-OCRv5/{kind}"),
        artifact(
            format!("ch_PP-OCRv5_det_{kind}.onnx"),
            format!("onnx/PP-OCRv5/det/ch_PP-OCRv5_det_{kind}.onnx"),
            detector_bytes,
            detector_sha,
        ),
        artifact(
            format!("ch_PP-OCRv5_rec_{kind}.onnx"),
            format!("onnx/PP-OCRv5/rec/ch_PP-OCRv5_rec_{kind}.onnx"),
            recognizer_bytes,
            recognizer_sha,
        ),
        artifact(
            "ppocrv5_dict.txt".into(),
            format!("paddle/PP-OCRv5/rec/ch_PP-OCRv5_rec_{kind}/ppocrv5_dict.txt"),
            74_012,
            "d1979e9f794c464c0d2e0b70a7fe14dd978e9dc644c0e71f14158cdf8342af1b",
        ),
        DICTIONARY_ENTRIES,
    )
    .expect("the built-in model catalog is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_catalog_pins_every_payload_and_ctc_class_count() {
        for model in [OcrModelType::Mobile, OcrModelType::Server] {
            let sources = for_model(model);
            assert_eq!(sources.recognition_classes(), 18_385);
            for artifact in sources.artifacts() {
                assert!(artifact.url().contains(REVISION));
                assert_eq!(artifact.expected_sha256().len(), 64);
                assert!(artifact.expected_bytes() > 0);
            }
        }
    }
}
