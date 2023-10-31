use crate::adler32::Adler32;
use crate::bitstream::BitStream;
use crate::huffman::{HuffmanCode, HuffmanError, HuffmanTable};

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
    hash: Adler32,
    buffer: Box<[u8; T]>,
    offset: usize,
}

#[derive(Debug)]
pub enum InflateSymbol {
    EndBlock,
    Literal { value: u8 },
    Match { length: u16, distance: u16 },
    Uncompressed { data: Vec<u8> },
}

#[derive(Debug)]
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

    #[error("Decoding failed in '{0}': {1}")]
    DecodingFailed(String, HuffmanError),

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

fn raise_decoding_failed<T>(table_name: &str, error: HuffmanError) -> InflateResult<T> {
    Err(InflateError::DecodingFailed(table_name.to_string(), error))
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

fn build_fixed_tables() -> InflateResult<(HuffmanTable<16, 288>, HuffmanTable<16, 32>)> {
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

    let literals = match HuffmanTable::new(literals) {
        Some(table) => table,
        None => return raise_invalid_state(format!("wrong lengths {:?}", literals).as_str()),
    };

    let distances = match HuffmanTable::new(distances) {
        Some(table) => table,
        None => return raise_invalid_state("wrong lengths"),
    };

    Ok((literals, distances))
}

fn build_length_table(hlen: usize, bitstream: &mut impl BitStream) -> InflateResult<HuffmanTable<8, 19>> {
    let mut lengths: [u16; 19] = [0; 19];
    let mapping = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

    for i in 0..hlen {
        lengths[mapping[i]] = match bitstream.next_bits(3) {
            Some(bits) => bits as u16,
            None => return raise_not_enough_data(format!("collecting lengths {}/{}", i, hlen - 1).as_str()),
        };
    }

    match HuffmanTable::new(lengths) {
        Some(table) => Ok(table),
        None => raise_invalid_state(format!("wrong lengths {:?}", lengths).as_str()),
    }
}

fn build_dynamic_table<const T: usize>(
    bitstream: &mut impl BitStream,
    lengths: &HuffmanTable<8, 19>,
    count: usize,
) -> InflateResult<HuffmanTable<16, T>> {
    let mut index: usize = 0;
    let mut symbols = [0; T];

    if count > T {
        return raise_invalid_state(format!("HuffmanTable of size {} cannot accept {} symbols", T, count).as_str());
    }

    while index < count {
        let value = match lengths.decode(bitstream) {
            Ok(value) => value,
            Err(error) => return raise_decoding_failed("lengths", error),
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

    match HuffmanTable::new(symbols) {
        Some(table) => Ok(table),
        None => raise_invalid_state(format!("wrong lengths {:?}", symbols).as_str()),
    }
}

fn build_dynamic_tables(
    bitstream: &mut impl BitStream,
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
        Ok(lengths) => lengths,
        Err(error) => return raise_invalid_table("lengths", error.to_string()),
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
    pub fn open(bitstream: &mut impl BitStream) -> InflateResult<Self> {
        let last = match bitstream.next_bit() {
            Some(bit) => bit == 1,
            None => return raise_not_enough_data("last_block bit"),
        };

        let mode = match bitstream.next_bits(2) {
            Some(bits) => bits as u8,
            None => return raise_not_enough_data("mode bit"),
        };

        let decoder = match mode {
            0 => InflateDecoder::Uncompressed {
                uncompressed: InflateUncompressed::new(),
            },
            1 => {
                let (literals, distances) = match build_fixed_tables() {
                    Ok((literals, distances)) => (literals, distances),
                    Err(error) => return Err(error),
                };

                InflateDecoder::huffman(literals, distances)
            }
            2 => match build_dynamic_tables(bitstream) {
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

    pub fn next(&mut self, bitstream: &mut impl BitStream) -> InflateResult<InflateSymbol> {
        match &self.decoder {
            InflateDecoder::Huffman { huffman } => huffman.next(bitstream),
            InflateDecoder::Uncompressed { uncompressed } => uncompressed.next(bitstream),
        }
    }
}

impl InflateReader {
    pub fn new() -> Self {
        Self {
            offset: 0,
            current: None,
            buffer: None,
            completed: false,
            failed: false,
        }
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
        self.completed && self.buffer.is_none()
    }

    pub fn is_broken(&self) -> bool {
        self.failed
    }

    pub fn next(&mut self, bitstream: &mut impl BitStream) -> InflateResult<InflateEvent> {
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
            hash: Adler32::new(),
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

    pub fn collect(&mut self, buffer: &mut [u8]) -> usize {
        let available = std::cmp::min(buffer.len(), self.offset);
        buffer[..available].copy_from_slice(&self.buffer[..available]);

        self.buffer.copy_within(available..self.offset, 0);
        self.offset -= available;

        self.hash.update(&buffer[..available]);
        available
    }

    pub fn checksum(&self) -> u32 {
        self.hash.finalize()
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

    fn decode(&self, bitstream: &mut impl BitStream, length: u16) -> InflateResult<InflateSymbol> {
        let length_bits = (std::cmp::max(length, 261) as usize - 261) / 4;
        let length_bits = if length_bits == 6 { 0 } else { length_bits };

        let length_extra = match bitstream.next_bits(length_bits) {
            Some(bits) => bits as u16,
            None => return raise_not_enough_data("literal bits"),
        };

        let distance = match self.distances.decode(bitstream) {
            Ok(value) => value,
            Err(error) => return raise_decoding_failed("distances", error),
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

    fn next(&self, bitstream: &mut impl BitStream) -> InflateResult<InflateSymbol> {
        let literal = match self.literals.decode(bitstream) {
            Ok(value) => value,
            Err(error) => return raise_decoding_failed("literals", error),
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

    fn next(&self, bitstream: &mut impl BitStream) -> InflateResult<InflateSymbol> {
        let len = match bitstream.next_bytes(2) {
            Ok(value) => ((value[1] as u16) << 8) + value[0] as u16,
            Err(error) => return raise_not_enough_data(format!("len bytes: {}", error.to_string()).as_str()),
        };

        let nlen = match bitstream.next_bytes(2) {
            Ok(value) => ((value[1] as u16) << 8) + value[0] as u16,
            Err(error) => return raise_not_enough_data(format!("nlen bytes: {}", error.to_string()).as_str()),
        };

        if !((len ^ 0xFFFF) == nlen) {
            return raise_invalid_state("nlen doesn't match len");
        }

        let symbol = match bitstream.next_bytes(len as usize) {
            Ok(data) => InflateSymbol::Uncompressed { data: data },
            Err(error) => return raise_not_enough_data(format!("{}", error).as_str()),
        };

        Ok(symbol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitstream::BitStreamDefault;

    fn bitstream<const T: usize>(data: &[u8]) -> BitStreamDefault<T> {
        let mut bitstream = BitStreamDefault::new();
        bitstream.append(data).unwrap();
        bitstream
    }

    #[test]
    fn lists_few_symbols_using_length_table() {
        let data = [0b11011001, 0b0000100];
        let mut bitstream: BitStreamDefault<10> = bitstream(&data);

        let table = build_length_table(4, &mut bitstream).unwrap();
        let codes = table.list();

        assert_eq!(codes[16], HuffmanCode::new(0b0, 1));
        assert_eq!(codes[17], HuffmanCode::new(0b110, 3));
        assert_eq!(codes[18], HuffmanCode::new(0b111, 3));
        assert_eq!(codes[0], HuffmanCode::new(0b10, 2));
    }

    #[test]
    fn decodes_few_symbols_using_fixed_huffman_tables() {
        let (literals, distances) = build_fixed_tables().unwrap();

        let data = [
            0b00001100, 0b11111101, 0b00010011, 0b11111110, 0b00000011, 0b11101000, 0b00000011, 0b11100011, 0b01010100,
            0b01111001,
        ];

        let mut bitstream: BitStreamDefault<10> = bitstream(&data);

        assert_eq!(literals.decode(&mut bitstream), Ok(0));
        assert_eq!(literals.decode(&mut bitstream), Ok(143));

        assert_eq!(literals.decode(&mut bitstream), Ok(144));
        assert_eq!(literals.decode(&mut bitstream), Ok(255));

        assert_eq!(literals.decode(&mut bitstream), Ok(256));
        assert_eq!(literals.decode(&mut bitstream), Ok(279));

        assert_eq!(literals.decode(&mut bitstream), Ok(280));
        assert_eq!(literals.decode(&mut bitstream), Ok(287));

        assert_eq!(distances.decode(&mut bitstream), Ok(5));
        assert_eq!(distances.decode(&mut bitstream), Ok(10));
        assert_eq!(distances.decode(&mut bitstream), Ok(15));
    }

    #[test]
    fn fails_building_lengths_table() {
        let data = [0b11011001];
        let mut bitstream: BitStreamDefault<10> = bitstream(&data);

        match build_length_table(4, &mut bitstream) {
            Ok(_) => assert!(false),
            Err(error) => match error {
                InflateError::NotEnoughData(_) => assert!(true),
                _ => assert!(false),
            },
        };
    }

    #[test]
    fn fails_building_dynamic_table_due_to_missing_data() {
        let data = [0b11011000];
        let mut bitstream: BitStreamDefault<10> = bitstream(&data);

        let lengths = [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let lengths: HuffmanTable<8, 19> = HuffmanTable::new(lengths).unwrap();

        let result: Result<HuffmanTable<16, 10>, InflateError> = build_dynamic_table(&mut bitstream, &lengths, 10);

        match result {
            Ok(_) => assert!(false),
            Err(error) => match error {
                InflateError::DecodingFailed(_, _) => assert!(true),
                _ => assert!(false),
            },
        };
    }
}
