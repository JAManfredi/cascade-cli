use thiserror::Error;

#[derive(Error, Debug)]
pub enum CascadeError {
    #[error("Git operation failed: {0}")]
    Git(#[from] git2::Error),
    
    #[error("Bitbucket API error: {status} - {message}")]
    BitbucketApi { status: u16, message: String },
    
    #[error("Stack validation failed: {0}")]
    StackValidation(String),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Conflict resolution failed for {file}: {reason}")]
    ConflictResolution { file: String, reason: String },
    
    #[error("Branch operation failed: {0}")]
    Branch(String),
    
    #[error("Authentication failed: {0}")]
    Auth(String),
    
    #[error("Repository not initialized: {0}")]
    NotInitialized(String),
    
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
    
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Parse error: {0}")]
    Parse(String),
}

impl CascadeError {
    pub fn config<S: Into<String>>(msg: S) -> Self {
        CascadeError::Config(msg.into())
    }
    
    pub fn stack_validation<S: Into<String>>(msg: S) -> Self {
        CascadeError::StackValidation(msg.into())
    }
    
    pub fn branch<S: Into<String>>(msg: S) -> Self {
        CascadeError::Branch(msg.into())
    }
    
    pub fn auth<S: Into<String>>(msg: S) -> Self {
        CascadeError::Auth(msg.into())
    }
    
    pub fn not_initialized<S: Into<String>>(msg: S) -> Self {
        CascadeError::NotInitialized(msg.into())
    }
    
    pub fn invalid_operation<S: Into<String>>(msg: S) -> Self {
        CascadeError::InvalidOperation(msg.into())
    }
    
    pub fn parse<S: Into<String>>(msg: S) -> Self {
        CascadeError::Parse(msg.into())
    }
    
    pub fn conflict_resolution<S: Into<String>>(file: S, reason: S) -> Self {
        CascadeError::ConflictResolution {
            file: file.into(),
            reason: reason.into(),
        }
    }
    
    pub fn bitbucket_api(status: u16, message: String) -> Self {
        CascadeError::BitbucketApi { status, message }
    }
}

pub type Result<T> = std::result::Result<T, CascadeError>;