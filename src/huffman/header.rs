use std::fmt::Display;

use crate::bitstream::BitReader;

#[derive(Debug, PartialEq, Default, Clone, Copy)]
pub struct HuffmanCode {
    pub bits: u16,
    pub length: usize,
}

pub trait HuffmanDecoder {
    fn decode(&self, bits: &mut impl BitReader) -> HuffmanResult<u16>;
}

pub type HuffmanResult<T> = Result<T, HuffmanError>;

#[derive(PartialEq, Debug, thiserror::Error)]
pub enum HuffmanError {
    #[error("Not enough data")]
    NotEnoughData,

    #[error("Invalid symbol")]
    InvalidSymbol,
}

impl HuffmanError {
    pub fn raise_not_enough_data<T>() -> HuffmanResult<T> {
        Err(HuffmanError::NotEnoughData)
    }

    pub fn raise_invalid_symbol<T>() -> HuffmanResult<T> {
        Err(HuffmanError::InvalidSymbol)
    }
}

impl HuffmanCode {
    pub fn new(bits: u16, length: usize) -> Self {
        Self {
            bits: bits,
            length: length,
        }
    }
}

impl Display for HuffmanCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:0width$b}", self.bits, width = self.length)
    }
}
