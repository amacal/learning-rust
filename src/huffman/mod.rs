mod header;
mod iterative;

pub use crate::huffman::header::{HuffmanDecoder, HuffmanCode, HuffmanResult, HuffmanError};
pub use crate::huffman::iterative::HuffmanTableIterative;