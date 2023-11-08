pub type BitStreamResult<T> = Result<T, BitStreamError>;

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum BitStreamError {
    #[error("Passed {passed} bytes, but stream can only accept {acceptable} bytes.")]
    TooMuchData { passed: usize, acceptable: usize },

    #[error("Requested {requested} number of bytes is more than available {available} bytes.")]
    NotEnoughData { requested: usize, available: usize },
}

pub trait BitReader {
    fn next_bit(&mut self) -> Option<u8>;
    fn next_bits(&mut self, count: usize) -> Option<u16>;
    fn next_bytes(&mut self, count: usize) -> BitStreamResult<Vec<u8>>;
}

pub struct BitReaderChecked<'a, T>(&'a mut T) where T: BitStream;
pub struct BitReaderUnchecked<'a, T>(&'a mut T) where T: BitStream;

impl<'a, T> BitReader for BitReaderChecked<'a, T> where T: BitStream {
    #[inline(always)]
    fn next_bit(&mut self) -> Option<u8> {
        self.0.next_bit()
    }

    #[inline(always)]
    fn next_bits(&mut self, count: usize) -> Option<u16> {
        self.0.next_bits(count)
    }

    #[inline(always)]
    fn next_bytes(&mut self, count: usize) -> BitStreamResult<Vec<u8>> {
        self.0.next_bytes(count)
    }
}

impl<'a, T> BitReader for BitReaderUnchecked<'a, T> where T: BitStream {
    #[inline(always)]
    fn next_bit(&mut self) -> Option<u8> {
        Some(self.0.next_bit_unchecked())
    }

    #[inline(always)]
    fn next_bits(&mut self, count: usize) -> Option<u16> {
        Some(self.0.next_bits_unchecked(count))
    }

    #[inline(always)]
    fn next_bytes(&mut self, count: usize) -> BitStreamResult<Vec<u8>> {
        self.0.next_bytes(count)
    }
}

pub trait BitStreamExt: BitStream + Sized {
    fn as_checked(&mut self) -> BitReaderChecked<Self> {
        BitReaderChecked(self)
    }

    fn as_unchecked(&mut self) -> BitReaderUnchecked<Self> {
        BitReaderUnchecked(self)
    }
}

impl<T: BitStream> BitStreamExt for T {}

pub trait BitStream {
    fn available(&self) -> usize;

    fn appendable(&self) -> Option<usize>;
    fn append(&mut self, data: &[u8]) -> BitStreamResult<()>;

    fn next_bit(&mut self) -> Option<u8>;
    fn next_bit_unchecked(&mut self) -> u8;

    fn next_bits(&mut self, count: usize) -> Option<u16>;
    fn next_bits_unchecked(&mut self, count: usize) -> u16;

    fn next_bytes(&mut self, count: usize) -> BitStreamResult<Vec<u8>>;
}


impl BitStreamError {
    pub fn raise_too_much_data<T>(passed: usize, acceptable: usize) -> BitStreamResult<T> {
        Err(BitStreamError::TooMuchData {
            passed: passed,
            acceptable: acceptable,
        })
    }

    pub fn raise_not_enough_data<T>(requested: usize, available: usize) -> BitStreamResult<T> {
        Err(BitStreamError::NotEnoughData {
            requested: requested,
            available: available,
        })
    }
}
