/// Cascade Error Types
#[derive(Debug, thiserror::Error)]
pub enum CascadeError {
    /// Git-related errors
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Branch management errors
    #[error("Branch error: {0}")]
    Branch(String),

    /// Network errors
    #[error("Network error: {0}")]
    Network(String),

    /// Authentication errors
    #[error("Authentication error: {0}")]
    Auth(String),

    /// I/O errors
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization errors
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// HTTP client errors
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// URL parsing errors
    #[error("URL error: {0}")]
    Url(#[from] url::ParseError),

    /// Conflict resolution errors
    #[error("Conflict error: {0}")]
    Conflict(String),

    /// Repository corruption errors
    #[error("Repository corruption: {0}")]
    Corruption(String),

    /// Rebase operation errors
    #[error("Rebase error: {0}")]
    Rebase(String),

    /// Missing dependency errors
    #[error("Missing dependency: {0}")]
    MissingDependency(String),

    /// API rate limit errors
    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    /// Validation errors
    #[error("Validation error: {0}")]
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
}

pub type Result<T> = std::result::Result<T, CascadeError>;
