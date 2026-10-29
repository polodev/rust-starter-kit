use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaError {
    /// Nothing was uploaded.
    Empty,
    TooLarge {
        max_bytes: usize,
    },
    /// The file is not one of the accepted types (checked from its content, not its name).
    UnsupportedType,
    Storage(String),
    Database(String),
}

pub type MediaResult<T> = Result<T, MediaError>;

impl MediaError {
    /// Fluent key a form can show for this error, when it is the visitor's mistake.
    pub fn message_key(&self) -> &'static str {
        match self {
            Self::Empty => "logo-missing",
            Self::TooLarge { .. } => "logo-too-large",
            Self::UnsupportedType => "logo-invalid-type",
            Self::Storage(_) | Self::Database(_) => "request-rejected",
        }
    }

    pub fn is_user_error(&self) -> bool {
        matches!(
            self,
            Self::Empty | Self::TooLarge { .. } | Self::UnsupportedType
        )
    }
}

impl fmt::Display for MediaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "no file was uploaded"),
            Self::TooLarge { max_bytes } => write!(f, "the file is larger than {max_bytes} bytes"),
            Self::UnsupportedType => write!(f, "this file type is not accepted"),
            Self::Storage(message) => write!(f, "storage error: {message}"),
            Self::Database(message) => write!(f, "database error: {message}"),
        }
    }
}

impl std::error::Error for MediaError {}

impl From<sea_orm::DbErr> for MediaError {
    fn from(error: sea_orm::DbErr) -> Self {
        Self::Database(error.to_string())
    }
}

impl From<MediaError> for loco_rs::Error {
    fn from(error: MediaError) -> Self {
        if error.is_user_error() {
            loco_rs::Error::BadRequest(error.to_string())
        } else {
            loco_rs::Error::string(&error.to_string())
        }
    }
}
