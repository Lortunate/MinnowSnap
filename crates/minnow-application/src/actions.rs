use crate::{ErrorKind, JobScope, OperationError, OperationResult};
use minnow_editor::{AnnotationOutputError, AnnotationOutputIdentity, AnnotationSnapshot};
use minnow_imaging::{CapturedFrame, ImageRect, ImageRef, RectF, ResolvedSelection, ViewRect};
use std::path::PathBuf;

/// Executable image operations. Window navigation and OCR presentation are UI policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImageCommand {
    Copy,
    Save { destination: SaveDestination },
    CopyQr,
    CopyColor,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SaveDestination {
    /// Resolve the application's configured directory when the command is submitted.
    Preferences,
    Directory(PathBuf),
}

#[derive(Clone, Debug)]
pub enum ImageCommandResult {
    Copied,
    Saved(crate::ports::ExportReceipt),
    QrCopied(String),
    ColorCopied(String),
}

#[derive(Clone, Debug)]
pub struct PreparedImage {
    pub image: ImageRef,
    /// Logical coordinates in the desktop's screen coordinate system, if known.
    pub source_bounds: Option<RectF>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ImageProjection {
    Full,
    Selection(ViewRect),
}

/// Immutable source owned by a document session. Output publication never accepts
/// replacement pixels from the caller.
#[derive(Clone, Debug)]
pub(crate) enum ImageSource {
    Image(ImageRef),
    Capture(CapturedFrame),
}

impl ImageSource {
    pub(crate) fn image(&self) -> &ImageRef {
        match self {
            Self::Image(image) => image,
            Self::Capture(frame) => frame.image(),
        }
    }
}

/// Frozen pixels, geometry and annotation state. Standalone constructors always
/// produce application-scoped input. Only session publication binds a revision.
#[derive(Clone, Debug)]
pub struct ImageInput {
    image: InputImage,
    annotations: Option<AnnotationSnapshot>,
    scope: JobScope,
}

#[derive(Clone, Debug)]
enum InputImage {
    Full(ImageRef),
    Selection { frame: CapturedFrame, selection: ResolvedSelection },
}

/// The source and geometry are fixed by the session. Only the resolved crop and
/// the annotation output identity can vary within a publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ImageContentIdentity {
    selection: Option<ImageRect>,
    annotations: Option<AnnotationOutputIdentity>,
}

impl ImageInput {
    /// Validate source and rendering requirements before accepting standalone input.
    pub fn image(image: ImageRef, annotations: Option<AnnotationSnapshot>) -> OperationResult<Self> {
        Self::from_image(image, annotations, JobScope::Application)
    }

    /// Validate selection and rendering requirements without allocating output pixels.
    pub fn selection(frame: CapturedFrame, selection: ViewRect, annotations: Option<AnnotationSnapshot>) -> OperationResult<Self> {
        Self::from_frame(frame, selection, annotations, JobScope::Application)
    }

    pub fn scope(&self) -> JobScope {
        self.scope
    }

    pub(crate) fn content_identity(&self) -> ImageContentIdentity {
        ImageContentIdentity {
            selection: match &self.image {
                InputImage::Full(_) => None,
                InputImage::Selection { selection, .. } => Some(selection.pixel_rect()),
            },
            annotations: self
                .annotations
                .as_ref()
                .filter(|snapshot| !snapshot.items().is_empty())
                .map(AnnotationSnapshot::output_identity),
        }
    }

    pub(crate) fn from_source(
        source: &ImageSource,
        projection: ImageProjection,
        annotations: Option<AnnotationSnapshot>,
        scope: JobScope,
    ) -> OperationResult<Self> {
        match (source, projection) {
            (ImageSource::Image(image), ImageProjection::Full) => Self::from_image(image.clone(), annotations, scope),
            (ImageSource::Image(_), ImageProjection::Selection(_)) => Err(OperationError::new(
                ErrorKind::InvalidInput,
                "this document source has no capture geometry for a selection",
            )),
            (ImageSource::Capture(frame), projection) => {
                let selection = match projection {
                    ImageProjection::Full => {
                        let geometry = frame.geometry();
                        ViewRect(RectF::new(
                            0.0,
                            0.0,
                            f64::from(frame.image().width()) / geometry.scale_factor,
                            f64::from(frame.image().height()) / geometry.scale_factor,
                        ))
                    }
                    ImageProjection::Selection(selection) => selection,
                };
                Self::from_frame(frame.clone(), selection, annotations, scope)
            }
        }
    }

    fn from_image(image: ImageRef, annotations: Option<AnnotationSnapshot>, scope: JobScope) -> OperationResult<Self> {
        let input = Self {
            image: InputImage::Full(image),
            annotations,
            scope,
        };
        input.validate()?;
        Ok(input)
    }

    fn from_frame(frame: CapturedFrame, selection: ViewRect, annotations: Option<AnnotationSnapshot>, scope: JobScope) -> OperationResult<Self> {
        let selection = frame.geometry().resolve_selection(selection).map_err(input_error)?;
        let input = Self {
            image: InputImage::Selection { frame, selection },
            annotations,
            scope,
        };
        input.validate()?;
        Ok(input)
    }

    fn validate(&self) -> OperationResult<()> {
        let (image, scale) = match &self.image {
            InputImage::Full(image) => (image, 1.0),
            InputImage::Selection { frame, selection } => (frame.image(), selection.scale_factor()),
        };
        if image.width() == 0 || image.height() == 0 {
            return Err(OperationError::new(ErrorKind::InvalidInput, "image is empty"));
        }
        if let Some(snapshot) = &self.annotations {
            snapshot.validate_for_composition(scale).map_err(composition_error)?;
        }
        Ok(())
    }

    pub(crate) fn resolve(&self) -> OperationResult<PreparedImage> {
        self.validate()?;
        match &self.image {
            InputImage::Full(image) => {
                let image = match &self.annotations {
                    Some(snapshot) => snapshot.compose_asset(image, 1.0).map_err(composition_error)?,
                    None => image.clone(),
                };
                Ok(PreparedImage { image, source_bounds: None })
            }
            InputImage::Selection { frame, selection } => {
                let image = match &self.annotations {
                    Some(snapshot) => snapshot.compose_selection(frame.image(), *selection).map_err(composition_error)?,
                    None => frame.crop_resolved(*selection).map_err(input_error)?,
                };
                let rect = selection.view_rect().0;
                Ok(PreparedImage {
                    image,
                    source_bounds: Some(RectF::new(
                        frame.geometry().screen_origin.x + rect.x,
                        frame.geometry().screen_origin.y + rect.y,
                        rect.width,
                        rect.height,
                    )),
                })
            }
        }
    }
}

fn input_error(error: impl std::fmt::Display) -> OperationError {
    OperationError::new(ErrorKind::InvalidInput, error.to_string())
}

fn composition_error(error: AnnotationOutputError) -> OperationError {
    let kind = match &error {
        AnnotationOutputError::MissingFont => ErrorKind::Unavailable,
        AnnotationOutputError::InvalidScale | AnnotationOutputError::Geometry(_) => ErrorKind::InvalidInput,
    };
    OperationError::new(kind, error.to_string())
}
