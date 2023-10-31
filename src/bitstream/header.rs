pub type BitStreamResult<T> = Result<T, BitStreamError>;

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum BitStreamError {
    #[error("Passed {passed} bytes, but stream can only accept {acceptable} bytes.")]
    TooMuchData { passed: usize, acceptable: usize },

    #[error("Requested {requested} number of bytes is more than available {available} bytes.")]
    NotEnoughData { requested: usize, available: usize },
}

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
