use std::collections::BinaryHeap;

#[derive(Debug, Eq, PartialEq)]
pub struct HuffmanNode {
    value: Option<u8>,
    frequency: usize,
    left: Option<Box<HuffmanNode>>,
    right: Option<Box<HuffmanNode>>,
}

impl HuffmanNode {
    fn leaf(value: u8, frequency: usize) -> Self {
        Self {
            value: Some(value),
            frequency: frequency,
            left: None,
            right: None,
        }
    }

    fn node(frequency: usize, left: HuffmanNode, right: HuffmanNode) -> Self {
        Self {
            value: None,
            frequency: frequency,
            left: Some(Box::new(left)),
            right: Some(Box::new(right)),
        }
    }
}

impl HuffmanNode {
    pub fn from_frequencies(frequencies: [usize; 256]) -> Option<Box<Self>> {
        let mut queue = BinaryHeap::new();

        for i in 0..256 {
            if frequencies[i] > 0 {
                queue.push(Self::leaf(i as u8, frequencies[i]))
            }
        }

        while queue.len() > 1 {
            if let (Some(left), Some(right)) = (queue.pop(), queue.pop()) {
                queue.push(Self::node(left.frequency + right.frequency, left, right))
            }
        }

        match queue.pop() {
            None => None,
            Some(root) => Some(Box::new(root)),
        }
    }
}

impl Ord for HuffmanNode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.frequency.cmp(&self.frequency).then(self.value.cmp(&other.value))
    }
}

impl PartialOrd for HuffmanNode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(&other))
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct HuffmanCode {
    value: u8,
    frequency: usize,
    length: usize,
    bits: u128,
}

impl HuffmanCode {
    pub fn from_tree(tree: &Option<Box<HuffmanNode>>) -> Vec<Self> {
        fn collect(output: &mut Vec<HuffmanCode>, node: &Option<Box<HuffmanNode>>, indent: usize, bits: u128) {
            if let Some(node) = node {
                if let Some(value) = node.value {
                    output.push(HuffmanCode {
                        value: value,
                        frequency: node.frequency,
                        length: indent,
                        bits: bits,
                    });
                }

                collect(output, &node.left, indent + 1, bits << 1);
                collect(output, &node.right, indent + 1, bits << 1 | 0x1);
            }
        }

        let mut result = Vec::with_capacity(256);
        collect(&mut result, &tree, 0, 0);
        result
    }

    pub fn as_canonical(codes: &Vec<Self>) -> Vec<Self> {
        let mut sorted: Vec<Self> = codes.iter().cloned().collect();

        sorted.sort();

        let mut bits = 0;
        let mut length = 0;

        for mut code in sorted.iter_mut() {
            while length < code.length {
                bits <<= 1;
                length += 1;
            }

            code.bits = bits;
            bits += 1;
        }

        sorted
    }

    pub fn describe(codes: &Vec<Self>) {
        for code in codes.iter() {
            println!(
                "{:>3} {} {} {:0width$b}",
                code.value,
                code.frequency,
                code.length,
                code.bits,
                width = code.length
            )
        }
    }
}

impl Ord for HuffmanCode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.length.cmp(&other.length).then(self.value.cmp(&other.value))
    }
}

impl PartialOrd for HuffmanCode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(&other))
    }
}

#[derive(Debug)]
pub struct HuffmanTable {
    counts: [u8; 8],
    symbols: [u8; 32],
}

impl HuffmanTable {
    pub fn from_codes(codes: &Vec<HuffmanCode>) -> Self {
        let mut counts = [0; 8];
        let mut symbols = [0; 32];
        let mut offset = 0;
    
        for code in codes.iter() {
            counts[code.length as usize] += 1;
            symbols[offset] = code.value;
            offset += 1;
        }
    
        Self {
            counts: counts,
            symbols: symbols,
        }
    }

    pub fn decode(&self, bits: u128) -> Option<u8> {
        let mut length = 1;
        let mut first: u128 = 0;
        let mut bits = bits;
    
        let mut count: u128;
        let mut code = 0;
        let mut offset: u128 = 0;
    
        while length < 8 {
            code |= bits & 0x1;
            count = self.counts[length] as u128;
    
            if code < first + count {
                return Some(self.symbols[offset as usize + (code - first) as usize]);
            }
    
            offset += count;
            first += count;
            length += 1;
    
            first <<= 1;
            code <<= 1;
            bits >>= 1;
        }
    
        None
    }    

    pub fn describe(&self) {
        println!("Counts:  {:?}", self.counts);
        println!("Symbols: {:?}", self.symbols);
    }
}

fn main() {
    let mut frequencies = [0; 256];
    let sentence = b"the quick brown fox jumps over the lazy dog";

    for key in sentence.iter() {
        frequencies[*key as usize] += 1;        
    }

    let tree = HuffmanNode::from_frequencies(frequencies);
    let codes = HuffmanCode::from_tree(&tree);
    
    HuffmanCode::describe(&codes);

    let canonical = HuffmanCode::as_canonical(&codes);
    HuffmanCode::describe(&canonical);

    let table = HuffmanTable::from_codes(&canonical);
    table.describe();

    println!("{:?}", table.decode(0b0100));
}
