mod header;
mod iterative;
mod lookup;

pub use crate::huffman::header::{HuffmanDecoder, HuffmanCode, HuffmanResult, HuffmanError};
pub use crate::huffman::iterative::HuffmanTableIterative;
pub use crate::huffman::lookup::HuffmanTableLookup;