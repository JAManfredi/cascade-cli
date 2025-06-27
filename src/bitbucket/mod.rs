//! Bitbucket Server integration module
//! 
//! This module provides integration with Bitbucket Server for:
//! - API client for Bitbucket Server
//! - Authentication handling
//! - Pull request management
//! - Repository operations

pub mod client;
pub mod pull_request;
pub mod integration;

pub use client::BitbucketClient;
pub use pull_request::{
    PullRequestManager, CreatePullRequestRequest, PullRequest, PullRequestRef,
    PullRequestState, Repository, Project, Participant, User
};
pub use integration::{BitbucketIntegration, StackSubmissionStatus};

// Placeholder to satisfy module import in lib.rs
// This will be implemented in Phase 3: Bitbucket Server Integration
