#[macro_use]
extern crate lazy_static;

use anyhow::Error;
use spate_bencode::Value;
use std::convert::{Into, TryFrom};

lazy_static! {
    static ref ANNOUNCE_KEY: Value = Value::from("announce");
    static ref ANNOUNCE_LIST_KEY: Value = Value::from("announce list");
}

#[derive(Debug)]
pub struct MetaInfo<'a> {
    // A dictionary that describes the file(s) of the torrent.
    info: MetaInfoFiles,
    // The announce URL of the tracker
    announce: &'a str,
    // This is an extension to the official specification, offering backwards-compatibility.
    announce_list: Option<Vec<&'a str>>,
    // The creation time of the torrent, in standard UNIX epoch format (integer, seconds since 1-Jan-1970 00:00:00 UTC)
    creation_date: Option<usize>,
    // Free-form textual comments of the author
    comment: Option<String>,
    // Name and version of the program used to create the torrent
    created_by: Option<String>,
    // The string encoding format used to generate the pieces part of the info dictionary
    encoding: Option<String>,
}

#[derive(Debug)]
pub struct MetaInfoFiles {
    piece_length: i32,
    pieces: Vec<String>,
    private: bool,
    files: MetaInfoFileMode,
}

#[derive(Debug)]
pub enum MetaInfoFileMode {
    SingleFile(MetaInfoSingleFile),
    MultiFile(MetaInfoMultiFiles),
}

#[derive(Debug)]
pub struct MetaInfoSingleFile {
    file_name: String,
    length: usize,
    md5sum: Option<String>,
}

#[derive(Debug)]
pub struct MetaInfoMultiFiles {
    directory_name: String,
    files: Vec<MetaInfoMultiFileEntry>,
}

#[derive(Debug)]
pub struct MetaInfoMultiFileEntry {
    length: usize,
    md5sum: Option<String>,
    path: Vec<String>,
}

impl<'a> TryFrom<&'a Value> for MetaInfo<'a> {
    type Error = Error;

    fn try_from(value: &'a Value) -> Result<Self, Self::Error> {
        match value {
            Value::Dict(dict) => Ok(Self {
                info: MetaInfoFiles {
                    piece_length: 0,
                    pieces: vec![],
                    private: false,
                    files: MetaInfoFileMode::SingleFile(MetaInfoSingleFile {
                        file_name: String::from("foo"),
                        length: 0,
                        md5sum: None,
                    }),
                },
                announce: dict
                    .get(&ANNOUNCE_KEY)
                    .expect("announce key not found in dict")
                    .try_into()?,
                announce_list: dict.get(&ANNOUNCE_LIST_KEY)?.try_into()?,
                creation_date: Some(0),
                comment: None,
                created_by: None,
                encoding: None,
            }),
            _ => Err(Error::msg("expected dict")),
        }
    }
}

impl Into<Value> for MetaInfo<'_> {
    fn into(self) -> Value {
        Value::Bytes("test".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
