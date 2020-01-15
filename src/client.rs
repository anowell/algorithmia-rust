//! Internal client
//!
//! Do not use directly - use the [`Algorithmia`](../struct.Algorithmia.html) struct instead
use std::str::FromStr;
use std::sync::Arc;

use headers::{authorization::Credentials, Authorization, HeaderMapExt, UserAgent};
use http::header::HeaderMap;
use http::header::HeaderValue;
use reqwest::blocking::{Client, RequestBuilder};
pub use reqwest::Body;
use reqwest::{IntoUrl, Method, Url};

use crate::error::{Error, ResultExt};

struct Simple(HeaderValue);
impl Credentials for Simple {
    const SCHEME: &'static str = "Simple";
    fn decode(value: &HeaderValue) -> Option<Self> {
        debug_assert!(
            value.as_bytes().starts_with(b"Simple "),
            "HeaderValue to decode should start with \"Simple ..\", received = {:?}",
            value,
        );

        Some(Simple(value.clone()))
    }
    fn encode(&self) -> HeaderValue {
        self.0.clone()
    }
}

impl Simple {
    /// Try to create a `Simple` authorization header.
    pub fn new(token: &str) -> Result<Self, Error> {
        HeaderValue::from_str(&format!("Simple {}", token))
            .map(Simple)
            .context("API key is invalid")
    }
}
/// Represent the different ways to auth with the API
#[derive(Clone)]
pub enum ApiAuth {
    /// Algorithmia API key to use for authentication
    ApiKey(String),
    /// Use unauthenticated request (common for on-platform algorithms)
    None,
}

/// Internal `HttpClient` to build requests: wraps `reqwest` client
#[derive(Clone)]
pub struct HttpClient {
    pub base_url: Url,
    api_auth: ApiAuth,
    inner_client: Arc<Client>,
    user_agent: String,
}

impl HttpClient {
    /// Instantiate an `HttpClient` - creates a new `reqwest` client
    pub fn new<U: IntoUrl>(api_auth: ApiAuth, base_url: U) -> Result<HttpClient, Error> {
        Ok(HttpClient {
            api_auth: api_auth,
            base_url: base_url.into_url().context("Invalid base URL")?,
            inner_client: Self::inner_client(),
            user_agent: format!(
                "algorithmia-rust/{} (Rust {}",
                option_env!("CARGO_PKG_VERSION").unwrap_or("unknown"),
                crate::version::RUSTC_VERSION
            ),
        })
    }
    /// Helper to make Algorithmia GET requests with the API key
    pub fn get(&self, url: Url) -> RequestBuilder {
        self.build_request(Method::GET, url)
    }

    /// Helper to make Algorithmia GET requests with the API key
    pub fn head(&self, url: Url) -> RequestBuilder {
        self.build_request(Method::HEAD, url)
    }

    /// Helper to make Algorithmia POST requests with the API key
    pub fn post(&self, url: Url) -> RequestBuilder {
        self.build_request(Method::POST, url)
    }

    /// Helper to make Algorithmia PUT requests with the API key
    pub fn put(&self, url: Url) -> RequestBuilder {
        self.build_request(Method::PUT, url)
    }

    /// Helper to make Algorithmia POST requests with the API key
    pub fn delete(&self, url: Url) -> RequestBuilder {
        self.build_request(Method::DELETE, url)
    }

    fn build_request(&self, verb: Method, url: Url) -> RequestBuilder {
        let mut headers = HeaderMap::new();
        headers.typed_insert(
            UserAgent::from_str(&self.user_agent).expect("User Agent not valid ASCII"),
        );
        if let ApiAuth::ApiKey(ref api_key) = self.api_auth {
            headers.typed_insert(Authorization(
                Simple::new(api_key).expect("API Key not valid ASCII"),
            ));
        }

        self.inner_client
            .request(verb, url.clone())
            .headers(headers)
    }

    #[cfg(not(feature = "rust-tls"))]
    fn inner_client() -> Arc<Client> {
        Arc::new(Client::new())
    }

    #[cfg(feature = "rust-tls")]
    fn inner_client() -> Arc<Client> {
        Arc::new(Client::builder().use_rustls_tls().build().unwrap())
    }
}

impl<'a> From<&'a str> for ApiAuth {
    fn from(api_key: &'a str) -> Self {
        match api_key.len() {
            0 => ApiAuth::None,
            _ => ApiAuth::ApiKey(api_key.into()),
        }
    }
}

impl From<String> for ApiAuth {
    fn from(api_key: String) -> Self {
        match api_key.len() {
            0 => ApiAuth::None,
            _ => ApiAuth::ApiKey(api_key),
        }
    }
}

impl From<()> for ApiAuth {
    fn from(_: ()) -> Self {
        ApiAuth::None
    }
}

pub(crate) mod header {
    use http::header::HeaderValue;

    pub const X_DATA_TYPE: &'static str = "x-data-type";
    pub const X_ERROR_MESSAGE: &'static str = "x-error-message";
    pub(crate) fn lossy_header(val: &HeaderValue) -> String {
        String::from_utf8_lossy(val.as_bytes()).to_string()
    }
}
