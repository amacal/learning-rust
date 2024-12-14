use crate::regex::*;

pub struct Lexer {
    data: *const u8,
    offset: usize,
    mask: usize,
    dfa: DFA,
}

impl Lexer {
    pub fn new(data: *const u8, mask: usize) -> Option<Self> {
        let mut builder = RPN::<4096>::builder();

        // catch {, }, [, ], comma and colon
        builder.append(b"{#\x01\0".as_ptr());
        builder.append(b"}#\x02\0".as_ptr());
        builder.append(b"\\[#\x03\0".as_ptr());
        builder.append(b"\\]#\x04\0".as_ptr());
        builder.append(b",#\x05\0".as_ptr());
        builder.append(b":#\x06\0".as_ptr());

        // catch white characters
        builder.append(b"( |\n)+#\x07\0".as_ptr());

        // catch double quoted string literal
        builder.append(b"(\"([^\"\\\\]|\\\\u[0-9a-f][0-9a-f][0-9a-f][0-9a-f]|\\\\[\"\\\\/bfnrt])*\")#\x08\0".as_ptr());

        // catch number with optional floating part or scientific notation
        builder.append(b"(\\-?(0|[1-9][0-9]*)(\\.[0-9]+)?([eE](\\+|\\-)?[0-9]+)?)#\x09\0".as_ptr());

        // catch false, true, null literals
        builder.append(b"(false)#\x0a\0".as_ptr());
        builder.append(b"(true)#\x0b\0".as_ptr());
        builder.append(b"(null)#\x0c\0".as_ptr());

        let rpn: RPN<4096> = match builder.build() {
            Ok(rpn) => rpn,
            _ => return None,
        };

        let nfa = match NFA::build(rpn) {
            Some(nfa) => nfa,
            None => return None,
        };

        nfa.print();

        let dfa = match DFA::build(nfa) {
            Some(dfa) => dfa,
            None => return None,
        };

        dfa.print();

        Some(Self {
            data: data,
            mask: mask,
            dfa: dfa,
            offset: 0,
        })
    }

    pub fn process(&mut self, length: usize) -> Option<(u16, usize)> {
        let (token, length) = match self.dfa.traverse_ptr(self.data, self.mask, self.offset, length) {
            None => return None,
            Some(val) => val,
        };

        self.offset += length;
        Some((token, length))
    }

    pub fn offset(&self) -> usize {
        self.offset
    }
}
