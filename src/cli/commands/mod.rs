pub mod completions;
pub mod config;
pub mod doctor;
pub mod entry;
pub mod hooks;
pub mod init;
pub mod setup;
pub mod stack;
pub mod status;
pub mod tui;
pub mod version;
pub mod viz;

// Re-export commonly used types for CLI
pub use stack::{MergeStrategyArg, RebaseStrategyArg};
