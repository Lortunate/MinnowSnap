//! Native session policy shared by artifact validation and inference.
use anyhow::Result;
use ort::{
    environment::Environment,
    session::{
        Session,
        builder::{AutoDevicePolicy, GraphOptimizationLevel},
    },
};
use std::path::Path;

pub(super) fn create_onnx_session<P: AsRef<Path>>(environment: &Environment, model_path: P, threads: usize) -> Result<Session> {
    Session::builder(environment)
        .map_err(|e| anyhow::anyhow!("Failed to create session builder: {}", e))?
        // ort's MaxEfficiency default auto-selects CoreML on macOS. The
        // pinned models are validated on CPU; keep host hardware from changing
        // this contract (Mobile CTC can otherwise silently return all blanks).
        .with_auto_device(AutoDevicePolicy::PreferCPU)
        .map_err(|e| anyhow::anyhow!("Failed to select CPU execution: {}", e))?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|e| anyhow::anyhow!("Failed to set optimization level: {}", e))?
        .with_intra_threads(threads)
        .map_err(|e| anyhow::anyhow!("Failed to set intra threads: {}", e))?
        .commit_from_file(model_path)
        .map_err(|e| anyhow::anyhow!("Failed to commit session from file: {}", e))
}
