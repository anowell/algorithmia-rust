//! Algorithmia client library
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
//! // Run the algorithm using a type safe decoding of the output to Vec<f64>
//! //   since this algorithm outputs results as a JSON array of numbers
//! let input = (vec![0,1,2,3,15,4,5,6,7], 3);
//! let result: Vec<f64> = moving_avg.pipe(&input)?.decode()?;
//! println!("Completed with result: {:?}", result);
//! # Ok::<(), Box<std::error::Error>>(())
//! ```

#![doc(html_logo_url = "https://algorithmia.com/assets/images/logos/png/bintreePurple.png")]
#![doc(test(attr(allow(unused_variables), allow(dead_code))))]
#![allow(unknown_lints)]
#![recursion_limit = "1024"]

use crate::algo::{AlgoUri, Algorithm};
use crate::client::HttpClient;
use crate::data::{DataDir, DataFile, DataObject, HasDataPath};

#[macro_use]
pub mod error;
pub mod algo;
pub mod data;

#[cfg(feature = "handler")]
pub mod handler;

use crate::client::ApiAuth;
use crate::error::Error;
pub use reqwest::blocking::Body;
pub use reqwest::{IntoUrl, Url};

/// Reexports of the most common types and traits
pub mod prelude {
    pub use crate::algo::AlgoIo;
    pub use crate::data::HasDataPath;
    pub use crate::Algorithmia;

    #[cfg(feature = "handler")]
    pub use crate::handler;
}

mod client;
mod version;

const DEFAULT_API_BASE_URL: &'static str = "https://api.algorithmia.com";

/// The top-level struct for instantiating Algorithmia client endpoints
pub struct Algorithmia {
    http_client: HttpClient,
}

impl Algorithmia {
    /// Instantiate a new client
    ///
    /// The Algorithmia client uses environment variables
    ///   `ALGORITHMIA_API` to override the default base URL of the API
    ///   and `ALGORITHMIA_API_KEY` to optionally the API key.
    pub fn new() -> Result<Algorithmia, Error> {
        let api_address =
            std::env::var("ALGORITHMIA_API").unwrap_or_else(|_| DEFAULT_API_BASE_URL.into());
        let auth = std::env::var("ALGORITHMIA_API_KEY")
            .map(ApiAuth::from)
            .unwrap_or(ApiAuth::None);
        Ok(Algorithmia {
            http_client: HttpClient::new(auth, &api_address)?,
        })
    }

    /// Instantiate a new client
    ///
    /// Client should be instatiated with your API key, except
    ///   when running within an algorithm on the Algorithmia platform.
    ///
    /// # Examples
    /// ```
    /// use algorithmia::*;
    /// // Initialize a client
    /// let client = Algorithmia::client("simUseYourApiKey");
    /// ```
    pub fn client<A: Into<String>>(api_key: A) -> Result<Algorithmia, Error> {
        let api_address =
            std::env::var("ALGORITHMIA_API").unwrap_or_else(|_| DEFAULT_API_BASE_URL.into());
        Ok(Algorithmia {
            http_client: HttpClient::new(ApiAuth::from(api_key.into()), &api_address)?,
        })
    }

    /// Instantiate a new client against alternate API servers
    pub fn client_with_url<A: Into<String>, U: IntoUrl>(
        api_key: A,
        base_url: U,
    ) -> Result<Algorithmia, Error> {
        Ok(Algorithmia {
            http_client: HttpClient::new(ApiAuth::from(api_key.into()), base_url)?,
        })
    }

    /// Instantiate an [`Algorithm`](algo/algorithm.struct.html) from this client
    ///
    /// By using In
    /// # Examples
    ///
    /// ```
    /// use algorithmia::Algorithmia;
    /// let client = Algorithmia::client("111112222233333444445555566")?;
    /// let factor = client.algo("anowell/Dijkstra/0.1");
    /// # Ok::<(), Box<std::error::Error>>(())
    /// ```
    pub fn algo<A: Into<AlgoUri>>(&self, algorithm: A) -> Algorithm {
        Algorithm::new(self.http_client.clone(), algorithm.into())
    }

    /// Instantiate a `DataDirectory` from this client
    ///
    /// # Examples
    ///
    /// ```
    /// use algorithmia::Algorithmia;
    /// let client = Algorithmia::client("111112222233333444445555566")?;
    /// let rustfoo = client.dir("data://.my/rustfoo");
    /// # Ok::<(), Box<std::error::Error>>(())
    /// ```
    pub fn dir(&self, path: &str) -> DataDir {
        DataDir::new(self.http_client.clone(), path)
    }

    /// Instantiate a `DataDirectory` from this client
    ///
    /// # Examples
    ///
    /// ```
    /// use algorithmia::Algorithmia;
    /// let client = Algorithmia::client("111112222233333444445555566")?;
    /// let rustfoo = client.file("data://.my/rustfoo");
    /// # Ok::<(), Box<std::error::Error>>(())
    /// ```
    pub fn file(&self, path: &str) -> DataFile {
        DataFile::new(self.http_client.clone(), path)
    }

    /// Instantiate a `DataPath` from this client
    ///
    /// Use this if you don't explicitly know if a Data URI is to a directory or file
    ///
    /// # Examples
    ///
    /// ```
    /// use algorithmia::Algorithmia;
    /// let client = Algorithmia::client("111112222233333444445555566")?;
    /// let rustfoo = client.data("data://.my/rustfoo/what_am_i");
    /// # Ok::<(), Box<std::error::Error>>(())
    /// ```
    pub fn data(&self, path: &str) -> DataObject {
        DataObject::new(self.http_client.clone(), path)
    }
}

/// Allow cloning in order to reuse http client (and API key) for multiple connections
impl Clone for Algorithmia {
    fn clone(&self) -> Algorithmia {
        Algorithmia {
            http_client: self.http_client.clone(),
        }
    }
}
