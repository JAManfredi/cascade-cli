use super::{DynProvider, ProviderType, RepositoryProvider};
use crate::config::CascadeConfig;
use crate::errors::{CascadeError, Result};

/// Factory for creating repository providers
pub struct ProviderFactory;

impl ProviderFactory {
    /// Create a provider from the given configuration
    pub fn create_provider(config: &CascadeConfig) -> Result<DynProvider> {
        let provider_type = Self::detect_provider_type(config)?;

        match provider_type {
            ProviderType::Bitbucket => {
                let bitbucket_config = config
                    .bitbucket
                    .as_ref()
                    .ok_or_else(|| CascadeError::config("Bitbucket configuration not found"))?;

                let provider = super::bitbucket::BitbucketProvider::new(bitbucket_config.clone())?;
                Ok(Box::new(provider))
            }
            ProviderType::GitHub => {
                Err(CascadeError::config("GitHub provider not yet implemented"))
            }
            ProviderType::GitLab => {
                Err(CascadeError::config("GitLab provider not yet implemented"))
            }
        }
    }

    /// Try to create a provider, returning None if no valid configuration is found
    pub fn try_create_provider(config: &CascadeConfig) -> Option<DynProvider> {
        Self::create_provider(config).ok()
    }

    /// Detect the provider type from configuration
    fn detect_provider_type(config: &CascadeConfig) -> Result<ProviderType> {
        // For now, only support Bitbucket
        if config.bitbucket.is_some() {
            Ok(ProviderType::Bitbucket)
        } else {
            Err(CascadeError::config(
                "No supported repository provider configuration found. \
                Currently supported: Bitbucket Server",
            ))
        }
    }

    /// Get list of supported provider types
    pub fn supported_providers() -> Vec<ProviderType> {
        vec![ProviderType::Bitbucket]
    }

    /// Check if a provider type is supported
    pub fn is_provider_supported(provider_type: &ProviderType) -> bool {
        matches!(provider_type, ProviderType::Bitbucket)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::BitbucketConfig;

    #[test]
    fn test_supported_providers() {
        let supported = ProviderFactory::supported_providers();
        assert_eq!(supported.len(), 1);
        assert_eq!(supported[0], ProviderType::Bitbucket);
    }

    #[test]
    fn test_is_provider_supported() {
        assert!(ProviderFactory::is_provider_supported(
            &ProviderType::Bitbucket
        ));
        assert!(!ProviderFactory::is_provider_supported(
            &ProviderType::GitHub
        ));
        assert!(!ProviderFactory::is_provider_supported(
            &ProviderType::GitLab
        ));
    }

    #[test]
    fn test_detect_provider_type_bitbucket() {
        let mut config = CascadeConfig::default();
        config.bitbucket = Some(BitbucketConfig {
            url: "https://bitbucket.example.com".to_string(),
            project: "TEST".to_string(),
            repo: "test-repo".to_string(),
            username: None,
            token: None,
            default_reviewers: vec![],
        });

        let provider_type = ProviderFactory::detect_provider_type(&config).unwrap();
        assert_eq!(provider_type, ProviderType::Bitbucket);
    }

    #[test]
    fn test_detect_provider_type_none() {
        let config = CascadeConfig::default();
        let result = ProviderFactory::detect_provider_type(&config);
        assert!(result.is_err());
    }
}
