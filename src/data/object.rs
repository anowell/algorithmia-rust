use super::{algo_epoch, parse_data_uri, parse_headers};
use crate::client::HttpClient;
use crate::data::*;
use crate::error::{process_http_response, Error, ResultExt};

/// Algorithmia data object (file or directory)
pub struct DataObject {
    path: String,
    client: HttpClient,
}

impl HasDataPath for DataObject {
    #[doc(hidden)]
    fn new(client: HttpClient, path: &str) -> Self {
        DataObject {
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

impl DataObject {
    /// Determine if a particular data URI is for a file or directory
    ///
    /// ```no_run
    /// # use algorithmia::Algorithmia;
    /// # use algorithmia::data::{DataType, HasDataPath};
    /// # let client = Algorithmia::client("111112222233333444445555566")?;
    /// let my_obj = client.data("data://.my/some/path");
    /// match my_obj.get_type()? {
    ///     DataType::File => println!("{} is a file", my_obj.to_data_uri()),
    ///     DataType::Dir => println!("{} is a directory", my_obj.to_data_uri()),
    /// }
    /// # Ok::<(), Box<std::error::Error>>(())
    /// ```
    pub fn get_type(&self) -> Result<DataType, Error> {
        let url = self.to_url()?;
        let req = self.client.head(url);
        let res = req
            .send()
            .with_context(|| format!("request error getting type of '{}'", self.to_data_uri()))
            .and_then(process_http_response)
            .with_context(|| format!("response error getting type of '{}'", self.to_data_uri()))?;

        let metadata = parse_headers(res.headers())?;
        Ok(metadata.data_type)
    }

    /// Determine if a data URI is for a file or directory and convert into the appropriate type
    ///
    /// ```no_run
    /// # use algorithmia::Algorithmia;
    /// # use algorithmia::data::{DataItem, HasDataPath};
    /// # let client = Algorithmia::client("111112222233333444445555566")?;
    /// let my_obj = client.data("data://.my/some/path");
    /// match my_obj.into_type()? {
    ///     DataItem::File(f) => println!("{} is a file", f.to_data_uri()),
    ///     DataItem::Dir(d) => println!("{} is a directory", d.to_data_uri()),
    /// }
    /// # Ok::<(), Box<std::error::Error>>(())
    /// ```
    pub fn into_type(self) -> Result<DataItem, Error> {
        let metadata = {
            let url = self.to_url()?;
            let req = self.client.head(url);
            let res = req
                .send()
                .with_context(|| format!("request error getting type of '{}'", self.to_data_uri()))
                .and_then(process_http_response)
                .with_context(|| {
                    format!("response error getting type of '{}'", self.to_data_uri())
                })?;
            parse_headers(res.headers())?
        };

        match metadata.data_type {
            DataType::Dir => Ok(DataItem::Dir(DataDirItem { dir: self.into() })),
            DataType::File => Ok(DataItem::File(DataFileItem {
                size: metadata.content_length.unwrap_or(0),
                last_modified: metadata.last_modified.unwrap_or_else(algo_epoch),
                file: self.into(),
            })),
        }
    }
}

impl From<DataObject> for DataDir {
    fn from(d: DataObject) -> Self {
        let uri = d.to_data_uri();
        DataDir::new(d.client, &uri)
    }
}

impl From<DataObject> for DataFile {
    fn from(d: DataObject) -> Self {
        let uri = d.to_data_uri();
        DataFile::new(d.client, &uri)
    }
}
