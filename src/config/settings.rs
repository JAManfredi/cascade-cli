use serde::{Deserialize, Serialize};
use std::path::Path;
use std::fs;
use crate::errors::{CascadeError, Result};
use crate::config::auth::AuthConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CascadeConfig {
    pub bitbucket: Option<BitbucketConfig>,
    pub git: GitConfig,
    pub auth: AuthConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub default_sync_strategy: String,
    pub max_stack_size: usize,
    pub enable_notifications: bool,
    /// Rebase-specific settings
    pub rebase: RebaseSettings,
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
    /// Default branch versioning suffix pattern
    pub version_suffix_pattern: String,
    /// Whether to backup branches before rebasing
    pub backup_before_rebase: bool,
}

impl Default for CascadeConfig {
    fn default() -> Self {
        Self {
            bitbucket: None,
            git: GitConfig::default(),
            auth: AuthConfig::default(),
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            bitbucket: BitbucketConfig::default(),
            git: GitConfig::default(),
            cascade: CascadeSettings::default(),
        }
    }
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
            default_sync_strategy: "branch-versioning".to_string(),
            max_stack_size: 20,
            enable_notifications: true,
            rebase: RebaseSettings::default(),
        }
    }
}

impl Default for RebaseSettings {
    fn default() -> Self {
        Self {
            auto_resolve_conflicts: true,
            max_retry_attempts: 3,
            preserve_merges: true,
            version_suffix_pattern: "v{}".to_string(),
            backup_before_rebase: true,
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
            .map_err(|e| CascadeError::config(format!("Failed to read config file: {}", e)))?;
        
        let settings: Settings = serde_json::from_str(&content)
            .map_err(|e| CascadeError::config(format!("Failed to parse config file: {}", e)))?;
        
        Ok(settings)
    }
    
    /// Save settings to a file
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| CascadeError::config(format!("Failed to serialize config: {}", e)))?;
        
        fs::write(path, content)
            .map_err(|e| CascadeError::config(format!("Failed to write config file: {}", e)))?;
        
        Ok(())
    }
    
    /// Update a configuration value by key
    pub fn set_value(&mut self, key: &str, value: &str) -> Result<()> {
        let parts: Vec<&str> = key.split('.').collect();
        if parts.len() != 2 {
            return Err(CascadeError::config(format!("Invalid config key format: {}", key)));
        }
        
        match (parts[0], parts[1]) {
            ("bitbucket", "url") => self.bitbucket.url = value.to_string(),
            ("bitbucket", "project") => self.bitbucket.project = value.to_string(),
            ("bitbucket", "repo") => self.bitbucket.repo = value.to_string(),
            ("bitbucket", "token") => self.bitbucket.token = Some(value.to_string()),
            ("git", "default_branch") => self.git.default_branch = value.to_string(),
            ("git", "author_name") => self.git.author_name = Some(value.to_string()),
            ("git", "author_email") => self.git.author_email = Some(value.to_string()),
            ("git", "auto_cleanup_merged") => {
                self.git.auto_cleanup_merged = value.parse()
                    .map_err(|_| CascadeError::config(format!("Invalid boolean value: {}", value)))?;
            },
            ("git", "prefer_rebase") => {
                self.git.prefer_rebase = value.parse()
                    .map_err(|_| CascadeError::config(format!("Invalid boolean value: {}", value)))?;
            },
            ("cascade", "api_port") => {
                self.cascade.api_port = value.parse()
                    .map_err(|_| CascadeError::config(format!("Invalid port number: {}", value)))?;
            },
            ("cascade", "auto_cleanup") => {
                self.cascade.auto_cleanup = value.parse()
                    .map_err(|_| CascadeError::config(format!("Invalid boolean value: {}", value)))?;
            },
            ("cascade", "default_sync_strategy") => {
                self.cascade.default_sync_strategy = value.to_string();
            },
            ("cascade", "max_stack_size") => {
                self.cascade.max_stack_size = value.parse()
                    .map_err(|_| CascadeError::config(format!("Invalid number: {}", value)))?;
            },
            ("cascade", "enable_notifications") => {
                self.cascade.enable_notifications = value.parse()
                    .map_err(|_| CascadeError::config(format!("Invalid boolean value: {}", value)))?;
            },
            ("rebase", "auto_resolve_conflicts") => {
                self.cascade.rebase.auto_resolve_conflicts = value.parse()
                    .map_err(|_| CascadeError::config(format!("Invalid boolean value: {}", value)))?;
            },
            ("rebase", "max_retry_attempts") => {
                self.cascade.rebase.max_retry_attempts = value.parse()
                    .map_err(|_| CascadeError::config(format!("Invalid number: {}", value)))?;
            },
            ("rebase", "preserve_merges") => {
                self.cascade.rebase.preserve_merges = value.parse()
                    .map_err(|_| CascadeError::config(format!("Invalid boolean value: {}", value)))?;
            },
            ("rebase", "version_suffix_pattern") => {
                self.cascade.rebase.version_suffix_pattern = value.to_string();
            },
            ("rebase", "backup_before_rebase") => {
                self.cascade.rebase.backup_before_rebase = value.parse()
                    .map_err(|_| CascadeError::config(format!("Invalid boolean value: {}", value)))?;
            },
            _ => return Err(CascadeError::config(format!("Unknown config key: {}", key))),
        }
        
        Ok(())
    }
    
    /// Get a configuration value by key
    pub fn get_value(&self, key: &str) -> Result<String> {
        let parts: Vec<&str> = key.split('.').collect();
        if parts.len() != 2 {
            return Err(CascadeError::config(format!("Invalid config key format: {}", key)));
        }
        
        let value = match (parts[0], parts[1]) {
            ("bitbucket", "url") => &self.bitbucket.url,
            ("bitbucket", "project") => &self.bitbucket.project,
            ("bitbucket", "repo") => &self.bitbucket.repo,
            ("bitbucket", "token") => self.bitbucket.token.as_deref().unwrap_or(""),
            ("git", "default_branch") => &self.git.default_branch,
            ("git", "author_name") => self.git.author_name.as_deref().unwrap_or(""),
            ("git", "author_email") => self.git.author_email.as_deref().unwrap_or(""),
            ("git", "auto_cleanup_merged") => return Ok(self.git.auto_cleanup_merged.to_string()),
            ("git", "prefer_rebase") => return Ok(self.git.prefer_rebase.to_string()),
            ("cascade", "api_port") => return Ok(self.cascade.api_port.to_string()),
            ("cascade", "auto_cleanup") => return Ok(self.cascade.auto_cleanup.to_string()),
            ("cascade", "default_sync_strategy") => &self.cascade.default_sync_strategy,
            ("cascade", "max_stack_size") => return Ok(self.cascade.max_stack_size.to_string()),
            ("cascade", "enable_notifications") => return Ok(self.cascade.enable_notifications.to_string()),
            ("rebase", "auto_resolve_conflicts") => return Ok(self.cascade.rebase.auto_resolve_conflicts.to_string()),
            ("rebase", "max_retry_attempts") => return Ok(self.cascade.rebase.max_retry_attempts.to_string()),
            ("rebase", "preserve_merges") => return Ok(self.cascade.rebase.preserve_merges.to_string()),
            ("rebase", "version_suffix_pattern") => &self.cascade.rebase.version_suffix_pattern,
            ("rebase", "backup_before_rebase") => return Ok(self.cascade.rebase.backup_before_rebase.to_string()),
            _ => return Err(CascadeError::config(format!("Unknown config key: {}", key))),
        };
        
        Ok(value.to_string())
    }
    
    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        // Validate Bitbucket configuration if provided
        if !self.bitbucket.url.is_empty() {
            if !self.bitbucket.url.starts_with("http://") && !self.bitbucket.url.starts_with("https://") {
                return Err(CascadeError::config("Bitbucket URL must start with http:// or https://"));
            }
        }
        
        // Validate port
        if self.cascade.api_port == 0 {
            return Err(CascadeError::config("API port must be between 1 and 65535"));
        }
        
        // Validate sync strategy
        let valid_strategies = ["rebase", "cherry-pick", "branch-versioning", "three-way-merge"];
        if !valid_strategies.contains(&self.cascade.default_sync_strategy.as_str()) {
            return Err(CascadeError::config(format!(
                "Invalid sync strategy: {}. Valid options: {}", 
                self.cascade.default_sync_strategy,
                valid_strategies.join(", ")
            )));
        }
        
        Ok(())
    }
} 