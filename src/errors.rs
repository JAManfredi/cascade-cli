use thiserror::Error;

#[derive(Error, Debug)]
pub enum CascadeError {
    #[error("Git operation failed: {0}")]
    Git(#[from] git2::Error),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}