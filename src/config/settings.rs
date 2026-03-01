use crate::config::auth::AuthConfig;
use crate::errors::{CascadeError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CascadeConfig {
    pub bitbucket: Option<BitbucketConfig>,
    pub git: GitConfig,
    pub auth: AuthConfig,
    pub cascade: CascadeSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    pub bitbucket: BitbucketConfig,
    pub git: GitConfig,
    pub cascade: CascadeSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitbucketConfig {
    pub url: String,
    pub project: String,
    pub repo: String,
    pub username: Option<String>,
    pub token: Option<String>,
    pub default_reviewers: Vec<String>,
    /// Accept invalid TLS certificates (development only)
    pub accept_invalid_certs: Option<bool>,
    /// Path to custom CA certificate bundle
    pub ca_bundle_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConfig {
    pub default_branch: String,
    pub author_name: Option<String>,
    pub author_email: Option<String>,
    pub auto_cleanup_merged: bool,
    pub prefer_rebase: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CascadeSettings {
    pub api_port: u16,
    pub auto_cleanup: bool,
    pub max_stack_size: usize,
    pub enable_notifications: bool,
    /// Default PR description template (markdown supported)
    pub pr_description_template: Option<String>,
    /// Veto message patterns to treat as advisory (non-blocking) during merge checks.
    /// When the only remaining vetoes match these patterns, the PR is treated as mergeable.
    /// Example: ["Code Owners"] to treat Code Owners checks as advisory.
    #[serde(default)]
    pub advisory_merge_checks: Vec<String>,
    /// Rebase-specific settings
    pub rebase: RebaseSettings,
    /// DEPRECATED: Old sync strategy setting (ignored, kept for backward compatibility)
    #[serde(default, skip_serializing)]
    pub default_sync_strategy: Option<String>,
}

/// Settings specific to rebase operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebaseSettings {
    /// Whether to auto-resolve simple conflicts
    pub auto_resolve_conflicts: bool,
    /// Maximum number of retry attempts for rebase operations
    pub max_retry_attempts: usize,
    /// Whether to preserve merge commits during rebase
    pub preserve_merges: bool,
    /// Whether to backup branches before rebasing (creates backup-* branches)
    pub backup_before_rebase: bool,
    /// DEPRECATED: Old version suffix pattern (ignored, kept for backward compatibility)
    #[serde(default, skip_serializing)]
    pub version_suffix_pattern: Option<String>,
}

impl Default for BitbucketConfig {
    fn default() -> Self {
        Self {
            url: "https://bitbucket.example.com".to_string(),
            project: "PROJECT".to_string(),
            repo: "repo".to_string(),
            username: None,
            token: None,
            default_reviewers: Vec::new(),
            accept_invalid_certs: None,
            ca_bundle_path: None,
        }
    }
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            default_branch: "main".to_string(),
            author_name: None,
            author_email: None,
            auto_cleanup_merged: true,
            prefer_rebase: true,
        }
    }
}

impl Default for CascadeSettings {
    fn default() -> Self {
        Self {
            api_port: 8080,
            auto_cleanup: true,
            max_stack_size: 20,
            enable_notifications: true,
            pr_description_template: None,
            advisory_merge_checks: Vec::new(),
            rebase: RebaseSettings::default(),
            default_sync_strategy: None, // Deprecated field
        }
    }
}

impl Default for RebaseSettings {
    fn default() -> Self {
        Self {
            auto_resolve_conflicts: true,
            max_retry_attempts: 3,
            preserve_merges: true,
            backup_before_rebase: true,
            version_suffix_pattern: None, // Deprecated field
        }
    }
}

impl Settings {
    /// Create default settings for a repository
    pub fn default_for_repo(bitbucket_url: Option<String>) -> Self {
        let mut settings = Self::default();
        if let Some(url) = bitbucket_url {
            settings.bitbucket.url = url;
        }
        settings
    }

    /// Load settings from a file
    pub fn load_from_file(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path)
            .map_err(|e| CascadeError::config(format!("Failed to read config file: {e}")))?;

        let settings: Settings = serde_json::from_str(&content)
            .map_err(|e| CascadeError::config(format!("Failed to parse config file: {e}")))?;

        Ok(settings)
    }

    /// Save settings to a file atomically
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        crate::utils::atomic_file::write_json(path, self)
    }

    /// Update a configuration value by key
    pub fn set_value(&mut self, key: &str, value: &str) -> Result<()> {
        let parts: Vec<&str> = key.split('.').collect();
        if parts.len() != 2 {
            return Err(CascadeError::config(format!(
                "Invalid config key format: {key}"
            )));
        }

        match (parts[0], parts[1]) {
            ("bitbucket", "url") => self.bitbucket.url = value.to_string(),
            ("bitbucket", "project") => self.bitbucket.project = value.to_string(),
            ("bitbucket", "repo") => self.bitbucket.repo = value.to_string(),
            ("bitbucket", "username") => self.bitbucket.username = Some(value.to_string()),
            ("bitbucket", "token") => self.bitbucket.token = Some(value.to_string()),
            ("bitbucket", "accept_invalid_certs") => {
                self.bitbucket.accept_invalid_certs = Some(value.parse().map_err(|_| {
                    CascadeError::config(format!("Invalid boolean value: {value}"))
                })?);
            }
            ("bitbucket", "ca_bundle_path") => {
                self.bitbucket.ca_bundle_path = Some(value.to_string());
            }
            ("git", "default_branch") => self.git.default_branch = value.to_string(),
            ("git", "author_name") => self.git.author_name = Some(value.to_string()),
            ("git", "author_email") => self.git.author_email = Some(value.to_string()),
            ("git", "auto_cleanup_merged") => {
                self.git.auto_cleanup_merged = value
                    .parse()
                    .map_err(|_| CascadeError::config(format!("Invalid boolean value: {value}")))?;
            }
            ("git", "prefer_rebase") => {
                self.git.prefer_rebase = value
                    .parse()
                    .map_err(|_| CascadeError::config(format!("Invalid boolean value: {value}")))?;
            }
            ("cascade", "api_port") => {
                self.cascade.api_port = value
                    .parse()
                    .map_err(|_| CascadeError::config(format!("Invalid port number: {value}")))?;
            }
            ("cascade", "auto_cleanup") => {
                self.cascade.auto_cleanup = value
                    .parse()
                    .map_err(|_| CascadeError::config(format!("Invalid boolean value: {value}")))?;
            }
            ("cascade", "max_stack_size") => {
                self.cascade.max_stack_size = value
                    .parse()
                    .map_err(|_| CascadeError::config(format!("Invalid number: {value}")))?;
            }
            ("cascade", "enable_notifications") => {
                self.cascade.enable_notifications = value
                    .parse()
                    .map_err(|_| CascadeError::config(format!("Invalid boolean value: {value}")))?;
            }
            ("cascade", "pr_description_template") => {
                self.cascade.pr_description_template = if value.is_empty() {
                    None
                } else {
                    Some(value.to_string())
                };
            }
            ("cascade", "advisory_merge_checks") => {
                if value.is_empty() {
                    self.cascade.advisory_merge_checks = Vec::new();
                } else {
                    // Accept JSON array or comma-separated values
                    if let Ok(parsed) = serde_json::from_str::<Vec<String>>(value) {
                        self.cascade.advisory_merge_checks = parsed;
                    } else {
                        self.cascade.advisory_merge_checks =
                            value.split(',').map(|s| s.trim().to_string()).collect();
                    }
                }
            }
            ("rebase", "auto_resolve_conflicts") => {
                self.cascade.rebase.auto_resolve_conflicts = value
                    .parse()
                    .map_err(|_| CascadeError::config(format!("Invalid boolean value: {value}")))?;
            }
            ("rebase", "max_retry_attempts") => {
                self.cascade.rebase.max_retry_attempts = value
                    .parse()
                    .map_err(|_| CascadeError::config(format!("Invalid number: {value}")))?;
            }
            ("rebase", "preserve_merges") => {
                self.cascade.rebase.preserve_merges = value
                    .parse()
                    .map_err(|_| CascadeError::config(format!("Invalid boolean value: {value}")))?;
            }
            ("rebase", "backup_before_rebase") => {
                self.cascade.rebase.backup_before_rebase = value
                    .parse()
                    .map_err(|_| CascadeError::config(format!("Invalid boolean value: {value}")))?;
            }
            _ => return Err(CascadeError::config(format!("Unknown config key: {key}"))),
        }

        Ok(())
    }

    /// Get a configuration value by key
    pub fn get_value(&self, key: &str) -> Result<String> {
        let parts: Vec<&str> = key.split('.').collect();
        if parts.len() != 2 {
            return Err(CascadeError::config(format!(
                "Invalid config key format: {key}"
            )));
        }

        let value = match (parts[0], parts[1]) {
            ("bitbucket", "url") => &self.bitbucket.url,
            ("bitbucket", "project") => &self.bitbucket.project,
            ("bitbucket", "repo") => &self.bitbucket.repo,
            ("bitbucket", "username") => self.bitbucket.username.as_deref().unwrap_or(""),
            ("bitbucket", "token") => self.bitbucket.token.as_deref().unwrap_or(""),
            ("bitbucket", "accept_invalid_certs") => {
                return Ok(self
                    .bitbucket
                    .accept_invalid_certs
                    .unwrap_or(false)
                    .to_string())
            }
            ("bitbucket", "ca_bundle_path") => {
                self.bitbucket.ca_bundle_path.as_deref().unwrap_or("")
            }
            ("git", "default_branch") => &self.git.default_branch,
            ("git", "author_name") => self.git.author_name.as_deref().unwrap_or(""),
            ("git", "author_email") => self.git.author_email.as_deref().unwrap_or(""),
            ("git", "auto_cleanup_merged") => return Ok(self.git.auto_cleanup_merged.to_string()),
            ("git", "prefer_rebase") => return Ok(self.git.prefer_rebase.to_string()),
            ("cascade", "api_port") => return Ok(self.cascade.api_port.to_string()),
            ("cascade", "auto_cleanup") => return Ok(self.cascade.auto_cleanup.to_string()),
            ("cascade", "max_stack_size") => return Ok(self.cascade.max_stack_size.to_string()),
            ("cascade", "enable_notifications") => {
                return Ok(self.cascade.enable_notifications.to_string())
            }
            ("cascade", "pr_description_template") => self
                .cascade
                .pr_description_template
                .as_deref()
                .unwrap_or(""),
            ("cascade", "advisory_merge_checks") => {
                return Ok(serde_json::to_string(&self.cascade.advisory_merge_checks)
                    .unwrap_or_else(|_| "[]".to_string()))
            }
            ("rebase", "auto_resolve_conflicts") => {
                return Ok(self.cascade.rebase.auto_resolve_conflicts.to_string())
            }
            ("rebase", "max_retry_attempts") => {
                return Ok(self.cascade.rebase.max_retry_attempts.to_string())
            }
            ("rebase", "preserve_merges") => {
                return Ok(self.cascade.rebase.preserve_merges.to_string())
            }
            ("rebase", "backup_before_rebase") => {
                return Ok(self.cascade.rebase.backup_before_rebase.to_string())
            }
            _ => return Err(CascadeError::config(format!("Unknown config key: {key}"))),
        };

        Ok(value.to_string())
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        // Validate Bitbucket configuration if provided
        if !self.bitbucket.url.is_empty()
            && !self.bitbucket.url.starts_with("http://")
            && !self.bitbucket.url.starts_with("https://")
        {
            return Err(CascadeError::config(
                "Bitbucket URL must start with http:// or https://",
            ));
        }

        // Validate port
        if self.cascade.api_port == 0 {
            return Err(CascadeError::config("API port must be between 1 and 65535"));
        }

        // Validate sync strategy
        // All rebase operations now use force-push strategy by default
        // No validation needed for sync strategy
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backward_compatibility_with_old_config_format() {
        // Simulate an old config file with deprecated fields
        let old_config_json = r#"{
            "bitbucket": {
                "url": "https://bitbucket.example.com",
                "project": "TEST",
                "repo": "test-repo",
                "username": null,
                "token": null,
                "default_reviewers": [],
                "accept_invalid_certs": null,
                "ca_bundle_path": null
            },
            "git": {
                "default_branch": "main",
                "author_name": null,
                "author_email": null,
                "auto_cleanup_merged": true,
                "prefer_rebase": true
            },
            "cascade": {
                "api_port": 8080,
                "auto_cleanup": true,
                "default_sync_strategy": "branch-versioning",
                "max_stack_size": 20,
                "enable_notifications": true,
                "pr_description_template": null,
                "rebase": {
                    "auto_resolve_conflicts": true,
                    "max_retry_attempts": 3,
                    "preserve_merges": true,
                    "version_suffix_pattern": "v{}",
                    "backup_before_rebase": true
                }
            }
        }"#;

        // Should successfully parse old config format
        let settings: Settings = serde_json::from_str(old_config_json)
            .expect("Failed to parse old config format - backward compatibility broken!");

        // Verify main settings are preserved
        assert_eq!(settings.cascade.api_port, 8080);
        assert!(settings.cascade.auto_cleanup);
        assert_eq!(settings.cascade.max_stack_size, 20);

        // Verify deprecated fields were loaded but are ignored
        assert_eq!(
            settings.cascade.default_sync_strategy,
            Some("branch-versioning".to_string())
        );
        assert_eq!(
            settings.cascade.rebase.version_suffix_pattern,
            Some("v{}".to_string())
        );

        // Verify that when saved, deprecated fields are NOT included
        let new_json =
            serde_json::to_string_pretty(&settings).expect("Failed to serialize settings");

        assert!(
            !new_json.contains("default_sync_strategy"),
            "Deprecated field should not appear in new config files"
        );
        assert!(
            !new_json.contains("version_suffix_pattern"),
            "Deprecated field should not appear in new config files"
        );
    }

    #[test]
    fn test_new_config_format_without_deprecated_fields() {
        // Simulate a new config file without deprecated fields
        let new_config_json = r#"{
            "bitbucket": {
                "url": "https://bitbucket.example.com",
                "project": "TEST",
                "repo": "test-repo",
                "username": null,
                "token": null,
                "default_reviewers": [],
                "accept_invalid_certs": null,
                "ca_bundle_path": null
            },
            "git": {
                "default_branch": "main",
                "author_name": null,
                "author_email": null,
                "auto_cleanup_merged": true,
                "prefer_rebase": true
            },
            "cascade": {
                "api_port": 8080,
                "auto_cleanup": true,
                "max_stack_size": 20,
                "enable_notifications": true,
                "pr_description_template": null,
                "rebase": {
                    "auto_resolve_conflicts": true,
                    "max_retry_attempts": 3,
                    "preserve_merges": true,
                    "backup_before_rebase": true
                }
            }
        }"#;

        // Should successfully parse new config format
        let settings: Settings =
            serde_json::from_str(new_config_json).expect("Failed to parse new config format!");

        // Verify deprecated fields default to None
        assert_eq!(settings.cascade.default_sync_strategy, None);
        assert_eq!(settings.cascade.rebase.version_suffix_pattern, None);
    }
}
