use serde::{Deserialize, Serialize};
use std::path::Path;
use std::fs;
use crate::errors::{CascadeError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub bitbucket: BitbucketConfig,
    pub git: GitConfig,
    pub cascade: CascadeConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitbucketConfig {
    pub server_url: Option<String>,
    pub project_key: Option<String>,
    pub repo_slug: Option<String>,
    pub auth_token: Option<String>,
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
pub struct CascadeConfig {
    pub api_port: u16,
    pub auto_cleanup: bool,
    pub default_sync_strategy: String,
    pub max_stack_size: usize,
    pub enable_notifications: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            bitbucket: BitbucketConfig::default(),
            git: GitConfig::default(),
            cascade: CascadeConfig::default(),
        }
    }
}

impl Default for BitbucketConfig {
    fn default() -> Self {
        Self {
            server_url: None,
            project_key: None,
            repo_slug: None,
            auth_token: None,
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

impl Default for CascadeConfig {
    fn default() -> Self {
        Self {
            api_port: 8080,
            auto_cleanup: true,
            default_sync_strategy: "branch-versioning".to_string(),
            max_stack_size: 20,
            enable_notifications: true,
        }
    }
}

impl Settings {
    /// Create default settings for a repository
    pub fn default_for_repo(bitbucket_url: Option<String>) -> Self {
        let mut settings = Self::default();
        if let Some(url) = bitbucket_url {
            settings.bitbucket.server_url = Some(url);
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
            ("bitbucket", "url") => self.bitbucket.server_url = Some(value.to_string()),
            ("bitbucket", "project") => self.bitbucket.project_key = Some(value.to_string()),
            ("bitbucket", "repo") => self.bitbucket.repo_slug = Some(value.to_string()),
            ("bitbucket", "token") => self.bitbucket.auth_token = Some(value.to_string()),
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
            ("bitbucket", "url") => self.bitbucket.server_url.as_deref().unwrap_or(""),
            ("bitbucket", "project") => self.bitbucket.project_key.as_deref().unwrap_or(""),
            ("bitbucket", "repo") => self.bitbucket.repo_slug.as_deref().unwrap_or(""),
            ("bitbucket", "token") => self.bitbucket.auth_token.as_deref().unwrap_or(""),
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
            _ => return Err(CascadeError::config(format!("Unknown config key: {}", key))),
        };
        
        Ok(value.to_string())
    }
    
    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        // Validate Bitbucket configuration if provided
        if let Some(url) = &self.bitbucket.server_url {
            if !url.starts_with("http://") && !url.starts_with("https://") {
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