use crate::bitstream::{BitStream, BitStreamError};
use crate::inflate::{InflateBlockInfo, InflateError, InflateEvent, InflateReader, InflateResult};

pub struct ZlibReader<const T: usize> {
    inflate: InflateReader,
    bitstream: BitStream<T>,
    verified: Option<u32>,
    exhausted: bool,
}

#[derive(Debug)]
pub enum ZlibEvent {
    Inflate(InflateEvent),
    Checksum(u32),
}

type ZlibResult<T> = Result<T, ZlibError>;

#[derive(Debug, thiserror::Error)]
pub enum ZlibError {
    #[error("Not Enough Data: {0}")]
    NotEnoughData(String),

    #[error("Not Implemented: {0}")]
    NotImplemented(String),

    #[error("Bitstreaming failed: {0}")]
    BitStream(BitStreamError),

    #[error("Inflating failed: {0}")]
    Inflate(InflateError),
}

fn raise_not_enough_data<T>(description: &str) -> ZlibResult<T> {
    Err(ZlibError::NotEnoughData(description.to_string()))
}

fn raise_not_implemented<T>(description: &str) -> ZlibResult<T> {
    Err(ZlibError::NotImplemented(description.to_string()))
}

fn raise_bitstream_error<T>(error: BitStreamError) -> ZlibResult<T> {
    Err(ZlibError::BitStream(error))
}

fn raise_inflate_error<T>(error: InflateError) -> ZlibResult<T> {
    Err(ZlibError::Inflate(error))
}

impl<const T: usize> ZlibReader<T> {
    pub fn open(data: &[u8]) -> ZlibResult<Self> {
        if data.len() < 2 {
            return raise_not_enough_data("zlib archive needs at least two bytes");
        }

        let compression_method = data[0] & 0x0f;
        if compression_method != 8 {
            return raise_not_implemented(format!("only deflate, compression method {}", compression_method).as_str());
        }

        let _compression_info = (data[0] & 0xf0) >> 4;
        let _check_bits = data[1] & 0x1f;

        let preset_dictionary = (data[1] & 0x20) >> 5;
        if preset_dictionary == 1 {
            return raise_not_implemented("preset dictionary support is not available");
        }

        let _compression_level = (data[1] & 0x60) >> 6;
        let mut bitstream: BitStream<T> = BitStream::new();

        if let Err(error) = bitstream.append(&data[2..]) {
            return raise_bitstream_error(error);
        }

        Ok(Self {
            verified: None,
            exhausted: false,
            bitstream: bitstream,
            inflate: InflateReader::new(),
        })
    }

    pub fn next(&mut self) -> ZlibResult<ZlibEvent> {
        if self.verified.is_none() && self.inflate.is_completed() {
            let bytes = match self.bitstream.next_bytes(4) {
                Ok(value) => value,
                Err(error) => return raise_bitstream_error(error),
            };

            let mut checksum = 0;
            for i in 0..bytes.len() {
                checksum <<= 8;
                checksum |= bytes[i] as u32;
            }

            self.verified = Some(checksum);
            return Ok(ZlibEvent::Checksum(checksum));
        }

        match self.inflate.next(&mut self.bitstream) {
            Ok(event) => Ok(ZlibEvent::Inflate(event)),
            Err(error) => raise_inflate_error(error),
        }
    }

    pub fn appendable(&self) -> Option<usize> {
        if self.exhausted {
            None
        } else {
            self.bitstream.appendable()
        }
    }

    pub fn append(&mut self, data: &[u8]) -> ZlibResult<()> {
        self.bitstream.collect(None);
        self.exhausted = data.len() == 0;

        match self.bitstream.append(data) {
            Ok(()) => Ok(()),
            Err(error) => raise_bitstream_error(error),
        }
    }

    pub fn is_completed(&self) -> bool {
        self.verified.is_some() && self.inflate.is_completed()
    }

    pub fn is_broken(&self) -> bool {
        self.inflate.is_broken()
    }

    pub fn block(&self) -> InflateResult<InflateBlockInfo> {
        self.inflate.block()
    }
}
