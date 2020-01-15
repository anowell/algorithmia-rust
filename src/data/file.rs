//! File module for managing Algorithmia Data Files
//!
//! # Examples
//!
//! ```no_run
//! use algorithmia::Algorithmia;
//! let client = Algorithmia::client("111112222233333444445555566")?;
//! let my_file = client.file(".my/my_dir/some_filename");
//!
//! my_file.put("file_contents")?;
//! # Ok::<(), Box<std::error::Error>>(())
//! ```

use super::{parse_data_uri, parse_headers};
use crate::client::HttpClient;
use crate::data::{DataType, HasDataPath};
use crate::error::{process_http_response, Error, ResultExt};
use crate::Body;
use chrono::{DateTime, TimeZone, Utc};
use std::io::{self, Read};

/// Response and reader when downloading a `DataFile`
pub struct FileData {
    /// Size of file in bytes
    pub size: u64,
    /// Last modified timestamp
    pub last_modified: DateTime<Utc>,
    data: Box<dyn Read>,
}

impl Read for FileData {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.data.read(buf)
    }
}

impl FileData {
    /// Reads the result into a byte vector
    ///
    /// This is a convenience wrapper around `Read::read_to_end`
    /// that allocates once with capacity of `self.size`.
    pub fn into_bytes(mut self) -> io::Result<Vec<u8>> {
        let mut bytes = Vec::with_capacity(self.size as usize);
        self.read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    /// Reads the result into a `String`
    ///
    /// This is a convenience wrapper around `Read::read_to_string`
    /// that allocates once with capacity of `self.size`.
    pub fn into_string(mut self) -> io::Result<String> {
        let mut text = String::with_capacity(self.size as usize);
        self.read_to_string(&mut text)?;
        Ok(text)
    }
}

/// Algorithmia data file
pub struct DataFile {
    path: String,
    client: HttpClient,
}

impl HasDataPath for DataFile {
    #[doc(hidden)]
    fn new(client: HttpClient, path: &str) -> Self {
        DataFile {
            client: client,
            path: parse_data_uri(path).to_string(),
        }
    }
    #[doc(hidden)]
    fn path(&self) -> &str {
        &self.path
    }
    #[doc(hidden)]
    fn client(&self) -> &HttpClient {
        &self.client
    }
}

impl DataFile {
    /// Write to the Algorithmia Data API
    ///
    /// # Examples
    /// ```no_run
    /// # use algorithmia::Algorithmia;
    /// # use std::fs::File;
    /// let client = Algorithmia::client("111112222233333444445555566")?;
    ///
    /// client.file(".my/my_dir/string.txt").put("file_contents")?;
    /// client.file(".my/my_dir/bytes.txt").put("file_contents".as_bytes())?;
    ///
    /// let file = File::open("/path/to/file.jpg")?;
    /// client.file(".my/my_dir/file.jpg").put(file)?;
    /// # Ok::<(), Box<std::error::Error>>(())
    /// ```
    pub fn put<B>(&self, body: B) -> Result<(), Error>
    where
        B: Into<Body>,
    {
        let url = self.to_url()?;
        self.client
            .put(url)
            .body(body)
            .send()
            .with_context(|| format!("request error writing file '{}'", self.to_data_uri()))
            .and_then(process_http_response)
            .with_context(|| format!("response error writing file '{}'", self.to_data_uri()))?;

        Ok(())
    }

    /// Get a file from the Algorithmia Data API
    ///
    /// # Examples
    /// ```no_run
    /// # use algorithmia::Algorithmia;
    /// # use std::io::Read;
    /// let client = Algorithmia::client("111112222233333444445555566")?;
    /// let my_file = client.file(".my/my_dir/sample.txt");
    ///
    /// let data = my_file.get()?.into_string()?;
    /// # Ok::<_, Box<std::error::Error>>(())
    /// ```
    pub fn get(&self) -> Result<FileData, Error> {
        let url = self.to_url()?;
        let req = self.client.get(url);
        let res = req
            .send()
            .with_context(|| format!("request error downloading file '{}'", self.to_data_uri()))
            .and_then(process_http_response)
            .with_context(|| format!("response error downloading file '{}'", self.to_data_uri()))?;

        let metadata = parse_headers(res.headers())?;
        match metadata.data_type {
            DataType::File => (),
            DataType::Dir => {
                bail!("expected API response with data type 'file', received 'directory'")
            }
        }

        Ok(FileData {
            size: metadata.content_length.unwrap_or(0),
            last_modified: metadata
                .last_modified
                .unwrap_or_else(|| Utc.ymd(2015, 3, 14).and_hms(8, 0, 0)),
            data: Box::new(res),
        })
    }

    /// Delete a file from from the Algorithmia Data API
    ///
    /// # Examples
    /// ```no_run
    /// # use algorithmia::Algorithmia;
    /// let client = Algorithmia::client("111112222233333444445555566")?;
    /// let my_file = client.file(".my/my_dir/sample.txt");
    ///
    /// match my_file.delete() {
    ///   Ok(_) => println!("Successfully deleted file"),
    ///   Err(err) => println!("Error deleting file: {}", err),
    /// };
    /// # Ok::<(), Box<std::error::Error>>(())
    /// ```
    pub fn delete(&self) -> Result<(), Error> {
        let url = self.to_url()?;
        let req = self.client.delete(url);
        req.send()
            .with_context(|| format!("request error deleting file '{}'", self.to_data_uri()))
            .and_then(process_http_response)
            .with_context(|| format!("response error deleting file '{}'", self.to_data_uri()))?;

        Ok(())
    }
}
