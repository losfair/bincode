use alloc::{Vec};
use alloc::boxed::Box;

use core_io as io;
use error::Result;
use serde;

/// An optional Read trait for advanced Bincode usage.
///
/// It is highly recommended to use bincode with `io::Read` or `&[u8]` before
/// implementing a custom `BincodeRead`.
pub trait BincodeRead<'storage>: io::Read {
    /// Forwards reading `length` bytes of a string on to the serde reader.
    fn forward_read_str<V>(&mut self, length: usize, visitor: V) -> Result<V::Value>
    where
        V: serde::de::Visitor<'storage>;

    /// Return the first `length` bytes of the internal byte buffer.
    fn get_byte_buffer(&mut self, length: usize) -> Result<Vec<u8>>;

    /// Forwards reading `length` bytes on to the serde reader.
    fn forward_read_bytes<V>(&mut self, length: usize, visitor: V) -> Result<V::Value>
    where
        V: serde::de::Visitor<'storage>;
}

/// A BincodeRead implementation for byte slices
/// NOT A PART OF THE STABLE PUBLIC API
#[doc(hidden)]
pub struct SliceReader<'storage> {
    slice: &'storage [u8],
}

/// A BincodeRead implementation for io::Readers
/// NOT A PART OF THE STABLE PUBLIC API
#[doc(hidden)]
pub struct IoReader<R> {
    reader: R,
    temp_buffer: Vec<u8>,
}

impl<'storage> SliceReader<'storage> {
    /// Constructs a slice reader
    pub fn new(bytes: &'storage [u8]) -> SliceReader<'storage> {
        SliceReader { slice: bytes }
    }
}

impl<R> IoReader<R> {
    /// Constructs an IoReadReader
    pub fn new(r: R) -> IoReader<R> {
        IoReader {
            reader: r,
            temp_buffer: vec![],
        }
    }
}

impl<'storage> io::Read for SliceReader<'storage> {
    #[inline(always)]
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        (&mut self.slice).read(out)
    }
    #[inline(always)]
    fn read_exact(&mut self, out: &mut [u8]) -> io::Result<()> {
        (&mut self.slice).read_exact(out)
    }
}

impl<R: io::Read> io::Read for IoReader<R> {
    #[inline(always)]
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        self.reader.read(out)
    }
    #[inline(always)]
    fn read_exact(&mut self, out: &mut [u8]) -> io::Result<()> {
        self.reader.read_exact(out)
    }
}

impl<'storage> SliceReader<'storage> {
    #[inline(always)]
    fn unexpected_eof() -> Box<::ErrorKind> {
        return Box::new(::ErrorKind::Io(
            io::Error::new(io::ErrorKind::UnexpectedEof, ""),
        ));
    }
}

impl<'storage> BincodeRead<'storage> for SliceReader<'storage> {
    #[inline(always)]
    fn forward_read_str<V>(&mut self, length: usize, visitor: V) -> Result<V::Value>
    where
        V: serde::de::Visitor<'storage>,
    {
        use ErrorKind;
        if length > self.slice.len() {
            return Err(SliceReader::unexpected_eof());
        }

        let string = match ::core::str::from_utf8(&self.slice[..length]) {
            Ok(s) => s,
            Err(e) => return Err(ErrorKind::InvalidUtf8Encoding(e).into()),
        };
        let r = visitor.visit_borrowed_str(string);
        self.slice = &self.slice[length..];
        r
    }

    #[inline(always)]
    fn get_byte_buffer(&mut self, length: usize) -> Result<Vec<u8>> {
        if length > self.slice.len() {
            return Err(SliceReader::unexpected_eof());
        }

        let r = &self.slice[..length];
        self.slice = &self.slice[length..];
        Ok(r.to_vec())
    }

    #[inline(always)]
    fn forward_read_bytes<V>(&mut self, length: usize, visitor: V) -> Result<V::Value>
    where
        V: serde::de::Visitor<'storage>,
    {
        if length > self.slice.len() {
            return Err(SliceReader::unexpected_eof());
        }

        let r = visitor.visit_borrowed_bytes(&self.slice[..length]);
        self.slice = &self.slice[length..];
        r
    }
}

impl<R> IoReader<R>
where
    R: io::Read,
{
    fn fill_buffer(&mut self, length: usize) -> Result<()> {
        let current_length = self.temp_buffer.len();
        if length > current_length {
            self.temp_buffer.reserve_exact(length - current_length);
        }

        unsafe {
            self.temp_buffer.set_len(length);
        }

        self.reader.read_exact(&mut self.temp_buffer)?;
        Ok(())
    }
}

impl<R> BincodeRead<'static> for IoReader<R>
where
    R: io::Read,
{
    fn forward_read_str<V>(&mut self, length: usize, visitor: V) -> Result<V::Value>
    where
        V: serde::de::Visitor<'static>,
    {
        self.fill_buffer(length)?;

        let string = match ::core::str::from_utf8(&self.temp_buffer[..]) {
            Ok(s) => s,
            Err(e) => return Err(::ErrorKind::InvalidUtf8Encoding(e).into()),
        };

        let r = visitor.visit_str(string);
        r
    }

    fn get_byte_buffer(&mut self, length: usize) -> Result<Vec<u8>> {
        self.fill_buffer(length)?;
        Ok(::core::mem::replace(&mut self.temp_buffer, Vec::new()))
    }

    fn forward_read_bytes<V>(&mut self, length: usize, visitor: V) -> Result<V::Value>
    where
        V: serde::de::Visitor<'static>,
    {
        self.fill_buffer(length)?;
        let r = visitor.visit_bytes(&self.temp_buffer[..]);
        r
    }
}
