mod header;
mod default;
mod optimized;

pub use crate::bitstream::header::{BitStream, BitStreamError, BitStreamResult};

pub use crate::bitstream::default::BitStreamDefault;
pub use crate::bitstream::optimized::BitStreamOptimized;