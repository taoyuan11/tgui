use super::{GeometryError, ResourceId};
use std::error::Error as StdError;
use std::fmt;

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Stable top-level classification for diagnostics and recovery policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    UserInput,
    Compile,
    Resource,
    Platform,
    Degraded,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputError {
    pub field: Option<String>,
    pub message: String,
}

impl InputError {
    pub fn new(field: impl Into<Option<String>>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompileError {
    pub stage: String,
    pub message: String,
}

impl CompileError {
    pub fn new(stage: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            stage: stage.into(),
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceError {
    pub resource: Option<ResourceId>,
    pub message: String,
    pub recoverable: bool,
}

impl ResourceError {
    pub fn new(
        resource: Option<ResourceId>,
        message: impl Into<String>,
        recoverable: bool,
    ) -> Self {
        Self {
            resource,
            message: message.into(),
            recoverable,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformError {
    pub operation: String,
    pub message: String,
    pub recoverable: bool,
}

impl PlatformError {
    pub fn new(
        operation: impl Into<String>,
        message: impl Into<String>,
        recoverable: bool,
    ) -> Self {
        Self {
            operation: operation.into(),
            message: message.into(),
            recoverable,
        }
    }
}

/// A successfully diagnosed fallback, represented separately from hard errors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Degradation {
    pub capability: String,
    pub fallback: String,
    pub reason: String,
}

impl Degradation {
    pub fn new(
        capability: impl Into<String>,
        fallback: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            capability: capability.into(),
            fallback: fallback.into(),
            reason: reason.into(),
        }
    }
}

/// Unified error boundary for application-visible operations.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    UserInput(InputError),
    Compile(CompileError),
    Resource(ResourceError),
    Platform(PlatformError),
    Degraded(Degradation),
}

impl Error {
    pub fn invalid_input(field: impl Into<Option<String>>, message: impl Into<String>) -> Self {
        Self::UserInput(InputError::new(field, message))
    }

    pub fn compile(stage: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Compile(CompileError::new(stage, message))
    }

    pub fn resource(
        resource: Option<ResourceId>,
        message: impl Into<String>,
        recoverable: bool,
    ) -> Self {
        Self::Resource(ResourceError::new(resource, message, recoverable))
    }

    pub fn platform(
        operation: impl Into<String>,
        message: impl Into<String>,
        recoverable: bool,
    ) -> Self {
        Self::Platform(PlatformError::new(operation, message, recoverable))
    }

    pub fn degraded(
        capability: impl Into<String>,
        fallback: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::Degraded(Degradation::new(capability, fallback, reason))
    }

    pub const fn kind(&self) -> ErrorKind {
        match self {
            Self::UserInput(_) => ErrorKind::UserInput,
            Self::Compile(_) => ErrorKind::Compile,
            Self::Resource(_) => ErrorKind::Resource,
            Self::Platform(_) => ErrorKind::Platform,
            Self::Degraded(_) => ErrorKind::Degraded,
        }
    }

    pub const fn is_recoverable(&self) -> bool {
        match self {
            Self::UserInput(_) | Self::Compile(_) => false,
            Self::Resource(error) => error.recoverable,
            Self::Platform(error) => error.recoverable,
            Self::Degraded(_) => true,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UserInput(error) => match &error.field {
                Some(field) => write!(formatter, "invalid {field}: {}", error.message),
                None => write!(formatter, "invalid input: {}", error.message),
            },
            Self::Compile(error) => {
                write!(
                    formatter,
                    "{} compilation failed: {}",
                    error.stage, error.message
                )
            }
            Self::Resource(error) => match error.resource {
                Some(resource) => write!(formatter, "resource {resource:?}: {}", error.message),
                None => write!(formatter, "resource error: {}", error.message),
            },
            Self::Platform(error) => {
                write!(
                    formatter,
                    "platform {} failed: {}",
                    error.operation, error.message
                )
            }
            Self::Degraded(degradation) => write!(
                formatter,
                "{} degraded to {}: {}",
                degradation.capability, degradation.fallback, degradation.reason
            ),
        }
    }
}

impl StdError for Error {}

impl From<GeometryError> for Error {
    fn from(error: GeometryError) -> Self {
        Self::invalid_input(Some(error.field().to_owned()), error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_and_recovery_are_explicit() {
        let input = Error::invalid_input(Some("width".to_owned()), "must be positive");
        let fallback = Error::degraded("blur", "solid fill", "transient budget exhausted");

        assert_eq!(input.kind(), ErrorKind::UserInput);
        assert!(!input.is_recoverable());
        assert_eq!(fallback.kind(), ErrorKind::Degraded);
        assert!(fallback.is_recoverable());
        assert!(fallback.to_string().contains("solid fill"));
    }
}
