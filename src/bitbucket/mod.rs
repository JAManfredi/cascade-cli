//! Bitbucket Server integration module
//!
//! This module provides integration with Bitbucket Server for:
//! - API client for Bitbucket Server
//! - Authentication handling
//! - Pull request management
//! - Repository operations

pub mod client;
pub mod integration;
pub mod pull_request;

pub use client::BitbucketClient;
pub use integration::{BitbucketIntegration, StackSubmissionStatus};
pub use pull_request::{
    CreatePullRequestRequest, Participant, Project, PullRequest, PullRequestManager,
    PullRequestRef, PullRequestState, Repository, User,
};

// Placeholder to satisfy module import in lib.rs
// This will be implemented in Phase 3: Bitbucket Server Integration
