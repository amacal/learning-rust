use crate::regex::*;

pub struct Lexer {
    data: *const u8,
    offset: usize,
    mask: usize,
    dfa: DFA,
}

impl Lexer {
    pub fn new(data: *const u8, mask: usize) -> Option<Self> {
        let regex = b"({#\x01)|(}#\x02)|([[]#\x03)|(]#\x04)|(,#\x05)|(:#\x06)|(( |\n)+#\x07)|((\"[^\"]+\")#\x08)|((-?(0|[1-9][0-9]*)(.[0-9]+)?([eE]([+]|-)?[0-9]+)?)#\x09)|((false)#\x0a)|((true)#\x0b)|((null)#\x0c)\0".as_ptr();

        let rpn: RPN<4096> = match RPN::build(regex) {
            Some(rpn) => rpn,
            None => return None,
        };

        let nfa = match NFA::build(rpn) {
            Some(nfa) => nfa,
            None => return None,
        };

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
        //println!("{} {}", self.offset, length);

        let (token, length) = match self.dfa.traverse_ptr(self.data, self.mask, self.offset, length) {
            None => return None,
            Some(val) => val,
        };

        self.offset = self.offset.wrapping_add(length);
        Some((token, length))
    }

    pub fn offset(&self) -> usize {
        self.offset
    }
}
