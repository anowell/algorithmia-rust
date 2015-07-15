//! Algorithmia client library
//!
//! # Examples
//!
//! ```no_run
//! use algorithmia::Algorithmia;
//! use algorithmia::algo::{Algorithm, AlgoOutput, Version};
//!
//! // Initialize with an API key
//! let client = Algorithmia::client("111112222233333444445555566");
//! let moving_avg = client.algo("timeseries", "SimpleMovingAverage", Version::Minor(0,1));
//!
//! // Run the algorithm using a type safe decoding of the output to Vec<int>
//! //   since this algorithm outputs results as a JSON array of integers
//! let input = (vec![0,1,2,3,15,4,5,6,7], 3);
//! let output: AlgoOutput<Vec<f64>> = moving_avg.pipe(&input).unwrap();
//! println!("Completed in {} seconds with result: {:?}", output.metadata.duration, output.result);
//! ```

#![doc(html_logo_url = "https://algorithmia.com/assets/images/apple-touch-icon.png")]

extern crate hyper;
extern crate rustc_serialize;

pub mod algo;
pub mod data;

use algo::{Algorithm, Version};
use data::{DataDir, DataFile};

use hyper::{Client, Url};
use hyper::client::RequestBuilder;
use hyper::header::{Authorization, UserAgent};
use hyper::method::Method;
use rustc_serialize::{json, Decodable};
use self::AlgorithmiaError::*;
use std::{io, env};
use std::rc::Rc;

static DEFAULT_API_BASE_URL: &'static str = "https://api.algorithmia.com";

/// The top-level struct for instantiating Algorithmia client endpoints
pub struct Algorithmia {
    pub api_key: String,
    pub base_url: String,
    client: Rc<Client>,
}

/// Internal HttpClient to build requests: wraps `hyper` client
struct HttpClient<'a>{
    api_key: String,
    client: &'a Client,
    user_agent: String,
}

/// Errors that may be returned by this library
#[derive(Debug)]
pub enum AlgorithmiaError {
    /// Errors returned by the Algorithmia API, Optional Stacktrace
    AlgorithmiaApiError(ApiError),
    /// HTTP errors encountered by the hyper client
    HttpError(hyper::error::Error),
    /// Errors decoding response json
    DecoderError(json::DecoderError),
    /// Errors decoding response json with additional debugging context
    DecoderErrorWithContext(json::DecoderError, String),
    /// Errors encoding the request
    EncoderError(json::EncoderError),
    /// General IO errors
    IoError(io::Error),
}

#[derive(RustcDecodable, Debug)]
pub struct ApiError {
    pub message: String,
    pub stacktrace: Option<String>,
}

/// Struct for decoding Algorithmia API error responses
#[derive(RustcDecodable, Debug)]
pub struct ApiErrorResponse {
    pub error: ApiError,
}

impl<'a, 'c> Algorithmia {
    /// Instantiate a new client
    pub fn client(api_key: &str) -> Algorithmia {
        Algorithmia {
            api_key: api_key.to_string(),
            base_url: Self::get_base_url(),
            client: Rc::new(Client::new()),
        }
    }

    fn get_base_url() -> String {
        // TODO: memoize
        match env::var("ALGORITHMIA_API") {
            Ok(url) => url,
            Err(_) => DEFAULT_API_BASE_URL.to_string(),
        }
    }

    /// Instantiate a new hyper client - used internally by instantiating new api_client for every request
    fn http_client(&self) -> HttpClient {
        HttpClient::new(self.api_key.clone(), &self.client)
    }

    /// Instantiate an `Algorithm` from this client
    ///
    /// # Examples
    ///
    /// ```
    /// use algorithmia::Algorithmia;
    /// use algorithmia::algo::Version;
    /// let client = Algorithmia::client("111112222233333444445555566");
    /// let factor = client.algo("anowell", "Dijkstra", Version::Latest);
    /// ```
    pub fn algo(self, user: &'a str, repo: &'a str, version: Version<'a>) -> Algorithm<'a> {
        Algorithm {
            client: self,
            user: user,
            repo: repo,
            version: version
        }
    }

    /// Instantiate an `Algorithm` from this client using the algorithm's URI
    ///
    /// # Examples
    /// ```
    /// use algorithmia::Algorithmia;
    /// use algorithmia::algo::Version;
    /// let client = Algorithmia::client("111112222233333444445555566");
    /// let factor = client.algo_from_str("anowell/Dijkstra/0.1");
    /// ```
    pub fn algo_from_str(self, algo_uri: &'a str) -> Result<Algorithm<'a>, &'a str> {
        // TODO: test that this works for stripping algo:// prefix
        // let stripped = match algo_uri.rsplitn(2, "//").nth(0) {
        //     Some(path) => path,
        //     None => return Err("Invalid algorithm URI"),
        // };

        let parts: Vec<_> = algo_uri.split("/").collect();
        match parts.len() {
            3 => Ok(
                Algorithm {
                    client: self,
                    user: parts[0],
                    repo: parts[1],
                    version: Version::from_str(parts[2])
                }
            ),
            2 => Ok(
                Algorithm {
                    client: self,
                    user: parts[0],
                    repo: parts[1],
                    version: Version::Latest
                }
            ),
            _ => Err("Invalid algorithm URI")
        }
    }

    /// Instantiate a `DataDirectory` from this client
    ///
    /// # Examples
    ///
    /// ```
    /// use algorithmia::Algorithmia;
    /// let client = Algorithmia::client("111112222233333444445555566");
    /// let rustfoo = client.dir("data://.my/rustfoo");
    /// ```
    pub fn dir(self, path: &'a str) -> DataDir {
        DataDir::new(self, path)
    }

    /// Instantiate a `DataDirectory` from this client
    ///
    /// # Examples
    ///
    /// ```
    /// use algorithmia::Algorithmia;
    /// let client = Algorithmia::client("111112222233333444445555566");
    /// let rustfoo = client.file("data://.my/rustfoo");
    /// ```
    pub fn file(self, path: &'a str) -> DataFile {
        DataFile::new(self, path)
    }

    /// Helper to standardize decoding to a specific Algorithmia Result type
    pub fn decode_to_result<T: Decodable>(res_json: String) -> Result<T, AlgorithmiaError> {
        match json::decode::<T>(&res_json) {
            Ok(result) => Ok(result),
            Err(why) => match json::decode::<ApiErrorResponse>(&res_json) {
                Ok(err_res) => Err(AlgorithmiaError::AlgorithmiaApiError(err_res.error)),
                Err(_) => Err(AlgorithmiaError::DecoderErrorWithContext(why, res_json)),
            }
        }
    }

    pub fn decode_to_error(res_json: String) -> AlgorithmiaError {
        match json::decode::<ApiErrorResponse>(&res_json) {
            Ok(err_res) => AlgorithmiaError::AlgorithmiaApiError(err_res.error),
            Err(why) => AlgorithmiaError::DecoderErrorWithContext(why, res_json),
        }
    }

}

impl <'a> HttpClient<'a> {
    /// Instantiate an HttpClient - creates a new `hyper` client
    fn new(api_key: String, client: &'a Client) -> HttpClient {
        HttpClient {
            api_key: api_key,
            client: client,
            user_agent: format!("rust/{} algorithmia.rs/{}", option_env!("CFG_RELEASE").unwrap_or("unknown"), option_env!("CARGO_PKG_VERSION").unwrap_or("unknown")),
        }
    }

    /// Helper to make Algorithmia GET requests with the API key
    fn get(&self, url: Url) -> RequestBuilder<Url> {
        self.build_request(Method::Get, url)
    }

    /// Helper to make Algorithmia POST requests with the API key
    fn post(&self, url: Url) -> RequestBuilder<Url> {
        self.build_request(Method::Post, url)
    }

    /// Helper to make Algorithmia PUT requests with the API key
    fn put(&self, url: Url) -> RequestBuilder<Url> {
        self.build_request(Method::Put, url)
    }

    /// Helper to make Algorithmia POST requests with the API key
    fn delete(&self, url: Url) -> RequestBuilder<Url> {
        self.build_request(Method::Delete, url)
    }


    fn build_request(&self, verb: Method, url: Url) -> RequestBuilder<Url> {
        let req = self.client.request(verb, url);

        // TODO: Support Secure Auth
        req.header(UserAgent(self.user_agent.clone()))
           .header(Authorization(format!("Simple {}", self.api_key)))
    }
}


/*
* Trait implementations
*/
/// Allow cloning a client in order to reuse the API key for multiple connections
impl std::clone::Clone for Algorithmia {
    fn clone(&self) -> Algorithmia {
        Algorithmia {
            api_key: self.api_key.clone(),
            base_url: self.base_url.clone(),
            client: self.client.clone(),
        }
    }
}

impl From<io::Error> for AlgorithmiaError {
    fn from(err: io::Error) -> AlgorithmiaError {
        IoError(err)
    }
}

impl From<hyper::error::Error> for AlgorithmiaError {
    fn from(err: hyper::error::Error) -> AlgorithmiaError {
        HttpError(err)
    }
}

impl From<json::DecoderError> for AlgorithmiaError {
    fn from(err: json::DecoderError) -> AlgorithmiaError {
        DecoderError(err)
    }
}

impl From<json::EncoderError> for AlgorithmiaError {
    fn from(err: json::EncoderError) -> AlgorithmiaError {
        EncoderError(err)
    }
}