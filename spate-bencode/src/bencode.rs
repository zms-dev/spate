use core::fmt;
use std::{
    collections::BTreeMap,
    io::{self, BufRead, Read},
};

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

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

impl Value {
    pub fn decode<R: io::Read>(reader: &mut R) -> Result<Value, DecodeError> {
        let mut buf = io::BufReader::new(reader);
        Value::from_anything(&mut buf)
    }

    fn from_anything(reader: &mut io::BufReader<impl io::Read>) -> Result<Self, DecodeError> {
        match reader
            .fill_buf()
            .or_else(|op| Err(DecodeError::from_io(op)))?
            .get(0)
        {
            Some(i) => match i {
                &INTEGER_TOKEN => Value::from_integer(reader),
                &LIST_TOKEN => Value::from_list(reader),
                &DICT_TOKEN => Value::from_dict(reader),
                b'0'..=b'9' => Value::from_bytes(reader),
                _ => Err(DecodeError::from_custom("Unknown token")),
            },
            None => Err(DecodeError::from_custom("Expected token")),
        }
    }

    fn from_integer(reader: &mut io::BufReader<impl io::Read>) -> Result<Self, DecodeError> {
        reader.consume(1);
        let mut ret = Vec::new();
        reader
            .read_until(END_TOKEN, &mut ret)
            .or_else(|op| Err(DecodeError::from_io(op)))?;
        let int_str = String::from_utf8_lossy(&ret);
        let parsed_int = &int_str[..int_str.len() - 1]
            .parse::<i64>()
            .or_else(|_| Err(DecodeError::from_custom("parse integer failed")))?;
        Ok(Value::Integer(*parsed_int))
    }

    fn from_bytes(reader: &mut io::BufReader<impl io::Read>) -> Result<Self, DecodeError> {
        let mut buf = Vec::new();
        reader
            .read_until(DELIM_TOKEN, &mut buf)
            .or_else(|op| Err(DecodeError::from_io(op)))?;
        let length_str = String::from_utf8_lossy(&buf);
        let length = length_str[..length_str.len() - 1]
            .parse::<usize>()
            .or_else(|_| Err(DecodeError::from_custom("parse size failed")))?;
        let mut bytes = vec![0; length];
        reader
            .read_exact(&mut bytes)
            .or_else(|op| Err(DecodeError::from_io(op)))?;
        Ok(Value::Bytes(bytes))
    }

    fn from_list(reader: &mut io::BufReader<impl io::Read>) -> Result<Self, DecodeError> {
        reader.consume(1);
        let mut list = Vec::new();
        while reader
            .fill_buf()
            .or_else(|op| Err(DecodeError::from_io(op)))?
            .get(0)
            != Some(&END_TOKEN)
        {
            list.push(Value::from_anything(reader)?);
        }
        reader.consume(1);
        Ok(Value::List(list))
    }

    fn from_dict(reader: &mut io::BufReader<impl io::Read>) -> Result<Self, DecodeError> {
        reader.consume(1);
        let mut dict = BTreeMap::new();
        while reader
            .fill_buf()
            .or_else(|op| Err(DecodeError::from_io(op)))?
            .get(0)
            != Some(&END_TOKEN)
        {
            let key = Value::from_anything(reader)?;
            let value = Value::from_anything(reader)?;
            if let Value::Bytes(_) = key {
                dict.insert(key, value);
            } else {
                return Err(DecodeError::from_custom("Dict key must be a byte string"));
            }
        }
        reader.consume(1);
        Ok(Value::Dict(dict))
    }
}

impl Value {
    pub fn encode<W: io::Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            Value::Bytes(value) => Value::write_bytes(writer, value),
            Value::Integer(value) => Value::write_integer(writer, value),
            Value::List(value) => Value::write_list(writer, value),
            Value::Dict(value) => Value::write_dict(writer, value),
        }
    }

    fn write_bytes<W: io::Write>(writer: &mut W, value: &Vec<u8>) -> Result<(), io::Error> {
        writer.write_all(value.len().to_string().as_bytes())?;
        writer.write_all(&[DELIM_TOKEN])?;
        writer.write_all(&value)?;
        Ok(())
    }

    fn write_integer<W: io::Write>(writer: &mut W, value: &i64) -> Result<(), io::Error> {
        writer.write_all(&[INTEGER_TOKEN])?;
        writer.write_all(value.to_string().as_bytes())?;
        writer.write_all(&[END_TOKEN])?;
        Ok(())
    }

    fn write_list<W: io::Write>(writer: &mut W, value: &Vec<Value>) -> Result<(), io::Error> {
        writer.write_all(&[LIST_TOKEN])?;
        for v in value.iter() {
            v.encode(writer)?;
        }
        writer.write_all(&[END_TOKEN])?;
        Ok(())
    }

    fn write_dict<W: io::Write>(
        writer: &mut W,
        value: &BTreeMap<Value, Value>,
    ) -> Result<(), io::Error> {
        writer.write_all(&[DICT_TOKEN])?;
        for (k, v) in value {
            k.encode(writer)?;
            v.encode(writer)?;
        }
        writer.write_all(&[END_TOKEN])?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum DecodeErrorKind {
    IO(io::Error),
    DECODER(&'static str),
}

pub struct DecodeError {
    pub kind: DecodeErrorKind,
}

impl DecodeError {
    fn from_io(e: io::Error) -> Self {
        Self {
            kind: DecodeErrorKind::IO(e),
        }
    }

    fn from_custom(msg: &'static str) -> Self {
        Self {
            kind: DecodeErrorKind::DECODER(msg),
        }
    }
}

impl fmt::Debug for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecodeError")
            .field("kind", &self.kind)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_bytes() {
        let input = Value::Bytes("hello world".into());
        let mut buf = Vec::new();
        let got = input.encode(&mut buf);
        assert!(got.is_ok());
        assert_eq!(buf, b"11:hello world");
    }

    #[test]
    fn encode_integer() {
        let input = Value::Integer(1234);
        let mut buf = Vec::new();
        let got = input.encode(&mut buf);
        assert!(got.is_ok());
        assert_eq!(buf, b"i1234e");
    }

    #[test]
    fn encode_list() {
        let input = Value::List(vec![
            Value::Bytes("hello world".into()),
            Value::Integer(1234),
        ]);
        let mut buf = Vec::new();
        let got = input.encode(&mut buf);
        assert!(got.is_ok());
        assert_eq!(buf, b"l11:hello worldi1234ee");
    }

    #[test]
    fn encode_dict() {
        let input = Value::Dict(BTreeMap::from([
            (Value::Bytes("key1".into()), Value::Bytes("value1".into())),
            (Value::Bytes("key2".into()), Value::Integer(1234)),
        ]));
        let mut buf = Vec::new();
        let got = input.encode(&mut buf);
        assert!(got.is_ok());
        assert_eq!(buf, b"d4:key16:value14:key2i1234ee");
    }

    #[test]
    fn decode_integer() {
        let buf = b"i1234e";
        let got = Value::decode(&mut &buf[..]);
        assert_eq!(got.unwrap(), Value::Integer(1234));
    }

    #[test]
    fn decode_bytes() {
        let buf = b"11:hello world";
        let got = Value::decode(&mut &buf[..]);
        assert_eq!(got.unwrap(), Value::Bytes(b"hello world".into()));
    }

    #[test]
    fn decode_list() {
        let buf = b"l11:hello worlde";
        let got = Value::decode(&mut &buf[..]);
        assert_eq!(
            got.unwrap(),
            Value::List(vec![Value::Bytes(b"hello world".into())])
        );
    }

    #[test]
    fn decode_dict() {
        let buf = b"d4:key16:value14:key26:value2e";
        let got = Value::decode(&mut &buf[..]);
        assert_eq!(
            got.unwrap(),
            Value::Dict(BTreeMap::from([
                (Value::Bytes("key1".into()), Value::Bytes("value1".into())),
                (Value::Bytes("key2".into()), Value::Bytes("value2".into())),
            ]))
        );
    }
}
