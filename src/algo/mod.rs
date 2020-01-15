//! API client for calling Algorithmia algorithms
//!
//! # Examples
//!
//! ```no_run
//! use algorithmia::Algorithmia;
//!
//! // Initialize with an API key
//! let client = Algorithmia::client("111112222233333444445555566")?;
//! let moving_avg = client.algo("timeseries/SimpleMovingAverage/0.1");
//!
//! // Run the algorithm using a type safe decoding of the output to Vec<int>
//! //   since this algorithm outputs results as a JSON array of integers
//! let input = (vec![0,1,2,3,15,4,5,6,7], 3);
//! let result: Vec<f64> = moving_avg.pipe(&input)?.decode()?;
//! println!("Completed with result: {:?}", result);
//! # Ok::<(), Box<std::error::Error>>(())
//! ```

use crate::client::HttpClient;
use crate::error::{ApiErrorResponse, Error, ResultExt};
use crate::Body;

mod bytevec;
pub use bytevec::ByteVec;

use serde::de::DeserializeOwned;
use serde::de::Error as SerdeError;
use serde::{Deserialize, Serialize};
use serde_json::{self, json, Value};

use base64;
use headers::ContentType;
use mime::{self, Mime};
#[doc(hidden)]
pub use reqwest::blocking::Response;
use reqwest::Url;

use headers::HeaderMapExt;
use http::header::HeaderMap;
use std::collections::HashMap;
use std::fmt;
use std::io::{self, Read, Write};
use std::ops::{Deref, DerefMut};
use std::str::FromStr;

static ALGORITHM_BASE_PATH: &'static str = "v1/algo";

/// Types that store either input or ouput to an algorithm
#[derive(Debug, Clone)]
pub struct AlgoIo {
    pub(crate) data: AlgoData,
}

#[derive(Debug, Clone)]
pub(crate) enum AlgoData {
    /// Text input or output
    Text(String),
    /// Binary input or output
    Binary(Vec<u8>),
    /// JSON input or output
    Json(Value),
}

/// Algorithmia algorithm - intialized from the `Algorithmia` builder
pub struct Algorithm {
    algo_uri: AlgoUri,
    options: AlgoOptions,
    client: HttpClient,
}

/// Options used to alter the algorithm call, e.g. configuring the timeout
pub struct AlgoOptions {
    opts: HashMap<String, String>,
}

/// URI of an Algorithmia algorithm
#[derive(Clone)]
pub struct AlgoUri {
    path: String,
}

/// Metadata returned from the API
#[derive(Debug, Deserialize)]
pub struct AlgoMetadata {
    /// Algorithm execution duration
    pub duration: f32,
    /// Stdout from the algorithm (must enable stdout on request and be the algorithm author)
    pub stdout: Option<String>,
    /// API alerts (e.g. low balance warning)
    pub alerts: Option<Vec<String>>,
    /// Describes how the ouput's `result` field should be parsed (`text`, `json`, or `binary`)
    pub content_type: String,
    // Placeholder for API stability if additional fields are added later
    #[serde(skip_deserializing)]
    _dummy: (),
}

/// Successful API response that wraps the `AlgoIo` and its Metadata
pub struct AlgoResponse {
    /// Any metadata associated with the API response
    pub metadata: AlgoMetadata,
    /// The algorithm output decoded into an `AlgoIo` enum
    pub result: AlgoIo,
    // Placeholder for API stability if additional fields are added later
    _dummy: (),
}

impl Algorithm {
    #[doc(hidden)]
    pub fn new(client: HttpClient, algo_uri: AlgoUri) -> Algorithm {
        Algorithm {
            client: client,
            algo_uri: algo_uri,
            options: AlgoOptions::default(),
        }
    }

    /// Get the API Endpoint URL for this Algorithm
    pub fn to_url(&self) -> Result<Url, Error> {
        let path = format!("{}/{}", ALGORITHM_BASE_PATH, self.algo_uri.path);
        self.client
            .base_url
            .join(&path)
            .with_context(|| format!("invalid algorithm URI {}", path))
    }

    /// Get the Algorithmia algo URI for this Algorithm
    pub fn to_algo_uri(&self) -> &AlgoUri {
        &self.algo_uri
    }

    /// Execute an algorithm with the specified `input_data`.
    ///
    /// `input_data` can be any type which converts into `AlgoIo`,
    ///   including strings, byte slices, and any serializable type.
    ///   To create serializable objects for complex input, annotate your type
    ///   with `#[derive(Serialize)]` (see [serde.rs](http://serde.rs) for details).
    ///   If you want to send a raw, unparsed JSON string, use the `pipe_json` method instead.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use algorithmia::Algorithmia;
    /// let client = Algorithmia::client("111112222233333444445555566").unwrap();
    /// let moving_avg = client.algo("timeseries/SimpleMovingAverage/0.1");
    /// let input = (vec![0,1,2,3,15,4,5,6,7], 3);
    /// let res: Vec<f32> = moving_avg.pipe(&input)?.decode()?;
    /// # Ok::<(), Box<std::error::Error>>(())
    /// ```
    pub fn pipe<I>(&self, input_data: I) -> Result<AlgoResponse, Error>
    where
        I: Into<AlgoIo>,
    {
        let mut res = match input_data.into().data {
            AlgoData::Text(text) => self.pipe_as(text, mime::TEXT_PLAIN)?,
            AlgoData::Json(json) => {
                let encoded = serde_json::to_vec(&json)
                    .context("failed to encode algorithm input as JSON")?;
                self.pipe_as(encoded, mime::APPLICATION_JSON)?
            }
            AlgoData::Binary(bytes) => self.pipe_as(bytes, mime::APPLICATION_OCTET_STREAM)?,
        };

        let mut res_json = String::new();
        res.read_to_string(&mut res_json)
            .context("failed to read algorithm response")?;
        res_json.parse()
    }

    /// Execute an algorithm with a raw JSON string as input.
    ///
    /// While the `pipe` method is more flexible in accepting different types
    ///   of input, and inferring the content type when making an API call,
    ///   `pipe_json` explicitly sends the provided string with
    ///   `Content-Type: application/json` making no attempt to verify that
    ///   the input is valid JSON. By contrast, calling `pipe` with a string
    ///   would send it with `Content-Type: text/plain`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use algorithmia::Algorithmia;
    /// let client = Algorithmia::client("111112222233333444445555566")?;
    /// let minmax  = client.algo("codeb34v3r/FindMinMax/0.1");
    ///
    /// let output: Vec<u8> = minmax.pipe_json("[2,3,4]")?.decode()?;
    /// # Ok::<(), Box<std::error::Error>>(())
    pub fn pipe_json(&self, json_input: &str) -> Result<AlgoResponse, Error> {
        let mut res = self.pipe_as(json_input.to_owned(), mime::APPLICATION_JSON)?;

        let mut res_json = String::new();
        res.read_to_string(&mut res_json)
            .context("failed to read algorithm response")?;
        res_json.parse()
    }

    #[doc(hidden)]
    pub fn pipe_as<B>(&self, input_data: B, content_type: Mime) -> Result<Response, Error>
    where
        B: Into<Body>,
    {
        // Append options to URL as query parameters
        let mut url = self.to_url()?;
        if !self.options.is_empty() {
            let mut query_params = url.query_pairs_mut();
            for (k, v) in self.options.iter() {
                query_params.append_pair(&*k, &*v);
            }
        }

        // We just need the path and query string
        let mut headers = HeaderMap::new();
        headers.typed_insert(ContentType::from(content_type));
        self.client
            .post(url)
            .headers(headers)
            .body(input_data)
            .send()
            .with_context(|| format!("calling algorithm '{}'", self.algo_uri))
    }

    /// Builder method to explicitly configure options
    pub fn set_options(&mut self, options: AlgoOptions) -> &mut Algorithm {
        self.options = options;
        self
    }

    /// Builder method to configure the timeout in seconds
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use algorithmia::Algorithmia;
    ///
    /// let client = Algorithmia::client("111112222233333444445555566")?;
    /// client.algo("codeb34v3r/FindMinMax/0.1")
    ///     .timeout(3)
    ///     .pipe(vec![2,3,4])?;
    /// # Ok::<(), Box<std::error::Error>>(())
    /// ```
    pub fn timeout(&mut self, timeout: u32) -> &mut Algorithm {
        self.options.timeout(timeout);
        self
    }

    /// Builder method to enabled or disable stdout in the response metadata
    ///
    /// This has no affect unless authenticated as the owner of the algorithm
    pub fn stdout(&mut self, stdout: bool) -> &mut Algorithm {
        self.options.stdout(stdout);
        self
    }
}

impl AlgoUri {
    /// Returns the algorithm's URI path
    pub fn path(&self) -> &str {
        &self.path
    }
}

impl AlgoIo {
    /// If the `AlgoIo` is text (or a valid JSON string), returns the associated text
    pub fn as_string(&self) -> Option<&str> {
        match &self.data {
            AlgoData::Text(text) => Some(text),
            AlgoData::Json(json) => json.as_str(),
            AlgoData::Binary(_) => None,
        }
    }

    /// If the `AlgoIo` is binary, returns the associated byte slice
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match &self.data {
            AlgoData::Text(_) | AlgoData::Json(_) => None,
            AlgoData::Binary(bytes) => Some(bytes),
        }
    }

    /// If the `AlgoIo` is Json (or JSON encodable text), returns the associated JSON string
    pub fn to_json(&self) -> Option<String> {
        match &self.data {
            AlgoData::Text(text) => Some(json!(text).to_string()),
            AlgoData::Json(json) => Some(json.to_string()),
            AlgoData::Binary(_) => None,
        }
    }

    /// If the `AlgoIo` is valid JSON, decode it to a particular type
    ///
    pub fn decode<D: DeserializeOwned>(self) -> Result<D, Error> {
        let res_json = match self.data {
            AlgoData::Text(text) => json!(text),
            AlgoData::Json(json) => json,
            AlgoData::Binary(_) => bail!("cannot decode binary data as JSON"),
        };

        serde_json::from_value(res_json).context("failed to decode algorithm I/O to specified type")
    }
}

impl Deref for AlgoResponse {
    type Target = AlgoIo;
    fn deref(&self) -> &AlgoIo {
        &self.result
    }
}

// We need our own TryFrom trait because we can't implement
// the conversions from AlgoIo to any generic DeserializeOwned type until specialization
#[doc(hidden)]
pub trait TryFrom<T>: Sized {
    type Error;
    fn try_from(val: AlgoIo) -> Result<Self, Self::Error>;
}

impl TryFrom<AlgoIo> for AlgoIo {
    type Error = std::convert::Infallible;
    fn try_from(val: AlgoIo) -> Result<Self, Self::Error> {
        Ok(val)
    }
}

impl<D: DeserializeOwned> TryFrom<AlgoIo> for D {
    type Error = Error;
    fn try_from(val: AlgoIo) -> Result<Self, Self::Error> {
        val.decode()
    }
}

impl TryFrom<AlgoIo> for ByteVec {
    type Error = Error;
    fn try_from(val: AlgoIo) -> Result<Self, Self::Error> {
        match val.data {
            AlgoData::Text(_) => bail!("Cannot convert text to byte vector"),
            AlgoData::Json(_) => bail!("Cannot convert JSON to byte vector"),
            AlgoData::Binary(bytes) => Ok(ByteVec::from(bytes)),
        }
    }
}

impl AlgoResponse {
    /// If the algorithm output is JSON, decode it into a particular type
    pub fn decode<D>(self) -> Result<D, Error>
    where
        for<'de> D: Deserialize<'de>,
    {
        self.result.decode()
    }
}

impl AlgoOptions {
    /// Configure timeout in seconds
    pub fn timeout(&mut self, timeout: u32) {
        self.opts.insert("timeout".into(), timeout.to_string());
    }

    /// Enable or disable stdout retrieval
    ///
    /// This has no affect unless authenticated as the owner of the algorithm
    pub fn stdout(&mut self, stdout: bool) {
        self.opts.insert("stdout".into(), stdout.to_string());
    }
}

impl Default for AlgoOptions {
    fn default() -> AlgoOptions {
        AlgoOptions {
            opts: HashMap::new(),
        }
    }
}

impl Deref for AlgoOptions {
    type Target = HashMap<String, String>;
    fn deref(&self) -> &HashMap<String, String> {
        &self.opts
    }
}

impl DerefMut for AlgoOptions {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.opts
    }
}

impl FromStr for AlgoResponse {
    type Err = Error;
    fn from_str(json_str: &str) -> ::std::result::Result<Self, Self::Err> {
        // Early return if the response decodes into ApiErrorResponse
        if let Ok(err_res) = serde_json::from_str::<ApiErrorResponse>(json_str) {
            return Err(err_res.error.into());
        }

        // Parse into Json object
        let mut data =
            Value::from_str(json_str).context("failed to decode JSON as algorithm response")?;
        let metadata_value = data
            .as_object_mut()
            .and_then(|o| o.remove("metadata"))
            .ok_or_else(|| serde_json::Error::missing_field("metadata"))
            .context("failed to decode JSON as algorithm response")?;
        let result_value = data
            .as_object_mut()
            .and_then(|o| o.remove("result"))
            .ok_or_else(|| serde_json::Error::missing_field("result"))
            .context("failed to decode JSON as algorithm response")?;

        // Construct the AlgoIo object
        let metadata = serde_json::from_value::<AlgoMetadata>(metadata_value)
            .context("failed to decode JSON as algorithm response metadata")?;
        let data = match (&*metadata.content_type, result_value) {
            ("void", _) => AlgoData::Json(Value::Null),
            ("json", value) => AlgoData::Json(value),
            ("text", value) => match value.as_str() {
                Some(text) => AlgoData::Text(text.into()),
                None => bail!("content did not match content type 'text'"),
            },
            ("binary", value) => match value.as_str() {
                Some(text) => {
                    let binary = base64::decode(text)
                        .context("failed to decode base64 as algorithm response")?;
                    AlgoData::Binary(binary)
                }
                None => bail!("content did not match content type 'binary'"),
            },
            (content_type, _) => bail!("content did not match content type '{}'", content_type),
        };

        // Construct the AlgoResponse object
        Ok(AlgoResponse {
            metadata: metadata,
            result: AlgoIo { data },
            _dummy: (),
        })
    }
}

impl fmt::Display for AlgoUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.path)
    }
}

impl fmt::Display for AlgoResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.result.data {
            AlgoData::Text(s) => f.write_str(s),
            AlgoData::Json(s) => f.write_str(&s.to_string()),
            AlgoData::Binary(bytes) => f.write_str(&String::from_utf8_lossy(bytes)),
        }
    }
}

impl Read for AlgoResponse {
    fn read(&mut self, mut buf: &mut [u8]) -> io::Result<usize> {
        match &self.result.data {
            AlgoData::Text(s) => buf.write(s.as_bytes()),
            AlgoData::Json(s) => buf.write(s.to_string().as_bytes()),
            AlgoData::Binary(bytes) => buf.write(bytes),
        }
    }
}

impl<'a> From<&'a str> for AlgoUri {
    fn from(path: &'a str) -> Self {
        let path = match path {
            p if p.starts_with("algo://") => &p[7..],
            p if p.starts_with('/') => &p[1..],
            p => p,
        };
        AlgoUri {
            path: path.to_owned(),
        }
    }
}

impl From<String> for AlgoUri {
    fn from(path: String) -> Self {
        let path = match path {
            ref p if p.starts_with("algo://") => p[7..].to_owned(),
            ref p if p.starts_with('/') => p[1..].to_owned(),
            p => p,
        };
        AlgoUri { path: path }
    }
}

// AlgoIo Conversions
impl<S: Serialize> From<S> for AlgoIo {
    fn from(object: S) -> Self {
        let data = AlgoData::Json(serde_json::to_value(object).expect("Failed to serialize"));
        AlgoIo { data }
    }
}

impl From<ByteVec> for AlgoIo {
    fn from(bytes: ByteVec) -> Self {
        let data = AlgoData::Binary(bytes.into());
        AlgoIo { data }
    }
}

impl From<AlgoResponse> for AlgoIo {
    fn from(resp: AlgoResponse) -> Self {
        resp.result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Algorithmia;

    fn mock_client() -> Algorithmia {
        Algorithmia::client("").unwrap()
    }

    #[test]
    fn test_algo_without_version_to_url() {
        let mock_client = mock_client();
        let algorithm = mock_client.algo("/anowell/Pinky");
        assert_eq!(algorithm.to_url().unwrap().path(), "/v1/algo/anowell/Pinky");
    }

    #[test]
    fn test_algo_without_prefix_to_url() {
        let mock_client = mock_client();
        let algorithm = mock_client.algo("anowell/Pinky/0.1.0");
        assert_eq!(
            algorithm.to_url().unwrap().path(),
            "/v1/algo/anowell/Pinky/0.1.0"
        );
    }

    #[test]
    fn test_algo_with_prefix_to_url() {
        let mock_client = mock_client();
        let algorithm = mock_client.algo("algo://anowell/Pinky/0.1");
        assert_eq!(
            algorithm.to_url().unwrap().path(),
            "/v1/algo/anowell/Pinky/0.1"
        );
    }

    #[test]
    fn test_algo_with_sha_to_url() {
        let mock_client = mock_client();
        let algorithm = mock_client.algo("anowell/Pinky/abcdef123456");
        assert_eq!(
            algorithm.to_url().unwrap().path(),
            "/v1/algo/anowell/Pinky/abcdef123456"
        );
    }

    #[test]
    fn test_json_decoding() {
        let json_output =
            r#"{"metadata":{"duration":0.46739511,"content_type":"json"},"result":[5,41]}"#;
        let expected_result = [5, 41];
        let decoded = json_output.parse::<AlgoResponse>().unwrap();
        assert_eq!(0.46739511f32, decoded.metadata.duration);
        assert_eq!(expected_result, &*decoded.decode::<Vec<i32>>().unwrap());
    }
}
