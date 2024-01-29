use anyhow::Error;
use std::{
    collections::BTreeMap,
    io::{self},
};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const DELIM_TOKEN: u8 = b':';
const INTEGER_TOKEN: u8 = b'i';
const LIST_TOKEN: u8 = b'l';
const DICT_TOKEN: u8 = b'd';
const END_TOKEN: u8 = b'e';

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum Value {
    Bytes(Vec<u8>),
    Integer(i64),
    List(Vec<Value>),
    Dict(BTreeMap<Value, Value>),
}

impl Value {
    pub async fn decode<R: AsyncBufRead + Unpin>(reader: &mut R) -> Result<Value, DecodeError> {
        Decoder::new(reader).read_anything().await
    }

    pub async fn encode<W: AsyncWrite + Unpin>(&self, writer: &mut W) -> Result<(), io::Error> {
        Encoder::new(writer).write_anything(self).await
    }
}

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bytes(arg0) => f
                .debug_tuple("Bytes")
                .field(&String::from_utf8_lossy(arg0))
                .finish(),
            Self::Integer(arg0) => f.debug_tuple("Integer").field(arg0).finish(),
            Self::List(arg0) => f.debug_tuple("List").field(arg0).finish(),
            Self::Dict(arg0) => f.debug_tuple("Dict").field(arg0).finish(),
        }
    }
}

impl<'a> TryInto<&'a str> for &'a Value {
    type Error = Error;

    fn try_into(self) -> Result<&'a str, Self::Error> {
        match self {
            Value::Bytes(ref b) => std::str::from_utf8(b).map_err(Error::new),
            _ => Err(Error::msg("Only Byte values can be converted into strings")),
        }
    }
}

impl<'a> TryInto<Vec<&'a str>> for &'a Value {
    type Error = Error;

    fn try_into(self) -> Result<Vec<&'a str>, Self::Error> {
        match self {
            Value::List(ref l) => l.iter().map(|i| i.try_into()).collect(),
            _ => Err(Error::msg("Only List values can be converted into a Vec")),
        }
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::Bytes(value.into())
    }
}

#[derive(Debug)]
pub enum DecodeError {
    IO(std::io::Error),
    DECODER(&'static str),
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IO(arg0) => write!(f, "IO Error: {}", arg0),
            Self::DECODER(arg0) => write!(f, "Decoder Error: {}", arg0),
        }
    }
}

impl std::error::Error for DecodeError {}

pub struct Decoder<'a, R: AsyncBufRead + Unpin> {
    reader: &'a mut R,
}

impl<'a, R: AsyncBufRead + Unpin> Decoder<'a, R> {
    pub fn new(reader: &'a mut R) -> Self {
        Self { reader }
    }

    pub async fn read_anything(&mut self) -> Result<Value, DecodeError> {
        match self
            .reader
            .fill_buf()
            .await
            .map_err(DecodeError::IO)?
            .first()
        {
            Some(i) => match i {
                &INTEGER_TOKEN => self.read_integer().await,
                &LIST_TOKEN => Box::pin(self.read_list()).await,
                &DICT_TOKEN => Box::pin(self.read_dict()).await,
                b'0'..=b'9' => self.read_bytes().await,
                _ => Err(DecodeError::DECODER("Unknown token")),
            },
            None => Err(DecodeError::DECODER("Expected token")),
        }
    }

    pub async fn read_integer(&mut self) -> Result<Value, DecodeError> {
        self.reader.consume(1);
        let mut ret = Vec::new();
        self.reader
            .read_until(END_TOKEN, &mut ret)
            .await
            .map_err(DecodeError::IO)?;
        let int_str = String::from_utf8_lossy(&ret);
        let parsed_int = &int_str[..int_str.len() - 1]
            .parse::<i64>()
            .map_err(|_| DecodeError::DECODER("parse integer failed"))?;
        Ok(Value::Integer(*parsed_int))
    }

    pub async fn read_bytes(&mut self) -> Result<Value, DecodeError> {
        let mut buf = Vec::new();
        self.reader
            .read_until(DELIM_TOKEN, &mut buf)
            .await
            .map_err(DecodeError::IO)?;
        let length_str = String::from_utf8_lossy(&buf);
        let length = length_str[..length_str.len() - 1]
            .parse::<usize>()
            .map_err(|_| DecodeError::DECODER("parse size failed"))?;
        let mut bytes = vec![0; length];
        self.reader
            .read_exact(&mut bytes)
            .await
            .map_err(DecodeError::IO)?;
        Ok(Value::Bytes(bytes))
    }

    pub async fn read_list(&mut self) -> Result<Value, DecodeError> {
        self.reader.consume(1);
        let mut list = Vec::new();
        while self
            .reader
            .fill_buf()
            .await
            .map_err(DecodeError::IO)?
            .first()
            != Some(&END_TOKEN)
        {
            list.push(Box::pin(self.read_anything()).await?);
        }
        self.reader.consume(1);
        Ok(Value::List(list))
    }

    pub async fn read_dict(&mut self) -> Result<Value, DecodeError> {
        self.reader.consume(1);
        let mut dict = BTreeMap::new();
        while self
            .reader
            .fill_buf()
            .await
            .map_err(DecodeError::IO)?
            .first()
            != Some(&END_TOKEN)
        {
            let key = Box::pin(self.read_anything()).await?;
            let value = Box::pin(self.read_anything()).await?;
            if let Value::Bytes(_) = key {
                dict.insert(key, value);
            } else {
                return Err(DecodeError::DECODER("Dict key must be a byte string"));
            }
        }
        self.reader.consume(1);
        Ok(Value::Dict(dict))
    }
}

pub struct Encoder<'a, W: AsyncWrite + Unpin> {
    writer: &'a mut W,
}

impl<'a, W: AsyncWrite + Unpin> Encoder<'a, W> {
    pub fn new(writer: &'a mut W) -> Self {
        Self { writer }
    }

    pub async fn write_anything(&mut self, value: &Value) -> Result<(), io::Error> {
        match value {
            Value::Bytes(ref b) => self.write_bytes(b).await,
            Value::Integer(ref i) => self.write_integer(i).await,
            Value::List(ref l) => Box::pin(self.write_list(l)).await,
            Value::Dict(ref d) => Box::pin(self.write_dict(d)).await,
        }
    }

    pub async fn write_bytes(&mut self, value: &Vec<u8>) -> Result<(), io::Error> {
        self.writer
            .write_all(value.len().to_string().as_bytes())
            .await?;
        self.writer.write_all(&[DELIM_TOKEN]).await?;
        self.writer.write_all(value).await?;
        Ok(())
    }

    pub async fn write_integer(&mut self, value: &i64) -> Result<(), io::Error> {
        self.writer.write_all(&[INTEGER_TOKEN]).await?;
        self.writer.write_all(value.to_string().as_bytes()).await?;
        self.writer.write_all(&[END_TOKEN]).await?;
        Ok(())
    }

    pub async fn write_list(&mut self, value: &Vec<Value>) -> Result<(), io::Error> {
        self.writer.write_all(&[LIST_TOKEN]).await?;
        for v in value.iter() {
            self.write_anything(v).await?;
        }
        self.writer.write_all(&[END_TOKEN]).await?;
        Ok(())
    }

    pub async fn write_dict(&mut self, value: &BTreeMap<Value, Value>) -> Result<(), io::Error> {
        self.writer.write_all(&[DICT_TOKEN]).await?;
        for (k, v) in value {
            self.write_anything(k).await?;
            self.write_anything(v).await?;
        }
        self.writer.write_all(&[END_TOKEN]).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tokio::io::{BufReader, BufWriter};

    #[tokio::test]
    async fn encode_bytes() {
        let input = Value::Bytes("hello world".into());
        let mut buf = Vec::new();
        let mut writer = BufWriter::new(&mut buf);
        let got = input.encode(&mut writer).await;
        assert!(got.is_ok());

        writer.flush().await;
        assert_eq!(buf, b"11:hello world");
    }

    #[tokio::test]
    async fn encode_integer() {
        let input = Value::Integer(1234);
        let mut buf = Vec::new();
        let mut writer = BufWriter::new(&mut buf);
        let got = input.encode(&mut writer).await;
        assert!(got.is_ok());

        writer.flush().await;
        assert_eq!(buf, b"i1234e");
    }

    #[tokio::test]
    async fn encode_list() {
        let input = Value::List(vec![
            Value::Bytes("hello world".into()),
            Value::Integer(1234),
        ]);
        let mut buf = Vec::new();
        let mut writer = BufWriter::new(&mut buf);
        let got = input.encode(&mut writer).await;
        assert!(got.is_ok());

        writer.flush().await;
        assert_eq!(buf, b"l11:hello worldi1234ee");
    }

    #[tokio::test]
    async fn encode_dict() {
        let input = Value::Dict(BTreeMap::from([
            (Value::Bytes("key1".into()), Value::Bytes("value1".into())),
            (Value::Bytes("key2".into()), Value::Integer(1234)),
        ]));
        let mut buf = Vec::new();
        let mut writer = BufWriter::new(&mut buf);
        let got = input.encode(&mut writer).await;
        assert!(got.is_ok());

        writer.flush().await;
        assert_eq!(buf, b"d4:key16:value14:key2i1234ee");
    }

    #[tokio::test]
    async fn decode_integer() {
        let buf = b"i1234e";
        let cursor = Cursor::new(buf);
        let mut reader = BufReader::new(cursor);
        let got = Value::decode(&mut reader).await;
        assert_eq!(got.unwrap(), Value::Integer(1234));
    }

    #[tokio::test]
    async fn decode_bytes() {
        let buf = b"11:hello world";
        let cursor = Cursor::new(buf);
        let mut reader = BufReader::new(cursor);
        let got = Value::decode(&mut reader).await;
        assert_eq!(got.unwrap(), Value::Bytes(b"hello world".into()));
    }

    #[tokio::test]
    async fn decode_list() {
        let buf = b"l11:hello worlde";
        let cursor = Cursor::new(buf);
        let mut reader = BufReader::new(cursor);
        let got = Value::decode(&mut reader).await;
        assert_eq!(
            got.unwrap(),
            Value::List(vec![Value::Bytes(b"hello world".into())])
        );
    }

    #[tokio::test]
    async fn decode_dict() {
        let buf = b"d4:key16:value14:key26:value2e";
        let cursor = Cursor::new(buf);
        let mut reader = BufReader::new(cursor);
        let got = Value::decode(&mut reader).await;
        assert_eq!(
            got.unwrap(),
            Value::Dict(BTreeMap::from([
                (Value::Bytes("key1".into()), Value::Bytes("value1".into())),
                (Value::Bytes("key2".into()), Value::Bytes("value2".into())),
            ]))
        );
    }
}
