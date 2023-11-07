mod header;
mod bytewise;
mod bitwise;

pub use crate::bitstream::header::{BitStream, BitStreamError};

pub use crate::bitstream::bytewise::BitStreamBytewise;
pub use crate::bitstream::bitwise::BitStreamBitwise;