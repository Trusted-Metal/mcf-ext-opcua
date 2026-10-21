use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    Validation,
    UnsupportedSchemaVersion,
    LossyDowngrade,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionError {
    pub code: ErrorCode,
    /// Slash-joined location of the offending node; attributes are prefixed with `@`.
    pub path: String,
    pub message: String,
}

impl ExtensionError {
    pub fn new(code: ErrorCode, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code,
            path: path.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for ExtensionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} at {}: {}", self.code, self.path, self.message)
    }
}

impl std::error::Error for ExtensionError {}

pub type Result<T, E = ExtensionError> = std::result::Result<T, E>;

pub(crate) fn join_path(path: &str, name: &str) -> String {
    if path.is_empty() {
        name.to_string()
    } else {
        format!("{path}/{name}")
    }
}

pub(crate) fn validation(path: impl Into<String>, message: impl Into<String>) -> ExtensionError {
    ExtensionError::new(ErrorCode::Validation, path, message)
}
