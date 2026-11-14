//! Model metadata and runtime tensor contracts for the supported Paddle pipeline.
use anyhow::{Result, bail};
use ort::{
    session::Session,
    value::{TensorElementType, ValueType},
};

#[derive(Clone, Copy, Debug)]
pub(super) enum Stage {
    Detector,
    Recognizer { classes: usize },
}

pub(super) fn validate_session(session: &Session, stage: Stage) -> Result<()> {
    if session.inputs().len() != 1 || session.inputs()[0].name() != "x" || session.outputs().len() != 1 {
        bail!("{stage:?} requires one input named x and one output");
    }
    validate_types(session.inputs()[0].dtype(), session.outputs()[0].dtype(), stage)
}

fn tensor_shape<'a>(value: &'a ValueType, stage: Stage, direction: &str) -> Result<&'a [i64]> {
    match value {
        ValueType::Tensor {
            ty: TensorElementType::Float32,
            shape,
            ..
        } if shape.iter().all(|dimension| *dimension == -1 || *dimension > 0) => Ok(shape),
        _ => bail!("{stage:?} {direction} must be a float32 tensor with positive or dynamic dimensions, got {value:?}"),
    }
}

fn validate_types(input: &ValueType, output: &ValueType, stage: Stage) -> Result<()> {
    let input = tensor_shape(input, stage, "input")?;
    let output = tensor_shape(output, stage, "output")?;
    if input.len() != 4 || ![-1, 1].contains(&input[0]) || input[1] != 3 || input[3] != -1 {
        bail!("{stage:?} input must be NCHW RGB with a dynamic width, got {input:?}");
    }
    match stage {
        Stage::Detector => {
            if input[2] != -1 || output.len() != 4 || ![-1, 1].contains(&output[0]) || ![-1, 1].contains(&output[1]) {
                bail!("Detector requires dynamic input height and an N1HW output, got input={input:?}, output={output:?}");
            }
        }
        Stage::Recognizer { classes } => {
            if input[2] != 48 || output.len() != 3 || output[2] != i64::try_from(classes)? {
                bail!("Recognizer requires input height 48 and NTC output with {classes} dictionary classes, got input={input:?}, output={output:?}");
            }
        }
    }
    Ok(())
}

pub(super) fn detector_output(shape: &[i64], data_len: usize) -> Result<(usize, usize)> {
    if shape.len() != 4 || shape[0] != 1 || shape[1] != 1 || shape[2] <= 0 || shape[3] <= 0 {
        bail!("Detector output must be [1, 1, positive height, positive width], got {shape:?}");
    }
    let height = usize::try_from(shape[2])?;
    let width = usize::try_from(shape[3])?;
    if width > u32::MAX as usize || height > u32::MAX as usize || height.checked_mul(width) != Some(data_len) {
        bail!("Detector output shape/data length mismatch: shape={shape:?}, elements={data_len}");
    }
    Ok((height, width))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ort::value::{Shape, SymbolicDimensions};

    fn tensor(shape: &[i64]) -> ValueType {
        ValueType::Tensor {
            ty: TensorElementType::Float32,
            shape: Shape::new(shape.iter().copied()),
            dimension_symbols: SymbolicDimensions::empty(shape.len()),
        }
    }

    #[test]
    fn metadata_rejects_wrong_recognizer_classes_and_input_layout() {
        let input = tensor(&[-1, 3, 48, -1]);
        assert!(validate_types(&input, &tensor(&[-1, -1, 7]), Stage::Recognizer { classes: 7 }).is_ok());
        assert!(validate_types(&input, &tensor(&[-1, -1, 6]), Stage::Recognizer { classes: 7 }).is_err());
        assert!(validate_types(&tensor(&[-1, 48, -1, 3]), &tensor(&[-1, -1, 7]), Stage::Recognizer { classes: 7 }).is_err());
        assert!(validate_types(&input, &tensor(&[-1, -1, -1]), Stage::Recognizer { classes: 7 }).is_err());
    }

    #[test]
    fn detector_runtime_rejects_rank_channels_negative_dimensions_and_truncated_data() {
        assert_eq!(detector_output(&[1, 1, 2, 3], 6).unwrap(), (2, 3));
        for (shape, length) in [(&[1, 2, 3][..], 6), (&[1, 2, 2, 3], 12), (&[1, 1, -1, 3], 0), (&[1, 1, 2, 3], 5)] {
            assert!(detector_output(shape, length).is_err());
        }
    }
}
