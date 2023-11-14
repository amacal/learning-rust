mod header;
mod bounds;
mod iterative;
mod lookup;

pub use crate::huffman::header::{HuffmanDecoder, HuffmanCode, HuffmanResult, HuffmanError};

pub use crate::huffman::bounds::HuffmanTableBounds;
pub use crate::huffman::iterative::HuffmanTableIterative;
pub use crate::huffman::lookup::HuffmanTableLookup;