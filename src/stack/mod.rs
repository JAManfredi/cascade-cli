//! Stack management module
//! 
//! This module implements the core stacked diff functionality:
//! - Stack data structures and metadata
//! - Stack operations (create, push, pop, sync, rebase)
//! - Branch relationship management
//! - Commit tracking and dependencies

pub mod stack;
pub mod manager;
pub mod metadata;
pub mod rebase;

pub use stack::{Stack, StackEntry, StackStatus};
pub use manager::StackManager;
pub use metadata::{StackMetadata, CommitMetadata};
pub use rebase::{RebaseStrategy, RebaseManager, RebaseOptions, RebaseResult};
