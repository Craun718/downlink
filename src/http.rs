use async_trait::async_trait;
use serde_json::Value;
use std::time::Duration;

use crate::channel::{NotifyError, ResponseBody};

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
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl HttpResponse {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    pub fn retry_after(&self) -> Option<Duration> {
        self.header("retry-after").and_then(parse_retry_after)
    }

    /// Convert a non-success response into the shared HTTP error type.
    ///
    /// Channel adapters may inspect known provider error payloads first, then use
    /// this method as the lossless fallback when the payload is unknown.
    pub fn into_http_error(self, channel: &str) -> NotifyError {
        NotifyError::HttpStatus {
            channel: channel.to_owned(),
            status: self.status,
            body: ResponseBody::new(&self.body),
            retry_after: self.retry_after(),
        }
    }

    pub fn ensure_success(self, channel: &str) -> Result<Self, NotifyError> {
        if self.is_success() {
            Ok(self)
        } else {
            Err(self.into_http_error(channel))
        }
    }

    pub fn json(&self) -> Result<Value, NotifyError> {
        self.json_for("unknown")
    }

    pub fn json_for(&self, channel: &str) -> Result<Value, NotifyError> {
        serde_json::from_str(&self.body).map_err(|error| NotifyError::ResponseDecode {
            channel: channel.to_owned(),
            status: self.status,
            message: error.to_string(),
            body: ResponseBody::new(&self.body),
        })
    }
}

fn parse_retry_after(value: &str) -> Option<Duration> {
    value.trim().parse::<u64>().ok().map(Duration::from_secs)
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
            Self {
                inner: reqwest::Client::new(),
            }
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
            let headers = response
                .headers()
                .iter()
                .filter_map(|(key, value)| {
                    value
                        .to_str()
                        .ok()
                        .map(|value| (key.to_string(), value.to_owned()))
                })
                .collect();
            let body = response
                .text()
                .await
                .map_err(|e| NotifyError::Network(e.to_string()))?;
            Ok(HttpResponse {
                status,
                headers,
                body,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel::{ErrorKind, NotifyError};

    fn response(status: u16, headers: Vec<(&str, &str)>, body: &str) -> HttpResponse {
        HttpResponse {
            status,
            headers: headers
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .collect(),
            body: body.to_owned(),
        }
    }

    #[test]
    fn header_lookup_is_case_insensitive() {
        let response = response(200, vec![("Retry-After", "7")], "{}");
        assert_eq!(response.header("retry-after"), Some("7"));
    }

    #[test]
    fn non_success_response_becomes_structured_http_error() {
        let error = response(429, vec![("Retry-After", "7")], "rate limited")
            .ensure_success("telegram")
            .expect_err("429 must not be treated as success");

        assert_eq!(error.kind(), ErrorKind::RateLimited);
        assert_eq!(error.channel(), Some("telegram"));
        assert_eq!(error.http_status(), Some(429));
        assert_eq!(error.retry_after(), Some(Duration::from_secs(7)));
        assert!(error.is_retryable());
        assert!(matches!(error, NotifyError::HttpStatus { .. }));
    }

    #[test]
    fn response_decode_error_keeps_status_and_bounded_body() {
        let body = "x".repeat(ResponseBody::MAX_PREVIEW_BYTES + 1);
        let error = response(200, vec![], &body)
            .json_for("discord")
            .expect_err("invalid JSON must be reported");

        assert_eq!(error.kind(), ErrorKind::ResponseDecode);
        assert_eq!(error.http_status(), Some(200));
        match error {
            NotifyError::ResponseDecode { body, .. } => {
                assert_eq!(body.preview.len(), ResponseBody::MAX_PREVIEW_BYTES);
                assert!(body.truncated);
            }
            other => panic!("expected response decode error, got {other:?}"),
        }
    }
}
