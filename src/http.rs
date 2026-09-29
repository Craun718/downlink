use async_trait::async_trait;
use serde_json::Value;

use crate::channel::NotifyError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
}

impl HttpMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

impl HttpResponse {
    pub fn json(&self) -> Result<Value, NotifyError> {
        serde_json::from_str(&self.body)
            .map_err(|e| NotifyError::Channel(format!("invalid JSON response: {e}")))
    }
}

#[async_trait]
pub trait HttpClient: Send + Sync {
    async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, NotifyError>;
}

#[cfg(feature = "default-client")]
pub mod default {
    use super::*;

    #[derive(Debug, Clone)]
    pub struct ReqwestClient {
        inner: reqwest::Client,
    }

    impl ReqwestClient {
        pub fn new() -> Self {
            Self { inner: reqwest::Client::new() }
        }
    }

    impl Default for ReqwestClient {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl HttpClient for ReqwestClient {
        async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, NotifyError> {
            let mut builder = match request.method {
                HttpMethod::Get => self.inner.get(&request.url),
                HttpMethod::Post => self.inner.post(&request.url),
            };
            for (key, value) in &request.headers {
                builder = builder.header(key.as_str(), value.as_str());
            }
            if let Some(body) = &request.body {
                builder = builder.json(body);
            }
            let response = builder
                .send()
                .await
                .map_err(|e| NotifyError::Network(e.to_string()))?;
            let status = response.status().as_u16();
            let body = response
                .text()
                .await
                .map_err(|e| NotifyError::Network(e.to_string()))?;
            Ok(HttpResponse { status, body })
        }
    }
}
