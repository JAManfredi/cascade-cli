use serde::{Deserialize, Serialize};
use std::path::Path;
use std::fs;
use crate::errors::{CascadeError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub bitbucket_tokens: std::collections::HashMap<String, String>,
    pub default_server: Option<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            bitbucket_tokens: std::collections::HashMap::new(),
            default_server: None,
        }
    }
}

pub struct AuthManager {
    config: AuthConfig,
    config_path: std::path::PathBuf,
}

impl AuthManager {
    /// Create a new AuthManager
    pub fn new(config_dir: &Path) -> Result<Self> {
        let config_path = config_dir.join("auth.json");
        let config = if config_path.exists() {
            AuthConfig::load_from_file(&config_path)?
        } else {
            AuthConfig::default()
        };
        
        Ok(Self {
            config,
            config_path,
        })
    }
    
    /// Store an authentication token for a Bitbucket server
    pub fn store_token(&mut self, server_url: &str, token: &str) -> Result<()> {
        self.config.bitbucket_tokens.insert(server_url.to_string(), token.to_string());
        self.save()?;
        tracing::info!("Stored authentication token for {}", server_url);
        Ok(())
    }
    
    /// Retrieve an authentication token for a Bitbucket server
    pub fn get_token(&self, server_url: &str) -> Option<&String> {
        self.config.bitbucket_tokens.get(server_url)
    }
    
    /// Remove an authentication token
    pub fn remove_token(&mut self, server_url: &str) -> Result<bool> {
        let removed = self.config.bitbucket_tokens.remove(server_url).is_some();
        if removed {
            self.save()?;
            tracing::info!("Removed authentication token for {}", server_url);
        }
        Ok(removed)
    }
    
    /// List all configured servers
    pub fn list_servers(&self) -> Vec<&String> {
        self.config.bitbucket_tokens.keys().collect()
    }
    
    /// Set the default server
    pub fn set_default_server(&mut self, server_url: &str) -> Result<()> {
        if !self.config.bitbucket_tokens.contains_key(server_url) {
            return Err(CascadeError::auth(format!(
                "No token configured for server: {}", 
                server_url
            )));
        }
        
        self.config.default_server = Some(server_url.to_string());
        self.save()?;
        tracing::info!("Set default server to {}", server_url);
        Ok(())
    }
    
    /// Get the default server
    pub fn get_default_server(&self) -> Option<&String> {
        self.config.default_server.as_ref()
    }
    
    /// Validate that we have authentication for a server
    pub fn validate_auth(&self, server_url: &str) -> Result<()> {
        if self.get_token(server_url).is_none() {
            return Err(CascadeError::auth(format!(
                "No authentication token configured for server: {}. Use 'cc config set bitbucket.token <token>' to configure.",
                server_url
            )));
        }
        Ok(())
    }
    
    /// Save the configuration to disk
    fn save(&self) -> Result<()> {
        self.config.save_to_file(&self.config_path)
    }
}

impl AuthConfig {
    /// Load authentication config from a file
    pub fn load_from_file(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        
        let content = fs::read_to_string(path)
            .map_err(|e| CascadeError::config(format!("Failed to read auth config: {}", e)))?;
        
        let config: AuthConfig = serde_json::from_str(&content)
            .map_err(|e| CascadeError::config(format!("Failed to parse auth config: {}", e)))?;
        
        Ok(config)
    }
    
    /// Save authentication config to a file
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| CascadeError::config(format!("Failed to create config directory: {}", e)))?;
        }
        
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| CascadeError::config(format!("Failed to serialize auth config: {}", e)))?;
        
        // Write to temporary file first, then rename for atomic write
        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, content)
            .map_err(|e| CascadeError::config(format!("Failed to write auth config: {}", e)))?;
        
        fs::rename(&temp_path, path)
            .map_err(|e| CascadeError::config(format!("Failed to finalize auth config: {}", e)))?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_auth_manager_basic_operations() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path();
        
        let mut auth_manager = AuthManager::new(config_dir).unwrap();
        
        // Test storing and retrieving tokens
        auth_manager.store_token("https://bitbucket.company.com", "test-token").unwrap();
        assert_eq!(auth_manager.get_token("https://bitbucket.company.com"), Some(&"test-token".to_string()));
        
        // Test setting default server
        auth_manager.set_default_server("https://bitbucket.company.com").unwrap();
        assert_eq!(auth_manager.get_default_server(), Some(&"https://bitbucket.company.com".to_string()));
        
        // Test validation
        auth_manager.validate_auth("https://bitbucket.company.com").unwrap();
        assert!(auth_manager.validate_auth("https://unknown.server.com").is_err());
        
        // Test removing tokens
        assert!(auth_manager.remove_token("https://bitbucket.company.com").unwrap());
        assert!(!auth_manager.remove_token("https://bitbucket.company.com").unwrap());
    }
} 