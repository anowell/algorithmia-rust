//! Directory module for managing Algorithmia Data Directories
//!
//! # Examples
//!
//! ```no_run
//! use algorithmia::Service;
//! use std::fs::File;
//!
//! let service = Service::new("111112222233333444445555566");
//! let my_dir = service.dir(".my/my_dir");
//!
//! my_dir.create();
//! my_dir.put_file("/path/to/file");
//! ```

extern crate hyper;
extern crate chrono;

use {Service, AlgorithmiaError, ApiErrorResponse};
use hyper::Url;
use hyper::status::StatusCode;
use rustc_serialize::{json, Decoder};
use std::io::Read;
use std::fs::File;
use std::path::Path;
use hyper::header::ContentType;
use mime::{Mime, TopLevel, SubLevel};
use self::chrono::{DateTime, UTC};
use super::{DataObject, FileAddedResult, FileAdded};
use std::ops::Deref;

/// Algorithmia Data Directory
pub struct DataDir {
    data_object: DataObject,
}

impl Deref for DataDir {
    type Target = DataObject;
    fn deref(&self) -> &DataObject {&self.data_object}
}

pub type DirectoryShowResult = Result<DirectoryShow, AlgorithmiaError>;
pub type DirectoryCreatedResult = Result<(), AlgorithmiaError>;
pub type DirectoryDeletedResult = Result<DirectoryDeleted, AlgorithmiaError>;

#[derive(RustcDecodable, Debug)]
pub struct DirectoryUpdated {
    pub acl: Option<DataAcl>,
}

#[derive(RustcDecodable, Debug)]
pub struct DeletedResult {
    pub deleted: u64,
}

/// Response when deleting a new Directory
#[derive(RustcDecodable, Debug)]
pub struct DirectoryDeleted {
    // Omitting deleted.number and error.number for now
    pub result: DeletedResult,
}

#[derive(RustcDecodable, RustcEncodable, Debug)]
pub struct DataFolder {
    pub name: String,
    pub acl: Option<DataAcl>,
}

#[derive(RustcDecodable, Debug)]
pub struct DataFile {
    pub filename: String,
    pub last_modified: DateTime<UTC>,
    pub size: u64,
}

#[derive(RustcDecodable, RustcEncodable, Debug)]
pub struct DataAcl {
    pub read: Vec<String>
}

/// Response when querying an existing Directory
#[derive(RustcDecodable, Debug)]
pub struct DirectoryShow {
    pub folders: Option<Vec<DataFolder>>,
    pub files: Option<Vec<DataFile>>,
    pub marker: Option<String>,
    pub acl: Option<DataAcl>,
}



impl DataDir {
    pub fn new(service: Service, data_uri: &str) -> DataDir {
        DataDir {
            data_object: DataObject::new(service, data_uri),
        }
    }


    /// Display Directory details if it exists
    ///
    /// # Examples
    /// ```no_run
    /// # use algorithmia::Service;
    /// let service = Service::new("111112222233333444445555566");
    /// let my_dir = service.dir(".my/my_dir");
    /// match my_dir.show() {
    ///   Ok(dir) => println!("Files: {}", dir.files.unwrap().iter().map(|f| f.filename.clone()).collect::<Vec<_>>().connect(", ")),
    ///   Err(e) => println!("ERROR: {:?}", e),
    /// };
    /// ```
    pub fn show(&self) -> DirectoryShowResult {
        let ref mut api_client = self.service.api_client();
        let req = api_client.get(self.to_url());

        let mut res = try!(req.send());
        let mut res_json = String::new();
        try!(res.read_to_string(&mut res_json));

        match json::decode::<DirectoryShow>(&res_json) {
            Ok(result) => Ok(result),
            Err(why) => match json::decode::<ApiErrorResponse>(&res_json) {
                Ok(err_res) => Err(AlgorithmiaError::AlgorithmiaApiError(err_res.error)),
                Err(_) => Err(AlgorithmiaError::DecoderErrorWithContext(why, res_json)),
            }
        }
    }

    /// Create a Directory
    ///
    /// # Examples
    /// ```no_run
    /// # use algorithmia::Service;
    /// let service = Service::new("111112222233333444445555566");
    /// let my_dir = service.dir(".my/my_dir");
    /// match my_dir.create() {
    ///   Ok(_) => println!("Successfully created Directory"),
    ///   Err(e) => println!("ERROR creating Directory: {:?}", e),
    /// };
    /// ```
    pub fn create(&self) -> DirectoryCreatedResult {
        // Construct URL
        // let url_string = format!("{}/{}/{}", Service::get_api(), Directory_BASE_PATH, self.parent());
        let url = self.parent().unwrap().to_url(); //TODO: don't unwrap this

        let input_data = DataFolder {
            name: self.basename().unwrap().to_string(), //TODO: don't unwrap this
            acl: Some(DataAcl { read: vec![] }),
        };
        let raw_input = try!(json::encode(&input_data));

        // POST request
        let ref mut api_client = self.service.api_client();
        let req = api_client.post(url)
            .header(ContentType(Mime(TopLevel::Application, SubLevel::Json, vec![])))
            .body(&raw_input);

        // Parse response
        let mut res = try!(req.send());

        match res.status {
            StatusCode::Ok | StatusCode::Created => Ok(()),
            _ => {
                let mut res_json = String::new();
                try!(res.read_to_string(&mut res_json));
                Err(Service::decode_to_error(res_json))
            }
        }
    }


    /// Delete a Directory
    ///
    /// # Examples
    /// ```no_run
    /// # use algorithmia::Service;
    /// let service = Service::new("111112222233333444445555566");
    /// let my_dir = service.dir(".my/my_dir");
    /// match my_dir.delete() {
    ///   Ok(_) => println!("Successfully deleted Directory"),
    ///   Err(e) => println!("ERROR deleting Directory: {:?}", e),
    /// };
    /// ```
    pub fn delete(&self) -> DirectoryDeletedResult {
        // DELETE request
        let ref mut api_client = self.service.api_client();
        let req = api_client.delete(self.to_url());

        // Parse response
        let mut res = try!(req.send());
        let mut res_json = String::new();
        try!(res.read_to_string(&mut res_json));

        Service::decode_to_result::<DirectoryDeleted>(res_json)
    }


    /// Upload a file to an existing Directory
    ///
    /// # Examples
    /// ```no_run
    /// # use algorithmia::Service;
    /// let service = Service::new("111112222233333444445555566");
    /// let my_dir = service.dir(".my/my_dir");
    ///
    /// match my_dir.put_file("/path/to/file") {
    ///   Ok(response) => println!("Successfully uploaded to: {}", response.result),
    ///   Err(e) => println!("ERROR uploading file: {:?}", e),
    /// };
    /// ```
    pub fn put_file<P: AsRef<Path>>(&self, file_path: P) -> FileAddedResult {
        // FIXME: A whole lot of unwrap going on here...
        let path_ref = file_path.as_ref();
        let url_string = format!("{}/{}",
            self.to_url(),
            path_ref.file_name().unwrap().to_str().unwrap()
        );
        let url = Url::parse(&url_string).unwrap();

        let mut file = File::open(path_ref).unwrap();
        let ref mut api_client = self.service.api_client();
        let req = api_client.post(url).body(&mut file);

        let mut res = try!(req.send());
        let mut res_json = String::new();
        try!(res.read_to_string(&mut res_json));

        Service::decode_to_result::<FileAdded>(res_json)
    }

}


#[cfg(test)]
mod tests {
    use super::*;
    // use super::super::DataPath;
    use Service;

    fn mock_service() -> Service { Service::new("") }

    #[test]
    fn test_to_url() {
        let dir = DataDir::new(mock_service(), "data://anowell/foo");
        assert_eq!(dir.to_url().serialize(), format!("{}/v1/data/anowell/foo", Service::get_api()));
    }

    #[test]
    fn test_to_data_uri() {
        let dir = DataDir::new(mock_service(), "/anowell/foo");
        assert_eq!(dir.to_data_uri(), "data://anowell/foo".to_string());
    }

    #[test]
    fn test_parent() {
        let dir = DataDir::new(mock_service(), "data://anowell/foo");
        let expected = DataDir::new(mock_service(), "data://anowell");
        assert_eq!(dir.parent().unwrap().path, expected.path);
    }
}