//! Directory module for managing Algorithmia Data Directories
//!
//! # Examples
//!
//! ```no_run
//! use algorithmia::Algorithmia;
//! use algorithmia::data::DataAcl;
//!
//! let client = Algorithmia::client("111112222233333444445555566")?;
//! let my_dir = client.dir(".my/my_dir");
//!
//! my_dir.create(DataAcl::default())?;
//! my_dir.put_file("/path/to/file")?;
//! # Ok::<(), Box<std::error::Error>>(())
//! ```

use super::parse_data_uri;
use crate::client::header::{lossy_header, X_DATA_TYPE};
use crate::client::HttpClient;
use crate::data::{DataDirItem, DataFile, DataFileItem, DataItem, HasDataPath};
use crate::error::{err_msg, process_http_response, Error, ResultExt};

use std::fs::File;
use std::path::Path;
use std::vec::IntoIter;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Algorithmia Data Directory
pub struct DataDir {
    path: String,
    client: HttpClient,
}

#[derive(Debug, Deserialize)]
struct DeletedResponse {
    result: DirectoryDeleted,
}

/// Response when deleting a file form the Data API
#[derive(Debug, Deserialize)]
pub struct DirectoryDeleted {
    /// Number of files that were deleted
    ///
    /// Note: some backing stores may indicate deletion succeeds for non-existing files
    pub deleted: u64,
    // Placeholder for API stability if additional fields are added later
    #[serde(skip_deserializing)]
    _dummy: (),
}

#[derive(Debug, Deserialize, Serialize)]
struct FolderItem {
    pub name: String,
    pub acl: Option<DataAcl>,
}

#[derive(Debug, Deserialize)]
struct FileItem {
    pub filename: String,
    pub size: u64,
    pub last_modified: DateTime<Utc>,
}

/// ACL that indicates permissions for a `DataDir`
/// See also: [`ReadAcl`](enum.ReadAcl.html) enum to construct a `DataACL`
#[derive(Debug, Deserialize, Serialize)]
pub struct DataAcl {
    /// Read ACL
    pub read: Vec<String>,
    // Placeholder for stability with API additions
    _dummy: (),
}

/// Read access control values
pub enum ReadAcl {
    /// Readable only by owner
    Private,
    /// Readable by owner's algorithms (regardless of caller)
    MyAlgorithms,
    /// Readable by any user
    Public,

    /// Non-exhaustive for API stability if ACL types are added
    #[doc(hidden)]
    __Nonexhaustive,
}

impl Default for DataAcl {
    fn default() -> Self {
        ReadAcl::MyAlgorithms.into()
    }
}

impl From<ReadAcl> for DataAcl {
    fn from(acl: ReadAcl) -> Self {
        match acl {
            ReadAcl::Private | ReadAcl::__Nonexhaustive => DataAcl {
                read: vec![],
                _dummy: (),
            },
            ReadAcl::MyAlgorithms => DataAcl {
                read: vec!["algo://.my/*".into()],
                _dummy: (),
            },
            ReadAcl::Public => DataAcl {
                read: vec!["user://*".into()],
                _dummy: (),
            },
        }
    }
}

/// Response when querying an existing Directory
#[derive(Debug, Deserialize)]
struct DirectoryShow {
    pub acl: Option<DataAcl>,
    pub folders: Option<Vec<FolderItem>>,
    pub files: Option<Vec<FileItem>>,
    pub marker: Option<String>,
}

/// Iterator over the listing of a `DataDir`
pub struct DirectoryListing<'a> {
    /// ACL indicates permissions for this `DataDir`
    pub acl: Option<DataAcl>,
    dir: &'a DataDir,
    folders: IntoIter<FolderItem>,
    files: IntoIter<FileItem>,
    marker: Option<String>,
    query_count: u32,
}

impl<'a> DirectoryListing<'a> {
    fn new(dir: &'a DataDir) -> DirectoryListing<'a> {
        DirectoryListing {
            acl: None,
            dir: dir,
            folders: Vec::new().into_iter(),
            files: Vec::new().into_iter(),
            marker: None,
            query_count: 0,
        }
    }
}

impl<'a> Iterator for DirectoryListing<'a> {
    type Item = Result<DataItem, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.folders.next() {
            // Return folders first
            Some(d) => Some(Ok(DataItem::Dir(DataDirItem {
                dir: self.dir.child(&d.name),
            }))),
            None => {
                match self.files.next() {
                    // Return files second
                    Some(f) => Some(Ok(DataItem::File(DataFileItem {
                        size: f.size,
                        last_modified: f.last_modified,
                        file: self.dir.child(&f.filename),
                    }))),
                    None => {
                        // Query if there is another page of files/folders
                        if self.query_count == 0 || self.marker.is_some() {
                            self.query_count += 1;
                            match get_directory(self.dir, self.marker.clone()) {
                                Ok(ds) => {
                                    self.folders = ds.folders.unwrap_or_else(Vec::new).into_iter();
                                    self.files = ds.files.unwrap_or_else(Vec::new).into_iter();
                                    self.marker = ds.marker;
                                    self.next()
                                }
                                Err(err) => Some(Err(err)),
                            }
                        } else {
                            None
                        }
                    }
                }
            }
        }
    }
}

fn get_directory(dir: &DataDir, marker: Option<String>) -> Result<DirectoryShow, Error> {
    let mut url = dir.to_url()?;
    if let Some(ref m) = marker {
        url.query_pairs_mut().append_pair("marker", m);
    }

    let res = dir
        .client
        .get(url)
        .send()
        .with_context(|| format!("request error listing directory '{}'", dir.to_data_uri()))
        .and_then(process_http_response)
        .with_context(|| format!("response error listing directory '{}'", dir.to_data_uri()))?;

    match res.headers().get(X_DATA_TYPE).map(lossy_header) {
        Some(ref dt) if dt == "directory" => (),
        data_type => {
            let dt = data_type.unwrap_or_else(|| "unknown".to_string());
            bail!("expected content type '{}', received '{}'", "directory", dt)
        }
    }

    res.json().with_context(|| {
        format!(
            "JSON decoding error listing directory '{}'",
            dir.to_data_uri()
        )
    })
}

impl HasDataPath for DataDir {
    #[doc(hidden)]
    fn new(client: HttpClient, path: &str) -> Self {
        DataDir {
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

impl DataDir {
    /// Display Directory details if it exists
    ///
    /// # Examples
    /// ```no_run
    /// # use algorithmia::Algorithmia;
    /// # use algorithmia::data::{DataItem, HasDataPath};
    /// let client = Algorithmia::client("111112222233333444445555566")?;
    /// let my_dir = client.dir(".my/my_dir");
    /// let dir_list = my_dir.list();
    /// for entry in dir_list {
    ///     match entry {
    ///         Ok(DataItem::File(f)) => println!("File: {}", f.to_data_uri()),
    ///         Ok(DataItem::Dir(d)) => println!("Dir: {}", d.to_data_uri()),
    ///         Err(err) => { println!("Error: {}", err); break; },
    ///     }
    /// };
    /// # Ok::<(), Box<std::error::Error>>(())
    /// ```
    pub fn list(&self) -> DirectoryListing<'_> {
        DirectoryListing::new(self)
    }

    /// Create a Directory
    ///
    /// Use `DataAcl::default()` or the `ReadAcl` enum to set the ACL
    ///
    /// # Examples
    /// ```no_run
    /// # use algorithmia::Algorithmia;
    /// # use algorithmia::data::DataAcl;
    /// let client = Algorithmia::client("111112222233333444445555566")?;
    /// let my_dir = client.dir(".my/my_dir");
    /// match my_dir.create(DataAcl::default()) {
    ///   Ok(_) => println!("Successfully created Directory"),
    ///   Err(e) => println!("Error created directory: {}", e),
    /// };
    /// # Ok::<(), Box<std::error::Error>>(())
    /// ```
    pub fn create<Acl: Into<DataAcl>>(&self, acl: Acl) -> Result<(), Error> {
        let parent = self.parent().ok_or_else(|| {
            err_msg(format!(
                "URI {} does not have a valid parent",
                self.to_data_uri()
            ))
        })?;
        let parent_url = parent.to_url()?;

        let input_data = FolderItem {
            name: self
                .basename()
                .ok_or_else(|| {
                    err_msg(format!(
                        "Data URI {} does not have a valid basename",
                        self.to_data_uri()
                    ))
                })?
                .into(),
            acl: Some(acl.into()),
        };

        // POST request
        self.client
            .post(parent_url)
            .json(&input_data)
            .send()
            .with_context(|| format!("request error creating directory '{}'", self.to_data_uri()))
            .and_then(process_http_response)
            .with_context(|| {
                format!("response error creating directory '{}'", self.to_data_uri())
            })?;

        Ok(())
    }

    /// Delete a Directory
    ///
    /// # Examples
    /// ```no_run
    /// # use algorithmia::Algorithmia;
    /// let client = Algorithmia::client("111112222233333444445555566")?;
    /// let my_dir = client.dir(".my/my_dir");
    /// match my_dir.delete(false) {
    ///   Ok(_) => println!("Successfully deleted Directory"),
    ///   Err(err) => println!("Error deleting directory: {}", err),
    /// };
    /// # Ok::<(), Box<std::error::Error>>(())
    /// ```
    pub fn delete(&self, force: bool) -> Result<DirectoryDeleted, Error> {
        // DELETE request
        let mut url = self.to_url()?;
        if force {
            url.query_pairs_mut().append_pair("force", "true");
        }

        // Parse response
        let res = self
            .client
            .delete(url)
            .send()
            .with_context(|| format!("request error deleting directory '{}'", self.to_data_uri()))
            .and_then(process_http_response)
            .with_context(|| {
                format!("response error deleting directory '{}'", self.to_data_uri())
            })?;

        res.json::<DeletedResponse>()
            .map(|res| res.result)
            .with_context(|| {
                format!(
                    "JSON decoding error deleting directory '{}'",
                    self.to_data_uri()
                )
            })
    }

    /// Upload a file to an existing Directory
    ///
    /// # Examples
    /// ```no_run
    /// # use algorithmia::prelude::*;
    /// let client = Algorithmia::client("111112222233333444445555566")?;
    /// let my_dir = client.dir(".my/my_dir");
    ///
    /// match my_dir.put_file("/path/to/file") {
    ///   Ok(_) => println!("Successfully uploaded to: {}", my_dir.to_data_uri()),
    ///   Err(err) => println!("Error uploading file: {}", err),
    /// };
    /// # Ok::<(), Box<std::error::Error>>(())
    /// ```
    pub fn put_file<P: AsRef<Path>>(&self, file_path: P) -> Result<(), Error> {
        let path_ref = file_path.as_ref();
        let file = File::open(path_ref)
            .with_context(|| format!("opening file for upload '{}'", path_ref.display()))?;

        // Safe to unwrap: we've already opened the file or returned an error
        let filename = path_ref.file_name().unwrap().to_string_lossy();
        let data_file: DataFile = self.child(&filename);
        data_file.put(file)
    }

    /// Instantiate `DataFile` or `DataDir` as a child of this `DataDir`
    pub fn child<T: HasDataPath>(&self, filename: &str) -> T {
        let new_uri = match self.to_data_uri() {
            ref uri if uri.ends_with('/') => format!("{}{}", uri, filename),
            uri => format!("{}/{}", uri, filename),
        };
        T::new(self.client.clone(), &new_uri)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::HasDataPath;
    use crate::Algorithmia;

    fn mock_client() -> Algorithmia {
        Algorithmia::client("").unwrap()
    }

    #[test]
    fn test_to_url() {
        let dir = mock_client().dir("data://anowell/foo");
        assert_eq!(
            dir.to_url().unwrap().path(),
            "/v1/connector/data/anowell/foo"
        );
    }

    #[test]
    fn test_to_data_uri() {
        let dir = mock_client().dir("/anowell/foo");
        assert_eq!(dir.to_data_uri(), "data://anowell/foo".to_string());
    }

    #[test]
    fn test_parent() {
        let dir = mock_client().dir("data://anowell/foo");
        let expected = mock_client().dir("data://anowell");
        assert_eq!(dir.parent().unwrap().path, expected.path);

        let dir = mock_client().dir("dropbox://anowell/foo");
        let expected = mock_client().dir("dropbox://anowell");
        assert_eq!(dir.parent().unwrap().path, expected.path);

        let dir = mock_client().dir("data://anowell");
        let expected = mock_client().dir("data://");
        assert_eq!(dir.parent().unwrap().path, expected.path);

        let dir = mock_client().dir("data://");
        assert!(dir.parent().is_none());
    }

    #[test]
    fn test_default_acl() {
        let acl: DataAcl = DataAcl::default();
        assert_eq!(acl.read, vec!["algo://.my/*".to_string()]);
    }

    #[test]
    fn test_private_acl() {
        let acl: DataAcl = ReadAcl::Private.into();
        assert!(acl.read.is_empty());
    }

    #[test]
    fn test_public_acl() {
        let acl: DataAcl = ReadAcl::Public.into();
        assert_eq!(acl.read, vec!["user://*".to_string()]);
    }

    #[test]
    fn test_myalgos_acl() {
        let acl: DataAcl = ReadAcl::MyAlgorithms.into();
        assert_eq!(acl.read, vec!["algo://.my/*".to_string()]);
    }
}
