// use crate::bencode::Value;
// use serde::{ser, Serialize};
// use std::io::{self, Error};

// pub struct Serializer<W: io::Write> {}

// pub fn write<T, W: io::Write>(value: &T, writer: W) -> Result<(), Error>
// where
//     T: Serialize,
// {
//     let mut serializer = Serializer { writer };
//     value.serialize(&mut serializer)
// }

// impl<'a> ser::Serializer for &'a mut Serializer {
//     type Ok = Value;
// }
