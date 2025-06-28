use crate::config::BitbucketConfig;
use crate::errors::{CascadeError, Result};
use base64::Engine;
use reqwest::{
    header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE},
    Client,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, trace};

/// Bitbucket Server API client
pub struct BitbucketClient {
    client: Client,
    base_url: String,
    project_key: String,
    repo_slug: String,
}

impl BitbucketClient {
    /// Create a new Bitbucket client
    pub fn new(config: &BitbucketConfig) -> Result<Self> {
        let mut headers = HeaderMap::new();

        // Set up authentication
        let auth_header = match (&config.username, &config.token) {
            (Some(username), Some(token)) => {
                let auth_string = format!("{}:{}", username, token);
                let auth_encoded = base64::engine::general_purpose::STANDARD.encode(auth_string);
                format!("Basic {}", auth_encoded)
            }
            (None, Some(token)) => {
                format!("Bearer {}", token)
            }
            _ => {
                return Err(CascadeError::config(
                    "Bitbucket authentication credentials not configured",
                ))
            }
        };

        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth_header)
                .map_err(|e| CascadeError::config(format!("Invalid auth header: {}", e)))?,
        );

        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .default_headers(headers)
            .build()
            .map_err(|e| CascadeError::config(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            client,
            base_url: config.url.clone(),
            project_key: config.project.clone(),
            repo_slug: config.repo.clone(),
        })
    }

    /// Get the base API URL for this repository
    fn api_url(&self, path: &str) -> String {
        format!(
            "{}/rest/api/1.0/projects/{}/repos/{}/{}",
            self.base_url.trim_end_matches('/'),
            self.project_key,
            self.repo_slug,
            path.trim_start_matches('/')
        )
    }

    /// Make a GET request to the Bitbucket API
    pub async fn get<T>(&self, path: &str) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let url = self.api_url(path);
        debug!("GET {}", url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| CascadeError::bitbucket(format!("GET request failed: {}", e)))?;

        self.handle_response(response).await
    }

    /// Make a POST request to the Bitbucket API
    pub async fn post<T, U>(&self, path: &str, body: &T) -> Result<U>
    where
        T: Serialize,
        U: for<'de> Deserialize<'de>,
    {
        let url = self.api_url(path);
        debug!("POST {}", url);

        let response = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| CascadeError::bitbucket(format!("POST request failed: {}", e)))?;

        self.handle_response(response).await
    }

    /// Make a PUT request to the Bitbucket API
    pub async fn put<T, U>(&self, path: &str, body: &T) -> Result<U>
    where
        T: Serialize,
        U: for<'de> Deserialize<'de>,
    {
        let url = self.api_url(path);
        debug!("PUT {}", url);

        let response = self
            .client
            .put(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| CascadeError::bitbucket(format!("PUT request failed: {}", e)))?;

        self.handle_response(response).await
    }

    /// Make a DELETE request to the Bitbucket API
    pub async fn delete(&self, path: &str) -> Result<()> {
        let url = self.api_url(path);
        debug!("DELETE {}", url);

        let response = self
            .client
            .delete(&url)
            .send()
            .await
            .map_err(|e| CascadeError::bitbucket(format!("DELETE request failed: {}", e)))?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(CascadeError::bitbucket(format!(
                "DELETE failed with status {}: {}",
                status, text
            )))
        }
    }

    /// Handle HTTP response and deserialize JSON
    async fn handle_response<T>(&self, response: reqwest::Response) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let status = response.status();

        if status.is_success() {
            let text = response.text().await.map_err(|e| {
                CascadeError::bitbucket(format!("Failed to read response body: {}", e))
            })?;

            trace!("Response body: {}", text);

            serde_json::from_str(&text).map_err(|e| {
                CascadeError::bitbucket(format!("Failed to parse JSON response: {}", e))
            })
        } else {
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(CascadeError::bitbucket(format!(
                "Request failed with status {}: {}",
                status, text
            )))
        }
    }

    /// Test the connection to Bitbucket Server
    pub async fn test_connection(&self) -> Result<()> {
        let url = format!(
            "{}/rest/api/1.0/projects/{}/repos/{}",
            self.base_url.trim_end_matches('/'),
            self.project_key,
            self.repo_slug
        );

        debug!("Testing connection to {}", url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| CascadeError::bitbucket(format!("Connection test failed: {}", e)))?;

        if response.status().is_success() {
            debug!("Connection test successful");
            Ok(())
        } else {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(CascadeError::bitbucket(format!(
                "Connection test failed with status {}: {}",
                status, text
            )))
        }
    }

    /// Get repository information
    pub async fn get_repository_info(&self) -> Result<RepositoryInfo> {
        self.get("").await
    }
}

/// Repository information from Bitbucket
#[derive(Debug, Clone, Deserialize)]
pub struct RepositoryInfo {
    pub id: u64,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub public: bool,
    pub project: ProjectInfo,
    pub links: RepositoryLinks,
}

/// Project information
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectInfo {
    pub id: u64,
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub public: bool,
}

/// Repository links
#[derive(Debug, Clone, Deserialize)]
pub struct RepositoryLinks {
    pub clone: Vec<CloneLink>,
    #[serde(rename = "self")]
    pub self_link: Vec<SelfLink>,
}

/// Clone link information
#[derive(Debug, Clone, Deserialize)]
pub struct CloneLink {
    pub href: String,
    pub name: String,
}

/// Self link information
#[derive(Debug, Clone, Deserialize)]
pub struct SelfLink {
    pub href: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_url_generation() {
        let config = BitbucketConfig {
            url: "https://bitbucket.example.com".to_string(),
            project: "TEST".to_string(),
            repo: "my-repo".to_string(),
            username: Some("user".to_string()),
            token: Some("token".to_string()),
            default_reviewers: Vec::new(),
        };

        let client = BitbucketClient::new(&config).unwrap();

        assert_eq!(
            client.api_url("pull-requests"),
            "https://bitbucket.example.com/rest/api/1.0/projects/TEST/repos/my-repo/pull-requests"
        );

        assert_eq!(
            client.api_url("/pull-requests/123"),
            "https://bitbucket.example.com/rest/api/1.0/projects/TEST/repos/my-repo/pull-requests/123"
        );
    }

    #[test]
    fn test_url_trimming() {
        let config = BitbucketConfig {
            url: "https://bitbucket.example.com/".to_string(), // Note trailing slash
            project: "TEST".to_string(),
            repo: "my-repo".to_string(),
            username: Some("user".to_string()),
            token: Some("token".to_string()),
            default_reviewers: Vec::new(),
        };

        let client = BitbucketClient::new(&config).unwrap();

        assert_eq!(
            client.api_url("pull-requests"),
            "https://bitbucket.example.com/rest/api/1.0/projects/TEST/repos/my-repo/pull-requests"
        );
    }
}
