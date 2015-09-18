//! Directory module for managing Algorithmia Data Directories
//!
//! # Examples
//!
//! ```no_run
//! use algorithmia::Algorithmia;
//! use std::fs::File;
//!
//! let client = Algorithmia::client("111112222233333444445555566");
//! let my_dir = client.dir(".my/my_dir");
//!
//! my_dir.create();
//! my_dir.put_file("/path/to/file");
//! ```

extern crate chrono;

use client::HttpClient;
use error::*;
use data::{self, DataFile, DeletedResult, XDataType};
use data::file::FileAdded;
use data::HasDataPath;
use json_helpers;

use std::io::Read;
use std::fs::File;
use std::path::Path;
use std::vec::IntoIter;
use std::ops::Deref;
use std::error::Error as StdError;

use hyper::header::ContentType;
use hyper::mime::{Mime, TopLevel, SubLevel};
use hyper::Url;
use rustc_serialize::{json, Decoder, Decodable};
use self::chrono::{DateTime, UTC};


/// Algorithmia Data Directory
pub struct DataDir {
    path: String,
    client: HttpClient,
}


#[derive(RustcDecodable, Debug)]
pub struct DirectoryUpdated {
    pub acl: Option<DataAcl>,
}


/// Response when deleting a new Directory
#[derive(RustcDecodable, Debug)]
pub struct DirectoryDeleted {
    // Omitting deleted.number and error.number for now
    pub result: DeletedResult,
}

#[derive(RustcDecodable, RustcEncodable, Debug)]
struct FolderItem {
    pub name: String,
    pub acl: Option<DataAcl>,
}

#[derive(Debug)]
struct FileItem {
    pub filename: String,
    pub size: u64,
    pub last_modified: DateTime<UTC>,
}

// Manual implemented Decodable: https://github.com/lifthrasiir/rust-chrono/issues/43
impl Decodable for FileItem {
    fn decode<D: Decoder>(d: &mut D) -> Result<FileItem, D::Error> {
        d.read_struct("root", 0, |d| {
            Ok(FileItem{
                filename: try!(d.read_struct_field("filename", 0, |d| Decodable::decode(d))),
                size: try!(d.read_struct_field("size", 0, |d| Decodable::decode(d))),
                last_modified: {
                    let json_str: String = try!(d.read_struct_field("last_modified", 0, |d| Decodable::decode(d)));
                    match json_str.parse() {
                        Ok(datetime) => datetime,
                        Err(err) => return Err(d.error(err.description())),
                    }
                },
            })
        })
    }
}


#[derive(RustcDecodable, RustcEncodable, Debug)]
pub struct DataAcl {
    pub read: Vec<String>
}

/// Response when querying an existing Directory
#[derive(RustcDecodable, Debug)]
struct DirectoryShow {
    pub acl: Option<DataAcl>,
    pub folders: Option<Vec<FolderItem>>,
    pub files: Option<Vec<FileItem>>,
    pub marker: Option<String>,
}


pub struct DirectoryListing<'a> {
    pub acl: Option<DataAcl>,
    dir: &'a DataDir,
    folders: IntoIter<FolderItem>,
    files: IntoIter<FileItem>,
    marker: Option<String>,
    query_count: u32,
}

impl <'a> DirectoryListing<'a> {
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

pub struct DataFileEntry {
    pub size: u64,
    pub last_modified: DateTime<UTC>,
    file: DataFile,
}

impl Deref for DataFileEntry {
    type Target = DataFile;
    fn deref(&self) -> &DataFile {&self.file}
}

pub enum DirEntry {
    File(DataFileEntry),
    Dir(DataDir),
}

impl <'a> Iterator for DirectoryListing<'a> {
    type Item = Result<DirEntry, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.folders.next() {
            // Return folders first
            Some(d) => Some(Ok(DirEntry::Dir(self.dir.child(&d.name)))),
            None => match self.files.next() {
                // Return files second
                Some(f) => Some(Ok(DirEntry::File(
                    DataFileEntry{
                        size: f.size,
                        last_modified: f.last_modified,
                        file: self.dir.child(&f.filename),
                    }
                ))),
                None => {
                    // Query if there is another page of files/folders
                    if self.query_count == 0 || self.marker.is_some() {
                        match get_directory(self.dir, self.marker.clone()) {
                            Ok(ds) => {
                                self.query_count = self.query_count + 1;
                                self.folders = ds.folders.unwrap_or(Vec::new()).into_iter();
                                self.files = ds.files.unwrap_or(Vec::new()).into_iter();
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

fn get_directory(dir: &DataDir, marker: Option<String>) -> Result<DirectoryShow, Error> {
    let url = match marker {
        Some(m) => Url::parse(&format!("{}?marker={}", dir.to_url(), m)).unwrap(),
        None => dir.to_url(),
    };

    let req = dir.client.get(url);
    let mut res = try!(req.send());

    if res.status.is_success() {
        if let Some(data_type) = res.headers.get::<XDataType>() {
            if "directory" != data_type.to_string() {
                return Err(Error::DataTypeError(format!("Expected directory, Received {}", data_type)));
            }
        }
    }

    let mut res_json = String::new();
    try!(res.read_to_string(&mut res_json));

    match res.status.is_success() {
        true => json_helpers::decode(&res_json),
        false => Err(json_helpers::decode_to_error(&res_json)),
    }
}

impl HasDataPath for DataDir {
    fn new(client: HttpClient, path: &str) -> Self { DataDir { client: client, path: data::parse_data_uri(path).to_string() } }
    fn path(&self) -> &str { &self.path }
    fn client(&self) -> &HttpClient { &self.client }
}

impl DataDir {
    /// Display Directory details if it exists
    ///
    /// # Examples
    /// ```no_run
    /// # use algorithmia::Algorithmia;
    /// # use algorithmia::data::{DirEntry, HasDataPath};
    /// let client = Algorithmia::client("111112222233333444445555566");
    /// let my_dir = client.dir(".my/my_dir");
    /// let dir_list = my_dir.list();
    /// for entry in dir_list {
    ///     match entry {
    ///         Ok(DirEntry::File(f)) => println!("File: {}", f.to_data_uri()),
    ///         Ok(DirEntry::Dir(d)) => println!("Dir: {}", d.to_data_uri()),
    ///         Err(err) => { println!("Error: {}", err); break; },
    ///     }
    /// };
    /// ```
    pub fn list(&self) -> DirectoryListing {
        DirectoryListing::new(self)
    }

    /// Create a Directory
    ///
    /// # Examples
    /// ```no_run
    /// # use algorithmia::Algorithmia;
    /// let client = Algorithmia::client("111112222233333444445555566");
    /// let my_dir = client.dir(".my/my_dir");
    /// match my_dir.create() {
    ///   Ok(_) => println!("Successfully created Directory"),
    ///   Err(e) => println!("Error created directory: {}", e),
    /// };
    /// ```
    pub fn create(&self) -> Result<(), Error> {
        let parent = try!(self.parent().ok_or(Error::DataPathError("has no parent".into())));
        let url = parent.to_url();

        // TODO: address complete abuse of this structure
        let input_data = FolderItem {
            name: try!(self.basename().ok_or(Error::DataPathError("has no basename".into()))).into(),
            acl: Some(DataAcl { read: vec![] }),
        };
        let raw_input = try!(json::encode(&input_data));

        // POST request
        let req = self.client.post(url)
            .header(ContentType(Mime(TopLevel::Application, SubLevel::Json, vec![])))
            .body(&raw_input);

        // Parse response
        let mut res = try!(req.send());

        match res.status.is_success() {
            true => Ok(()),
            false => {
                let mut res_json = String::new();
                try!(res.read_to_string(&mut res_json));
                Err(json_helpers::decode_to_error(&res_json))
            }
        }
    }


    /// Delete a Directory
    ///
    /// # Examples
    /// ```no_run
    /// # use algorithmia::Algorithmia;
    /// let client = Algorithmia::client("111112222233333444445555566");
    /// let my_dir = client.dir(".my/my_dir");
    /// match my_dir.delete(false) {
    ///   Ok(_) => println!("Successfully deleted Directory"),
    ///   Err(err) => println!("Error deleting directory: {}", err),
    /// };
    /// ```
    pub fn delete(&self, force: bool) -> Result<DirectoryDeleted, Error> {
        // DELETE request
        let url_string = format!("{}?force={}", self.to_url(), force.to_string());
        let url = Url::parse(&url_string).unwrap();
        let req = self.client.delete(url);

        // Parse response
        let mut res = try!(req.send());
        let mut res_json = String::new();
        try!(res.read_to_string(&mut res_json));

        match res.status.is_success() {
            true => json_helpers::decode(&res_json),
            false => Err(json_helpers::decode_to_error(&res_json)),
        }
    }


    /// Upload a file to an existing Directory
    ///
    /// # Examples
    /// ```no_run
    /// # use algorithmia::Algorithmia;
    /// let client = Algorithmia::client("111112222233333444445555566");
    /// let my_dir = client.dir(".my/my_dir");
    ///
    /// match my_dir.put_file("/path/to/file") {
    ///   Ok(response) => println!("Successfully uploaded to: {}", response.result),
    ///   Err(err) => println!("Error uploading file: {}", err),
    /// };
    /// ```
    pub fn put_file<P: AsRef<Path>>(&self, file_path: P) -> Result<FileAdded, Error> {
        // FIXME: A whole lot of unwrap going on here...
        let path_ref = file_path.as_ref();
        let url_string = format!("{}/{}",
            self.to_url(),
            path_ref.file_name().unwrap().to_str().unwrap()
        );
        let url = Url::parse(&url_string).unwrap();

        let mut file = File::open(path_ref).unwrap();
        let req = self.client.put(url).body(&mut file);

        let mut res = try!(req.send());
        let mut res_json = String::new();
        try!(res.read_to_string(&mut res_json));

        match res.status.is_success() {
            true => json_helpers::decode(&res_json),
            false => Err(json_helpers::decode_to_error(&res_json)),
        }
    }

    pub fn child<T: HasDataPath>(&self, filename: &str) -> T {
        T::new(self.client.clone(), &format!("{}/{}", self.to_data_uri(), filename))
    }
}


#[cfg(test)]
mod tests {
    use data::HasDataPath;
    use Algorithmia;

    fn mock_client() -> Algorithmia { Algorithmia::client("") }

    #[test]
    fn test_to_url() {
        let dir = mock_client().dir("data://anowell/foo");
        assert_eq!(dir.to_url().serialize_path().unwrap(), "/v1/data/anowell/foo");
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
    }
}