/// Cascade Error Types
#[derive(Debug, thiserror::Error)]
pub enum CascadeError {
    /// Git-related errors
    #[error("{0}")]
    Git(#[from] git2::Error),

    /// Configuration errors
    #[error("{0}")]
    Config(String),

    /// Branch management errors
    #[error("{0}")]
    Branch(String),

    /// Network errors
    #[error("{0}")]
    Network(String),

    /// Authentication errors
    #[error("{0}")]
    Auth(String),

    /// I/O errors
    #[error("{0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization errors
    #[error("{0}")]
    Json(#[from] serde_json::Error),

    /// HTTP client errors
    #[error("{0}")]
    Http(#[from] reqwest::Error),

    /// URL parsing errors
    #[error("{0}")]
    Url(#[from] url::ParseError),

    /// Conflict resolution errors
    #[error("{0}")]
    Conflict(String),

    /// Repository corruption errors
    #[error("{0}")]
    Corruption(String),

    /// Rebase operation errors
    #[error("{0}")]
    Rebase(String),

    /// Missing dependency errors
    #[error("{0}")]
    MissingDependency(String),

    /// API rate limit errors
    #[error("{0}")]
    RateLimit(String),

    /// Validation errors
    #[error("{0}")]
    Validation(String),
}

impl CascadeError {
    pub fn config<S: Into<String>>(msg: S) -> Self {
        CascadeError::Config(msg.into())
    }

    pub fn branch<S: Into<String>>(msg: S) -> Self {
        CascadeError::Branch(msg.into())
    }

    pub fn auth<S: Into<String>>(msg: S) -> Self {
        CascadeError::Auth(msg.into())
    }

    pub fn validation<S: Into<String>>(msg: S) -> Self {
        CascadeError::Validation(msg.into())
    }

    pub fn parse<S: Into<String>>(msg: S) -> Self {
        CascadeError::Validation(msg.into())
    }

    pub fn not_initialized<S: Into<String>>(msg: S) -> Self {
        CascadeError::config(msg.into())
    }

    pub fn invalid_operation<S: Into<String>>(msg: S) -> Self {
        CascadeError::Validation(msg.into())
    }

    pub fn conflict_resolution<S: Into<String>>(file: S, reason: S) -> Self {
        CascadeError::Conflict(format!("{}: {}", file.into(), reason.into()))
    }

    pub fn bitbucket_api(status: u16, message: String) -> Self {
        CascadeError::Conflict(format!("Bitbucket API error: {status} - {message}"))
    }

    pub fn bitbucket<S: Into<String>>(msg: S) -> Self {
        CascadeError::Conflict(msg.into())
    }

    /// Check if this error originated from git index lock contention.
    pub fn is_lock_error(&self) -> bool {
        match self {
            CascadeError::Git(e) => crate::utils::git_lock::is_lock_error(e),
            CascadeError::Branch(msg) | CascadeError::Config(msg) | CascadeError::Rebase(msg) => {
                msg.contains("index is locked") || msg.contains("index.lock")
            }
            _ => false,
        }
    }
}

pub type Result<T> = std::result::Result<T, CascadeError>;
