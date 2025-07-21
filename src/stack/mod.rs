//! Stack management module
//!
//! This module implements the core stacked diff functionality:
//! - Stack data structures and metadata
//! - Stack operations (create, push, pop, sync, rebase)
//! - Branch relationship management
//! - Commit tracking and dependencies

pub mod cleanup;
pub mod manager;
pub mod metadata;
pub mod rebase;
#[allow(clippy::module_inception)]
pub mod stack;

pub use cleanup::{
    CleanupCandidate, CleanupManager, CleanupOptions, CleanupReason, CleanupResult, CleanupStats,
};
pub use manager::StackManager;
pub use metadata::{CommitMetadata, EditModeState, StackMetadata};
pub use rebase::{RebaseManager, RebaseOptions, RebaseResult, RebaseStrategy};
pub use stack::{Stack, StackEntry, StackStatus};
