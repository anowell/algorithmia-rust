//! Algorithmia client library
//!
//! # Examples
//!
//! ```no_run
//! use algorithmia::Algorithmia;
//! use algorithmia::algo::{Algorithm, Version};
//!
//! // Initialize with an API key
//! let client = Algorithmia::client("111112222233333444445555566");
//! let moving_avg = client.algo(("timeseries/SimpleMovingAverage", "0.1"));
//!
//! // Run the algorithm using a type safe decoding of the output to Vec<int>
//! //   since this algorithm outputs results as a JSON array of integers
//! let input = (vec![0,1,2,3,15,4,5,6,7], 3);
//! let result: Vec<f64> = moving_avg.pipe(&input, None).unwrap().decode().unwrap();
//! println!("Completed with result: {:?}", result);
//! ```

#![doc(html_logo_url = "https://algorithmia.com/assets/images/apple-touch-icon.png")]

#[macro_use]
extern crate hyper;
extern crate rustc_serialize;

use algo::{Algorithm, AlgoRef};
use data::{DataDir, DataFile, DataPath, HasDataPath};
use client::HttpClient;

use std::clone;

pub mod algo;
pub mod data;
pub mod error;
pub mod client;
pub use hyper::{mime, Url};
pub use client::ApiAuth::{self, SimpleAuth, NoAuth};

static DEFAULT_API_BASE_URL: &'static str = "https://api.algorithmia.com";

/// The top-level struct for instantiating Algorithmia client endpoints
pub struct Algorithmia {
    http_client: HttpClient,
}

impl<'a, 'c> Algorithmia {
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
    ///
    /// // Initialize a client (for algorithms running on the Algorithmia platform)
    /// let client = Algorithmia::client(NoAuth);
    /// ```
    pub fn client<A: Into<ApiAuth>>(api_key: A) -> Algorithmia {
        let api_address = std::env::var("ALGORITHMIA_API").unwrap_or(DEFAULT_API_BASE_URL.into());
        Algorithmia {
            http_client: HttpClient::new(api_key.into(), api_address),
        }
    }

    /// Instantiate a new client against alternate API servers
    pub fn alt_client<A: Into<ApiAuth>>(base_url: Url, api_key: A) -> Algorithmia {
        Algorithmia {
            http_client: HttpClient::new(api_key.into(), base_url.serialize()),
        }
    }

    /// Instantiate an [`Algorithm`](algo/algorithm.struct.html) from this client
    ///
    /// By using In
    /// # Examples
    ///
    /// ```
    /// use algorithmia::Algorithmia;
    /// use algorithmia::algo::Version;
    /// let client = Algorithmia::client("111112222233333444445555566");
    /// let factor = client.algo("anowell/Dijkstra/0.1");
    /// let factor = client.algo(("anowell/Dijkstra", Version::Latest));
    /// ```
    pub fn algo<A: Into<AlgoRef>>(&self, algorithm: A) -> Algorithm {
        Algorithm::new(self.http_client.clone(), algorithm.into())
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
    pub fn dir(&self, path: &'a str) -> DataDir {
        DataDir::new(self.http_client.clone(), path)
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
    pub fn file(&self, path: &'a str) -> DataFile {
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
    /// let client = Algorithmia::client("111112222233333444445555566");
    /// let rustfoo = client.data("data://.my/rustfoo/what_am_i");
    /// ```
    pub fn data(&self, path: &'a str) -> DataPath {
        DataPath::new(self.http_client.clone(), path)
    }

}


/// Allow cloning in order to reuse http client (and API key) for multiple connections
impl clone::Clone for Algorithmia {
    fn clone(&self) -> Algorithmia {
        Algorithmia {
            http_client: self.http_client.clone(),
        }
    }
}
