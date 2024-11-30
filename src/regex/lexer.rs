use super::dfa::DFA;
use super::nfa::NFA;
use super::rpn::{Builder, RPN};

pub struct Lexer {
    input: *const u8,
}

pub struct Lexer2 {
    data: *const u8,
    group: DFA,
    class: DFA,
}

impl Lexer {
    pub fn new(input: *const u8) -> Option<Self> {
        Some(Self { input: input })
    }

    fn at(&self, off: i8) -> u8 {
        unsafe { *self.input.offset(off.into()) }
    }

    pub fn next_in_group(&mut self) -> Option<(*const u8, u8)> {
        let mut off = 0i8;
        let mut value = self.at(0);

        loop {
            match value {
                0 if off == 0 => return None,
                0 => break,
                b'#' if off == 0 && self.at(1) > 0 => break off = 2,
                b'(' | b')' | b'[' | b'|' if off == 0 => break off = 1,
                b'(' | b')' | b'[' | b'|' | b'#' => break,
                b'*' | b'+' | b'?' if off == 0 => break off = 1,
                b'*' | b'+' | b'?' if off == 1 => break,
                b'*' | b'+' | b'?' => break off = off.wrapping_sub(1), // literal was prepended
                _ => (),
            }

            off = off.wrapping_add(1);
            value = self.at(off);

            if off == 9 {
                break;
            }
        }

        unsafe {
            let value = self.input;
            self.input = self.input.offset(off.into());
            Some((value, off as u8))
        }
    }

    pub fn next_in_class(&mut self) -> (u8, u8) {
        let mut result = match self.at(0) {
            0 => (0, 0),
            b'^' => (b'^', 1),
            b']' => (b']', 1),
            value => (value, 3),
        };

        let expanded = result.1 == 3;

        if result.1 == 3 {
            if self.at(1) != b'-' || self.at(2) == 0 || self.at(2) == b']' {
                result = (result.0, 1);
            }
        }

        unsafe {
            self.input = self.input.add(result.1);
            (result.0, if expanded == false { 0 } else { self.at(-1) })
        }
    }
}

fn build_dfa<const SIZE: usize>(builder: Builder<SIZE>) -> Option<DFA> {
    let rpn = match builder.build() {
        Some(rpn) => rpn,
        None => return None,
    };

    let nfa = match NFA::build(rpn) {
        Some(nfa) => nfa,
        None => return None,
    };

    match DFA::build(nfa) {
        Some(dfa) => Some(dfa),
        None => None,
    }
}

impl Lexer2 {
    pub fn new(data: *const u8) -> Option<Self> {
        let mut group = RPN::<4096>::builder();
        let mut class = RPN::<4096>::builder();

        // literal
        group.append(b"[a-z]+#\x01\0".as_ptr());

        // marker
        group.append(b"[#][\x01-\xff]#\x02\0".as_ptr());

        // negate class
        class.append(b"^#\x03\0".as_ptr());

        // open class
        group.append(b"[[]#\x04\0".as_ptr());

        // close class
        class.append(b"]#\x05\0".as_ptr());

        // range class single without [, - or ^
        class.append(b"([\x01-\x2c\x2e-\x5b\x5f-\xff])#\x06\0".as_ptr());

        // range class single, escaped
        class.append(b"(\\[\x01-\xff])#\x07\0".as_ptr());

        // range class dashed without ], - or ^ both
        class.append(b"([\x01-\x2c\x2e-\x5b\x5f-\xff]-[\x01-\x2c\x2e-\x5b\x5f-\xff])#\x08\0".as_ptr());

        // range class dashed without ], - or ^ right, escaped left
        class.append(b"(\\[\x01-\xff]-[\x01-\x2c\x2e-\x5b\x5f-\xff])#\x09\0".as_ptr());

        // range class dashed without ], - or ^ left, escaped right
        class.append(b"([\x01-\x2c\x2e-\x5b\x5f-\xff]-\\[\x01-\xff])#\x0a\0".as_ptr());

        // range class dashed, escaped both
        class.append(b"(\\[\x01-\xff]-\\[\x01-\xff])#\x0b\0".as_ptr());

        // open grouo
        group.append(b"[()]#\x0c\0".as_ptr());

        // close grouo
        class.append(b"[)]#\x0d\0".as_ptr());

        let group = match build_dfa(group) {
            None => return None,
            Some(dfa) => dfa,
        };

        let class = match build_dfa(class) {
            None => return None,
            Some(dfa) => dfa,
        };

        Some(Self { data: data, group: group, class: class })
    }

    fn next(&mut self, in_group: bool) -> Option<Token> {
        let dfa = if in_group { &self.group } else { &self.class };
        let (token, start, end) = match dfa.traverse_ptr2(self.data) {
            None => return None,
            Some((token, end)) => (token, self.data, end),
        };

        let result = match token {
            id if id == 0x01 => Some(Token::Literal { start: start, length: unsafe { end.offset_from(start) as usize } }),
            id if id == 0x02 => Some(Token::Marker { value: unsafe { *start.add(1) } }),
            id if id == 0x03 => Some(Token::NegateClass {}),
            id if id == 0x04 => Some(Token::OpenClass {}),
            id if id == 0x05 => Some(Token::CloseClass {}),
            id if id == 0x06 => Some(Token::RangeClass { min: unsafe { *start }, max: unsafe { *start } }),
            id if id == 0x07 => Some(Token::RangeClass { min: unsafe { *start.add(1) }, max: unsafe { *start.add(1) } }),
            id if id == 0x08 => Some(Token::RangeClass { min: unsafe { *start }, max: unsafe { *start.add(2) } }),
            id if id == 0x09 => Some(Token::RangeClass { min: unsafe { *start.add(1) }, max: unsafe { *start.add(3) } }),
            id if id == 0x0a => Some(Token::RangeClass { min: unsafe { *start.add(0) }, max: unsafe { *start.add(3) } }),
            id if id == 0x0b => Some(Token::RangeClass { min: unsafe { *start.add(1) }, max: unsafe { *start.add(4) } }),
            id if id == 0x0c => Some(Token::OpenGroup {}),
            id if id == 0x0d => Some(Token::OpenGroup {}),
            _ => None,
        };

        self.data = end;
        return result;
    }

    pub fn next_in_group(&mut self) -> Option<Token> {
        self.next(true)
    }

    pub fn next_in_class(&mut self) -> Option<Token> {
        self.next(false)
    }
}

#[derive(PartialEq, Debug)]
pub enum Token {
    Literal { start: *const u8, length: usize },
    Marker { value: u8 },
    OpenGroup {},
    CloseGroup {},
    NegateClass {},
    OpenClass {},
    CloseClass {},
    RangeClass { min: u8, max: u8 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_regex_tokenization() {
        let input = b"(ab)*|[a-z_]+|[^abc0-9]+|ab+\0".as_ptr();
        let mut lexer = match Lexer::new(input) {
            None => return assert!(false),
            Some(lexer) => lexer,
        };

        unsafe {
            assert_eq!(lexer.next_in_group(), Some((input.add(0), 1)));
            assert_eq!(lexer.next_in_group(), Some((input.add(1), 2)));
            assert_eq!(lexer.next_in_group(), Some((input.add(3), 1)));
            assert_eq!(lexer.next_in_group(), Some((input.add(4), 1)));
            assert_eq!(lexer.next_in_group(), Some((input.add(5), 1)));
            assert_eq!(lexer.next_in_group(), Some((input.add(6), 1)));
            assert_eq!(lexer.next_in_class(), (b'a', b'z'));
            assert_eq!(lexer.next_in_class(), (b'_', b'_'));
            assert_eq!(lexer.next_in_class(), (b']', 0));
            assert_eq!(lexer.next_in_group(), Some((input.add(12), 1)));
            assert_eq!(lexer.next_in_group(), Some((input.add(13), 1)));
            assert_eq!(lexer.next_in_group(), Some((input.add(14), 1)));
            assert_eq!(lexer.next_in_class(), (b'^', 0));
            assert_eq!(lexer.next_in_class(), (b'a', b'a'));
            assert_eq!(lexer.next_in_class(), (b'b', b'b'));
            assert_eq!(lexer.next_in_class(), (b'c', b'c'));
            assert_eq!(lexer.next_in_class(), (b'0', b'9'));
            assert_eq!(lexer.next_in_class(), (b']', 0));
            assert_eq!(lexer.next_in_group(), Some((input.add(23), 1)));
            assert_eq!(lexer.next_in_group(), Some((input.add(24), 1)));
            assert_eq!(lexer.next_in_group(), Some((input.add(25), 1)));
            assert_eq!(lexer.next_in_group(), Some((input.add(26), 1)));
            assert_eq!(lexer.next_in_group(), Some((input.add(27), 1)));

            assert_eq!(lexer.next_in_group(), None);
            assert_eq!(lexer.next_in_group(), None);
            assert_eq!(lexer.next_in_group(), None);
        }
    }

    #[test]
    fn handles_regex_tokenization_long() {
        let input = b"abcdefghijklmnopqrstuvwxyz\0".as_ptr();
        let mut lexer = match Lexer::new(input) {
            None => return assert!(false),
            Some(lexer) => lexer,
        };

        unsafe {
            assert_eq!(lexer.next_in_group(), Some((input.add(0), 9)));
            assert_eq!(lexer.next_in_group(), Some((input.add(9), 9)));
            assert_eq!(lexer.next_in_group(), Some((input.add(18), 8)));
        }
    }

    #[test]
    fn handles_regex_tokenization_accepting_byte() {
        let input = b"ab#x\0".as_ptr();
        let mut lexer = match Lexer::new(input) {
            None => return assert!(false),
            Some(lexer) => lexer,
        };

        unsafe {
            assert_eq!(lexer.next_in_group(), Some((input.add(0), 2)));
            assert_eq!(lexer.next_in_group(), Some((input.add(2), 2)));
        }
    }

    #[test]
    fn handles_regex_tokenization_accepting_byte2() {
        let input = b"ab#\x65\0".as_ptr();
        let mut lexer = match Lexer2::new(input) {
            None => return assert!(false),
            Some(lexer) => lexer,
        };

        assert_eq!(lexer.next_in_group(), Some(Token::Literal { start: input, length: 2 }));
        assert_eq!(lexer.next_in_group(), Some(Token::Marker { value: 0x65 }));
        assert_eq!(lexer.next_in_group(), None);
    }

    #[test]
    fn handles_regex_tokenization_accepting_positive_class() {
        let input = b"[a-m0123n-z]\0".as_ptr();
        let mut lexer = match Lexer2::new(input) {
            None => return assert!(false),
            Some(lexer) => lexer,
        };

        assert_eq!(lexer.next_in_group(), Some(Token::OpenClass {}));
        assert_eq!(lexer.next_in_class(), Some(Token::RangeClass { min: b'a', max: b'm' }));
        assert_eq!(lexer.next_in_class(), Some(Token::RangeClass { min: b'0', max: b'0' }));
        assert_eq!(lexer.next_in_class(), Some(Token::RangeClass { min: b'1', max: b'1' }));
        assert_eq!(lexer.next_in_class(), Some(Token::RangeClass { min: b'2', max: b'2' }));
        assert_eq!(lexer.next_in_class(), Some(Token::RangeClass { min: b'3', max: b'3' }));
        assert_eq!(lexer.next_in_class(), Some(Token::RangeClass { min: b'n', max: b'z' }));
        assert_eq!(lexer.next_in_class(), Some(Token::CloseClass {}));
        assert_eq!(lexer.next_in_group(), None);
    }

    #[test]
    fn handles_regex_tokenization_accepting_negative_class() {
        let input = b"[^a-m0123n-z]\0".as_ptr();
        let mut lexer = match Lexer2::new(input) {
            None => return assert!(false),
            Some(lexer) => lexer,
        };

        assert_eq!(lexer.next_in_group(), Some(Token::OpenClass {}));
        assert_eq!(lexer.next_in_class(), Some(Token::NegateClass {}));
        assert_eq!(lexer.next_in_class(), Some(Token::RangeClass { min: b'a', max: b'm' }));
        assert_eq!(lexer.next_in_class(), Some(Token::RangeClass { min: b'0', max: b'0' }));
        assert_eq!(lexer.next_in_class(), Some(Token::RangeClass { min: b'1', max: b'1' }));
        assert_eq!(lexer.next_in_class(), Some(Token::RangeClass { min: b'2', max: b'2' }));
        assert_eq!(lexer.next_in_class(), Some(Token::RangeClass { min: b'3', max: b'3' }));
        assert_eq!(lexer.next_in_class(), Some(Token::RangeClass { min: b'n', max: b'z' }));
        assert_eq!(lexer.next_in_class(), Some(Token::CloseClass {}));
        assert_eq!(lexer.next_in_group(), None);
    }

    #[test]
    fn handles_regex_tokenization_accepting_positive_class_escaped() {
        let input = b"[\\a-m0\\12\\3n-\\z\\4-\\5\\^]\0".as_ptr();
        let mut lexer = match Lexer2::new(input) {
            None => return assert!(false),
            Some(lexer) => lexer,
        };

        assert_eq!(lexer.next_in_group(), Some(Token::OpenClass {}));
        assert_eq!(lexer.next_in_class(), Some(Token::RangeClass { min: b'a', max: b'm' }));
        assert_eq!(lexer.next_in_class(), Some(Token::RangeClass { min: b'0', max: b'0' }));
        assert_eq!(lexer.next_in_class(), Some(Token::RangeClass { min: b'1', max: b'1' }));
        assert_eq!(lexer.next_in_class(), Some(Token::RangeClass { min: b'2', max: b'2' }));
        assert_eq!(lexer.next_in_class(), Some(Token::RangeClass { min: b'3', max: b'3' }));
        assert_eq!(lexer.next_in_class(), Some(Token::RangeClass { min: b'n', max: b'z' }));
        assert_eq!(lexer.next_in_class(), Some(Token::RangeClass { min: b'4', max: b'5' }));
        assert_eq!(lexer.next_in_class(), Some(Token::RangeClass { min: b'^', max: b'^' }));
        assert_eq!(lexer.next_in_class(), Some(Token::CloseClass {}));
        assert_eq!(lexer.next_in_group(), None);
    }

    #[test]
    fn handles_regex_tokenization_accepting_negative_class_escaped() {
        let input = b"[^\\a-m\\012\\3n-\\z\\4-\\5\\^]\0".as_ptr();
        let mut lexer = match Lexer2::new(input) {
            None => return assert!(false),
            Some(lexer) => lexer,
        };

        assert_eq!(lexer.next_in_group(), Some(Token::OpenClass {}));
        assert_eq!(lexer.next_in_class(), Some(Token::NegateClass {}));
        assert_eq!(lexer.next_in_class(), Some(Token::RangeClass { min: b'a', max: b'm' }));
        assert_eq!(lexer.next_in_class(), Some(Token::RangeClass { min: b'0', max: b'0' }));
        assert_eq!(lexer.next_in_class(), Some(Token::RangeClass { min: b'1', max: b'1' }));
        assert_eq!(lexer.next_in_class(), Some(Token::RangeClass { min: b'2', max: b'2' }));
        assert_eq!(lexer.next_in_class(), Some(Token::RangeClass { min: b'3', max: b'3' }));
        assert_eq!(lexer.next_in_class(), Some(Token::RangeClass { min: b'n', max: b'z' }));
        assert_eq!(lexer.next_in_class(), Some(Token::RangeClass { min: b'4', max: b'5' }));
        assert_eq!(lexer.next_in_class(), Some(Token::RangeClass { min: b'^', max: b'^' }));
        assert_eq!(lexer.next_in_class(), Some(Token::CloseClass {}));
        assert_eq!(lexer.next_in_group(), None);
    }
}
