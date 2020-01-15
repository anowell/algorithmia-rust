//! API client for managing data through Algorithmia
//!
//! Instantiate from the [`Algorithmia`](../struct.Algorithmia.html) struct

pub use self::dir::*;
pub use self::file::*;
pub use self::object::*;
pub use self::path::*;

use crate::error::{err_msg, Error};
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use headers::{ContentLength, Date, HeaderMapExt};
use http::header::HeaderMap;
use std::ops::Deref;
use std::time::{SystemTime, UNIX_EPOCH};

mod dir;
mod file;
mod object;
mod path;

static DATA_BASE_PATH: &'static str = "v1/connector";

use crate::client::header::{lossy_header, X_DATA_TYPE};

/// Minimal representation of data type
pub enum DataType {
    File,
    Dir,
}

/// Data type wrapping the data item (including any metadata)
pub enum DataItem {
    File(DataFileItem),
    Dir(DataDirItem),
}

/// `DataFile` wrapper with metadata
pub struct DataFileItem {
    /// Size of file in bytes
    pub size: u64,
    /// Last modified timestamp
    pub last_modified: DateTime<Utc>,
    file: DataFile,
}

impl Deref for DataFileItem {
    type Target = DataFile;
    fn deref(&self) -> &DataFile {
        &self.file
    }
}

/// `DataDir` wrapper (currently no metadata)
pub struct DataDirItem {
    dir: DataDir,
}

impl Deref for DataDirItem {
    type Target = DataDir;
    fn deref(&self) -> &DataDir {
        &self.dir
    }
}

struct HeaderData {
    pub data_type: DataType,
    pub content_length: Option<u64>,
    pub last_modified: Option<DateTime<Utc>>,
}

fn parse_headers(headers: &HeaderMap) -> Result<HeaderData, Error> {
    let data_type = match &headers.get(X_DATA_TYPE).map(lossy_header) {
        Some(dt) if dt == "directory" => DataType::Dir,
        Some(dt) if dt == "file" => DataType::File,
        Some(dt) => {
            return Err(err_msg(format!(
                "API responded with invalid data type: '{}'",
                dt.to_string()
            )));
        }
        None => return Err(err_msg("API response missing data type")),
    };

    let content_length = headers.typed_get::<ContentLength>().map(|c| c.0);
    let last_modified = headers.typed_get::<Date>().map(|d| {
        let time = SystemTime::from(d);
        let ts = time
            .duration_since(UNIX_EPOCH)
            .expect("date header predates unix epoch");
        let naive_datetime =
            NaiveDateTime::from_timestamp(ts.as_secs() as i64, ts.subsec_nanos() as u32);
        Utc.from_utc_datetime(&naive_datetime)
    });

    Ok(HeaderData {
        data_type: data_type,
        content_length: content_length,
        last_modified: last_modified,
    })
}

fn parse_data_uri(data_uri: &str) -> String {
    match data_uri {
        p if p.contains("://") => p.split_terminator("://").collect::<Vec<_>>().join("/"),
        p if p.starts_with('/') => format!("data/{}", &p[1..]),
        p => format!("data/{}", p),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_data_uri;

    #[test]
    fn test_parse_protocol() {
        assert_eq!(parse_data_uri("data://"), "data");
        assert_eq!(parse_data_uri("data://foo"), "data/foo");
        assert_eq!(parse_data_uri("data://foo/"), "data/foo/");
        assert_eq!(parse_data_uri("data://foo/bar"), "data/foo/bar");
        assert_eq!(parse_data_uri("dropbox://"), "dropbox");
        assert_eq!(parse_data_uri("dropbox://foo"), "dropbox/foo");
        assert_eq!(parse_data_uri("dropbox://foo/"), "dropbox/foo/");
        assert_eq!(parse_data_uri("dropbox://foo/bar"), "dropbox/foo/bar");
    }

    #[test]
    fn test_parse_leading_slash() {
        assert_eq!(parse_data_uri("/foo"), "data/foo");
        assert_eq!(parse_data_uri("/foo/"), "data/foo/");
        assert_eq!(parse_data_uri("/foo/bar"), "data/foo/bar");
    }

    #[test]
    fn test_parse_unprefixed() {
        assert_eq!(parse_data_uri("foo"), "data/foo");
        assert_eq!(parse_data_uri("foo/"), "data/foo/");
        assert_eq!(parse_data_uri("foo/bar"), "data/foo/bar");
    }
}
