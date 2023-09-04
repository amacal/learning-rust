use crate::bitstream::BitStream;
use crate::huffman::{HuffmanCode, HuffmanTable};

enum InflateDecoder {
    Huffman { huffman: InflateHuffman },
    Uncompressed { uncompressed: InflateUncompressed },
}

impl InflateDecoder {
    fn huffman(literals: HuffmanTable<16, 288>, distances: HuffmanTable<16, 32>) -> Self {
        Self::Huffman {
            huffman: InflateHuffman {
                literals: literals,
                distances: distances,
            },
        }
    }
}

struct InflateHuffman {
    literals: HuffmanTable<16, 288>,
    distances: HuffmanTable<16, 32>,
}

struct InflateUncompressed {}

struct InflateBlock {
    last: bool,
    mode: u8,
    decoder: InflateDecoder,
}

pub struct InflateBlockInfo {
    pub last: bool,
    pub mode: String,
    pub decoder: String,
    pub literals: Option<[HuffmanCode; 288]>,
    pub distances: Option<[HuffmanCode; 32]>,
}

pub struct InflateReader {
    offset: u32,
    current: Option<InflateBlock>,
    buffer: Option<u32>,
    completed: bool,
    failed: bool,
}

pub struct InflateWriter<const T: usize> {
    buffer: Box<[u8; T]>,
    offset: usize,
}

pub enum InflateSymbol {
    EndBlock,
    Literal { value: u8 },
    Match { length: u16, distance: u16 },
    Uncompressed { data: Vec<u8> },
}

pub enum InflateEvent {
    BlockStarted(u32),
    BlockEnded(u32),
    SymbolDecoded(InflateSymbol),
}

#[derive(Debug, thiserror::Error)]
pub enum InflateError {
    #[error("Not Enough Data: {0}")]
    NotEnoughData(String),

    #[error("Invalid Table: {0} / {1}")]
    InvalidTable(String, String),

    #[error("Invalid State: {0}")]
    InvalidState(String),

    #[error("Not Implemented Protocol: {0}")]
    NotImplementedProtocol(String),

    #[error("End of Stream")]
    EndOfStream,
}

pub type InflateResult<T> = Result<T, InflateError>;

fn raise_not_enough_data<T>(description: &str) -> InflateResult<T> {
    Err(InflateError::NotEnoughData(description.to_string()))
}

fn raise_invalid_table<T>(table_name: &str, description: String) -> InflateResult<T> {
    Err(InflateError::InvalidTable(table_name.to_string(), description))
}

fn raise_invalid_state<T>(description: &str) -> InflateResult<T> {
    Err(InflateError::InvalidState(description.to_string()))
}

fn raise_not_implemented_protocol<T>(description: &str) -> InflateResult<T> {
    Err(InflateError::NotImplementedProtocol(description.to_string()))
}

fn raise_end_of_stream<T>() -> InflateResult<T> {
    Err(InflateError::EndOfStream)
}

fn build_fixed_tables() -> (HuffmanTable<16, 288>, HuffmanTable<16, 32>) {
    let mut literals = [0; 288];
    let mut distances = [0; 32];

    for i in 0..144 {
        literals[i] = 8;
    }

    for i in 144..256 {
        literals[i] = 9;
    }

    for i in 256..280 {
        literals[i] = 7;
    }

    for i in 280..288 {
        literals[i] = 8;
    }

    for i in 0..distances.len() {
        distances[i] = 5;
    }

    (HuffmanTable::new(literals), HuffmanTable::new(distances))
}

fn build_length_table<const T: usize>(hlen: usize, reader: &mut BitStream<T>) -> Option<HuffmanTable<8, 19>> {
    let mut lengths: [u16; 19] = [0; 19];
    let mapping = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

    for i in 0..hlen {
        lengths[mapping[i]] = match reader.next_bits(3) {
            Some(bits) => bits as u16,
            None => return None,
        };
    }

    Some(HuffmanTable::new(lengths))
}

fn build_dynamic_table<const T: usize, const B: usize>(
    bitstream: &mut BitStream<B>,
    lengths: &HuffmanTable<8, 19>,
    count: usize,
) -> InflateResult<HuffmanTable<16, T>> {
    let mut index: usize = 0;
    let mut symbols = [0; T];

    while index < count {
        let value = match lengths.decode(bitstream) {
            Some(value) => value,
            None => return raise_not_enough_data("decoding value"),
        };

        let (repeat, times) = match value {
            0..=15 => (0, 0),
            16 => match bitstream.next_bits(2) {
                Some(value) => (symbols[index - 1], 3 + value),
                None => return raise_not_enough_data("reading zeros"),
            },
            17 => match bitstream.next_bits(3) {
                Some(value) => (0, 3 + value),
                None => return raise_not_enough_data("reading zeros"),
            },
            18 => match bitstream.next_bits(7) {
                Some(value) => (0, 11 + value),
                None => return raise_not_enough_data("reading zeros"),
            },
            value => return raise_not_implemented_protocol(format!("symbol {} in the alphabet", value).as_str()),
        };

        if times == 0 {
            symbols[index] = value;
            index += 1;
        }

        for _ in 0..times {
            symbols[index] = repeat;
            index += 1;
        }
    }

    Ok(HuffmanTable::new(symbols))
}

fn build_dynamic_tables<const T: usize>(
    bitstream: &mut BitStream<T>,
) -> InflateResult<(HuffmanTable<16, 288>, HuffmanTable<16, 32>)> {
    let hlit = match bitstream.next_bits(5) {
        Some(bits) => 257 + bits as usize,
        None => return raise_not_enough_data("hlit value"),
    };

    let hdist = match bitstream.next_bits(5) {
        Some(bits) => 1 + bits as usize,
        None => return raise_not_enough_data("hdist value"),
    };

    let hclen = match bitstream.next_bits(4) {
        Some(bits) => 4 + bits as usize,
        None => return raise_not_enough_data("hclen value"),
    };

    let lengths = match build_length_table(hclen, bitstream) {
        Some(lengths) => lengths,
        None => return raise_not_enough_data("building lengths table"),
    };

    let literals = match build_dynamic_table(bitstream, &lengths, hlit) {
        Ok(table) => table,
        Err(error) => return raise_invalid_table("literals", error.to_string()),
    };

    let distances = match build_dynamic_table(bitstream, &lengths, hdist) {
        Ok(table) => table,
        Err(error) => return raise_invalid_table("distances", error.to_string()),
    };

    Ok((literals, distances))
}

impl InflateBlock {
    pub fn open<const T: usize>(reader: &mut BitStream<T>) -> InflateResult<Self> {
        let last = match reader.next_bit() {
            Some(bit) => bit == 1,
            None => return raise_not_enough_data("last_block bit"),
        };

        let mode = match reader.next_bits(2) {
            Some(bits) => bits as u8,
            None => return raise_not_enough_data("mode bit"),
        };

        let decoder = match mode {
            0 => InflateDecoder::Uncompressed {
                uncompressed: InflateUncompressed::new(),
            },
            1 => {
                let (literals, distances) = build_fixed_tables();
                InflateDecoder::huffman(literals, distances)
            }
            2 => match build_dynamic_tables(reader) {
                Ok((literals, distances)) => InflateDecoder::huffman(literals, distances),
                Err(error) => return Err(error),
            },
            _ => return raise_not_implemented_protocol(&format!("unknown mode {}", mode)),
        };

        Ok(Self {
            last: last,
            mode: mode,
            decoder: decoder,
        })
    }

    pub fn next<const T: usize>(&mut self, reader: &mut BitStream<T>) -> InflateResult<InflateSymbol> {
        match &self.decoder {
            InflateDecoder::Huffman { huffman } => huffman.next(reader),
            InflateDecoder::Uncompressed { uncompressed } => uncompressed.next(reader),
        }
    }
}

impl InflateReader {
    pub fn zlib<const T: usize>(bitstream: &mut BitStream<T>) -> InflateResult<Self> {
        let compression_method = match bitstream.next_bits(4) {
            Some(value) => value,
            None => return raise_not_enough_data("zlib, 1st byte, compression method"),
        };

        if compression_method != 8 {
            return raise_not_implemented_protocol(
                format!("only deflate, compression method {}", compression_method).as_str(),
            );
        }

        let _compression_info = match bitstream.next_bits(4) {
            Some(value) => value,
            None => return raise_not_enough_data("zlib, 1st byte, compression info"),
        };

        if let None = bitstream.next_bits(5) {
            return raise_not_enough_data("zlib, 2nd byte, check bits");
        }

        let preset_dictionary = match bitstream.next_bit() {
            Some(value) => value,
            None => return raise_not_enough_data("zlib, 2nd byte, preset dictionary"),
        };

        if preset_dictionary == 1 {
            return raise_not_implemented_protocol("preset dictionary is present");
        }

        let _compression_level = match bitstream.next_bits(2){
            Some(value) => value,
            None => return raise_not_enough_data("zlib, 2nd byte, compression level"),
        };

        Ok(Self {
            offset: 0,
            current: None,
            buffer: None,
            completed: false,
            failed: false,
        })
    }

    pub fn block(&self) -> InflateResult<InflateBlockInfo> {
        let current = match &self.current {
            Some(block) => block,
            None => return raise_invalid_state("non-active block"),
        };

        let (literals, distances) = match &current.decoder {
            InflateDecoder::Huffman { huffman } => (Some(huffman.literals.list()), Some(huffman.distances.list())),
            InflateDecoder::Uncompressed { uncompressed: _ } => (None, None),
        };

        Ok(InflateBlockInfo {
            last: current.last,
            mode: match current.mode {
                0 => "uncompressed".to_string(),
                1 => "fixed".to_string(),
                2 => "dynamic".to_string(),
                _ => "unknown".to_string(),
            },
            decoder: match &current.decoder {
                InflateDecoder::Huffman { huffman: _ } => "huffman".to_string(),
                InflateDecoder::Uncompressed { uncompressed: _ } => "uncompressed".to_string(),
            },
            literals: literals,
            distances: distances,
        })
    }

    pub fn is_completed(&self) -> bool {
        self.completed
    }

    pub fn is_broken(&self) -> bool {
        self.failed
    }

    pub fn next<const T: usize>(&mut self, bitstream: &mut BitStream<T>) -> InflateResult<InflateEvent> {
        if let Some(offset) = self.buffer {
            self.buffer = None;
            self.offset += 1;

            return Ok(InflateEvent::BlockEnded(offset));
        }

        if self.completed {
            return raise_end_of_stream();
        }

        if let None = self.current {
            let block = match InflateBlock::open(bitstream) {
                Ok(block) => block,
                Err(error) => return Err(error),
            };

            self.current = Some(block);
            return Ok(InflateEvent::BlockStarted(self.offset));
        }

        let current = match &mut self.current {
            Some(value) => value,
            None => return raise_invalid_state("missing current block"),
        };

        let symbol = match current.next(bitstream) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };

        if let InflateSymbol::EndBlock | InflateSymbol::Uncompressed { data: _ } = symbol {
            self.buffer = Some(self.offset);
            self.completed = current.last;
            self.current = None;
        }

        Ok(InflateEvent::SymbolDecoded(symbol))
    }
}

impl<const T: usize> InflateWriter<T> {
    pub fn new() -> Self {
        Self {
            offset: 0,
            buffer: Box::new([0; T]),
        }
    }

    pub fn handle(&mut self, symbol: InflateSymbol) -> Option<usize> {
        match symbol {
            InflateSymbol::Literal { value } => {
                self.buffer[self.offset] = value;
                self.offset += 1;
            }
            InflateSymbol::Match { length, distance } if length <= distance => {
                let source = self.offset - distance as usize..self.offset;
                self.buffer.copy_within(source, self.offset);
                self.offset += length as usize;
            }
            InflateSymbol::Match { length, distance } => {
                let mut length = length as usize;
                let mut distance = distance as usize;

                while length > 0 {
                    let available = std::cmp::min(distance, length);
                    let source = self.offset - distance..self.offset - distance + available;

                    self.buffer.copy_within(source, self.offset);
                    self.offset += available;

                    length -= available;
                    distance += available;
                }
            }
            InflateSymbol::Uncompressed { data } => {
                self.buffer[self.offset..self.offset + data.len()].copy_from_slice(&data[..]);
                self.offset += data.len();
            }
            InflateSymbol::EndBlock => {}
        };

        if self.offset >= 65_536 {
            Some(self.offset - 32_768)
        } else {
            None
        }
    }

    pub fn take(&mut self, buffer: &mut [u8]) -> usize {
        let available = std::cmp::min(buffer.len(), self.offset);
        buffer[..available].copy_from_slice(&self.buffer[..available]);

        self.buffer.copy_within(available..self.offset, 0);
        self.offset -= available;

        available
    }
}

impl InflateHuffman {
    fn decode_length(value: u16, extra: u16) -> InflateResult<u16> {
        match value {
            257..=264 => Ok((value - 257) + 3 + extra),
            265..=268 => Ok((value - 265) * 2 + 11 + extra),
            269..=272 => Ok((value - 269) * 4 + 19 + extra),
            273..=276 => Ok((value - 273) * 8 + 35 + extra),
            277..=280 => Ok((value - 277) * 16 + 67 + extra),
            281..=284 => Ok((value - 281) * 32 + 131 + extra),
            285 => Ok(258),
            _ => raise_invalid_state(format!("decoding length symbol '{}'", value).as_str()),
        }
    }

    fn decode_distance(value: u16, extra: u16) -> InflateResult<u16> {
        match value {
            0..=3 => Ok(value + 1),
            4..=5 => Ok((value - 4) * 2 + 5 + extra),
            6..=7 => Ok((value - 6) * 4 + 9 + extra),
            8..=9 => Ok((value - 8) * 8 + 17 + extra),
            10..=11 => Ok((value - 10) * 16 + 33 + extra),
            12..=13 => Ok((value - 12) * 32 + 65 + extra),
            14..=15 => Ok((value - 14) * 64 + 129 + extra),
            16..=17 => Ok((value - 16) * 128 + 257 + extra),
            18..=19 => Ok((value - 18) * 256 + 513 + extra),
            20..=21 => Ok((value - 20) * 512 + 1025 + extra),
            22..=23 => Ok((value - 22) * 1024 + 2049 + extra),
            24..=25 => Ok((value - 24) * 2048 + 4097 + extra),
            26..=27 => Ok((value - 26) * 4096 + 8193 + extra),
            28..=29 => Ok((value - 28) * 8192 + 16385 + extra),
            _ => raise_invalid_state(format!("decoding distance symbol '{}'", value).as_str()),
        }
    }

    fn decode<const T: usize>(&self, bitstream: &mut BitStream<T>, length: u16) -> InflateResult<InflateSymbol> {
        let length_bits = (std::cmp::max(length, 261) as usize - 261) / 4;
        let length_bits = if length_bits == 6 { 0 } else { length_bits };

        let length_extra = match bitstream.next_bits(length_bits) {
            Some(bits) => bits as u16,
            None => return raise_not_enough_data("literal bits"),
        };

        let distance = match self.distances.decode(bitstream) {
            Some(value) => value,
            None => return raise_invalid_state("decoding distances"),
        };

        let distance_bits = if distance < 4 { 0 } else { (distance as usize - 2) / 2 };
        let distance_extra = match bitstream.next_bits(distance_bits) {
            Some(value) => value as u16,
            None => return raise_not_enough_data("distance bits"),
        };

        let length = Self::decode_length(length, length_extra)?;
        let distance = Self::decode_distance(distance, distance_extra)?;

        Ok(InflateSymbol::Match {
            length: length,
            distance: distance,
        })
    }

    fn next<const T: usize>(&self, bitstream: &mut BitStream<T>) -> InflateResult<InflateSymbol> {
        let literal = match self.literals.decode(bitstream) {
            Some(value) => value,
            None => return raise_invalid_state("decoding literals"),
        };

        match literal {
            256 => Ok(InflateSymbol::EndBlock),
            0..=255 => Ok(InflateSymbol::Literal { value: literal as u8 }),
            value => self.decode(bitstream, value),
        }
    }
}

impl InflateUncompressed {
    fn new() -> Self {
        Self {}
    }

    fn next<const T: usize>(&self, bitstream: &mut BitStream<T>) -> InflateResult<InflateSymbol> {
        if let None = bitstream.align_to_next_byte() {
            return raise_not_enough_data("aligning to next byte");
        }

        let len = match bitstream.next_bits(16) {
            Some(value) => value,
            None => return raise_not_enough_data("len bytes"),
        };

        let nlen = match bitstream.next_bits(16) {
            Some(value) => value,
            None => return raise_not_enough_data("nlen bytes"),
        };

        if !((len ^ 0xFFFF) == nlen) {
            return raise_invalid_state("nlen doesn't match len");
        }

        let symbol = match bitstream.next_bytes(len as usize) {
            Some(data) => InflateSymbol::Uncompressed { data: data },
            None => return raise_not_enough_data(format!("requested {} bytes", len).as_str()),
        };

        Ok(symbol)
    }
}
