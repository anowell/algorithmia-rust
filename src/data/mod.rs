pub use self::dir::DataDir;
pub use self::file::{DataFile, FileAddedResult, FileAdded};
use hyper::Url;
use Service;

mod dir;
mod file;


static COLLECTION_BASE_PATH: &'static str = "v1/data";

pub struct DataObject {
    pub path: String,
    pub service: Service,
}

impl DataObject {
    fn new(service: Service, data_uri: &str) -> DataObject {
        let path = match data_uri {
            p if p.starts_with("data://") => p[7..].to_string(),
            p if p.starts_with("/") => p[1..].to_string(),
            p => p.to_string(),
        };

        DataObject {
            service: service,
            path: path
        }
    }

    /// Get the API Endpoint URL for a particular data URI
    pub fn to_url(&self) -> Url {
        let url_string = format!("{}/{}/{}", Service::get_api(), COLLECTION_BASE_PATH, self.path);
        Url::parse(&url_string).unwrap()
    }

    /// Get the Algorithmia data URI a given Data Object
    ///
    /// ```
    /// # use algorithmia::Service;
    /// # let service = Service::new("111112222233333444445555566");
    /// let my_dir = service.dir(".my/my_dir");
    /// assert_eq!(my_dir.to_data_uri(), "data://.my/my_dir");
    /// ```
    pub fn to_data_uri(&self) -> String {
        format!("data://{}", self.path)
    }

    /// Get the parent off a given Data Object
    ///
    /// ```
    /// # use algorithmia::Service;
    /// # let service = Service::new("111112222233333444445555566");
    /// let my_dir = service.file("data://.my/my_dir/my_file");
    /// assert_eq!(my_dir.parent().unwrap().path, ".my/my_dir");
    /// ```
    pub fn parent(&self) -> Option<DataDir>{
        match self.path.rsplitn(2, "/").nth(1) {
            Some(path) => Some(DataDir::new(self.service.clone(), &*path)),
            None => None
        }
    }

    /// Get the basename from the Data Object's path (i.e. unix `basename`)
    ///
    /// ```
    /// # use algorithmia::Service;
    /// # let service = Service::new("111112222233333444445555566");
    /// let my_dir = service.dir("data:///.my/my_dir");
    /// assert_eq!(my_dir.basename().unwrap(), "my_dir");
    /// ```
    pub fn basename(&self) -> Option<String> {
        match self.path.rsplitn(2, "/").next() {
            Some(s) => Some(s.to_string()),
            None => None
        }
    }
}