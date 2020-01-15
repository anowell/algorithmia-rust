//! Error types
use crate::client::header::{lossy_header, X_ERROR_MESSAGE};
use backtrace::Backtrace;
use reqwest::blocking::Response;
use serde::{Deserialize, Serialize};
use serde_json;
use std::error::Error as StdError;
use std::fmt::Display;
use std::{fmt, str};

/// Default error type for errors originating in algorithm code
const ALGORITHM_ERROR: &'static str = "AlgorithmError";

macro_rules! bail {
    ($e:expr) => {
        return Err($crate::error::err_msg($e));
    };
    ($fmt:expr, $($arg:tt)+) => {
        return Err($crate::error::err_msg(format!($fmt, $($arg)+)));
    };
}

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    ctx: String,
}

#[derive(Debug)]
pub(crate) enum ErrorKind {
    // Error from the Algorithmia API (may be from the algorithm)
    Api(ApiError),

    // Http errors calling the API (optionally with message from server)
    Http(reqwest::Error, Option<ApiError>),

    // Error context generated in this client
    Client,

    // Error context generated in this client
    Inner(Box<dyn StdError + Send + Sync + 'static>),
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ErrorKind::Http(err, _) => match err.status() {
                Some(status) => write!(f, "{}: {}", status, self.ctx),
                None => write!(f, "{}", self.ctx),
            },
            _ => write!(f, "{}", self.ctx),
        }
    }
}

impl Error {
    /// If the Algorithmia API returned an error, return the error response
    pub fn api_error(&self) -> Option<&ApiError> {
        match &self.kind {
            ErrorKind::Api(e) => Some(e),
            ErrorKind::Http(_, api_err) => api_err.as_ref(),
            _ => None,
        }
    }

    /// If an HTTP error occurred, return the relevant status code
    pub fn status(&self) -> Option<http::status::StatusCode> {
        match &self.kind {
            ErrorKind::Http(e, _) => e.status(),
            _ => None,
        }
    }
}

pub(crate) trait ResultExt<T> {
    fn context<D>(self, context: D) -> Result<T, Error>
    where
        D: Display + Send + Sync + 'static;

    fn with_context<F, D>(self, f: F) -> Result<T, Error>
    where
        D: Display + Send + Sync + 'static,
        F: FnOnce() -> D;
}

pub(crate) trait IntoErrorKind {
    fn into_error_kind(self) -> ErrorKind;
}

impl IntoErrorKind for Error {
    fn into_error_kind(self) -> ErrorKind {
        self.kind
    }
}

impl IntoErrorKind for reqwest::Error {
    fn into_error_kind(self) -> ErrorKind {
        ErrorKind::Http(self, None)
    }
}

macro_rules! impl_into_error_kind {
    ($p:ty) => {
        impl IntoErrorKind for $p {
            fn into_error_kind(self) -> ErrorKind {
                ErrorKind::Inner(Box::new(self))
            }
        }
    };
}

impl_into_error_kind!(std::io::Error);
impl_into_error_kind!(serde_json::error::Error);
impl_into_error_kind!(reqwest::header::InvalidHeaderValue);
impl_into_error_kind!(url::ParseError);
impl_into_error_kind!(base64::DecodeError);

impl<T, E> ResultExt<T> for Result<T, E>
where
    E: IntoErrorKind,
{
    fn context<D>(self, context: D) -> Result<T, Error>
    where
        D: Display + Send + Sync + 'static,
    {
        self.with_context(|| context)
    }

    fn with_context<F, D>(self, f: F) -> Result<T, Error>
    where
        D: Display + Send + Sync + 'static,
        F: FnOnce() -> D,
    {
        self.map_err(|source| Error {
            kind: source.into_error_kind(),
            ctx: f().to_string(),
        })
    }
}

/// Error from the Algorithmia API (may be from the algorithm)
#[derive(Debug, Deserialize, Serialize)]
pub struct ApiError {
    /// Error message returned from the Algorithmia API
    pub message: String,
    /// Error type
    pub error_type: Option<String>,
    /// Stacktrace of algorithm exception/panic
    pub stacktrace: Option<String>,
}

impl Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(error_type) = &self.error_type {
            write!(f, "{}: ", error_type)?;
        }
        write!(f, "{}", self.message)?;
        if let Some(trace) = &self.stacktrace {
            write!(f, "\n{}", trace)?;
        }
        Ok(())
    }
}

impl From<ApiError> for Error {
    fn from(err: ApiError) -> Self {
        Error {
            kind: ErrorKind::Api(err),
            ctx: String::new(), // TODO: should we allow this
        }
    }
}

pub(crate) fn err_msg<D: Display>(msg: D) -> Error {
    Error::from(msg.to_string())
}

impl From<String> for Error {
    fn from(msg: String) -> Self {
        Error {
            kind: ErrorKind::Client,
            ctx: msg,
        }
    }
}

impl StdError for ApiError {}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match &self.kind {
            ErrorKind::Api(e) => Some(e as &(dyn StdError + 'static)),
            ErrorKind::Http(_, Some(e)) => Some(e as &(dyn StdError + 'static)),
            ErrorKind::Http(e, None) => Some(e as &(dyn StdError + 'static)),
            ErrorKind::Inner(e) => Some(e.as_ref() as &(dyn StdError + 'static)),
            ErrorKind::Client => None,
        }
    }
}

impl ApiError {
    /// Creates an ApiError - intended for creating ApiErrors from Rust algorithms
    ///
    /// ## Examples:
    ///
    /// ```
    /// use algorithmia::error::ApiError;
    ///
    /// ApiError::new("InputError", "Input missing field 'url'");
    /// ```
    pub fn new<S: Into<String>>(error_type: S, message: S) -> ApiError {
        ApiError {
            error_type: Some(error_type.into()),
            message: message.into(),
            stacktrace: Some(format!("{:?}", Backtrace::new())),
        }
    }
}

impl<S> From<S> for ApiError
where
    S: Into<String>,
{
    fn from(message: S) -> ApiError {
        ApiError {
            error_type: Some(ALGORITHM_ERROR.into()),
            message: message.into(),
            stacktrace: Some(format!("{:?}", Backtrace::new())),
        }
    }
}

/// Struct for decoding Algorithmia API error responses
#[derive(Debug, Deserialize)]
#[doc(hidden)]
pub struct ApiErrorResponse {
    pub error: ApiError,
}

/// Helper to decode API responses into errors
impl Error {
    pub fn from_json(json: &str) -> Error {
        let decoded_error = serde_json::from_str::<ApiErrorResponse>(json);
        match decoded_error.context("Failed to decode API error response") {
            Ok(err_res) => Error::from(err_res.error),
            Err(err) => err,
        }
    }
}

pub(crate) fn process_http_response(resp: Response) -> Result<Response, Error> {
    let status = resp.status();
    if status.is_success() {
        Ok(resp)
    } else {
        let err_status = resp.error_for_status_ref().unwrap_err();
        let x_error_msg = resp.headers().get(X_ERROR_MESSAGE).map(lossy_header);
        let api_err = match resp.json::<ApiErrorResponse>() {
            Ok(err_res) => Some(err_res.error),
            Err(_) => match x_error_msg {
                Some(message) => Some(ApiError {
                    message,
                    error_type: None,
                    stacktrace: None,
                }),
                None => None,
            },
        };

        Err(Error {
            kind: ErrorKind::Http(err_status, api_err),
            ctx: String::new(),
        })
    }
}
