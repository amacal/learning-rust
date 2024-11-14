use super::array::*;
use super::heap::*;
use super::lexer::*;

struct ElementsMarker;
struct OperatorsMarker;

impl ArrayLike for ElementsMarker {}
impl StackLike for ElementsMarker {}
impl StackLike for OperatorsMarker {}

pub struct RPN<const SIZE: usize>(Array<ElementsMarker, u8, SIZE, GuardSegfault>);

impl<const SIZE: usize> RPN<SIZE> {
    #[cfg(test)]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut elements = Array::new();

        for &val in bytes.iter() {
            elements.stack_push_front(val);
        }

        Self(elements)
    }

    #[cfg(test)]
    fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes_front()
    }

    pub fn size(&self) -> u16 {
        self.0.stack_size_front()
    }

    pub fn at(&self, off: u16) -> u8 {
        self.0.array_get(off)
    }
}

struct Builder<const SIZE: usize> {
    elements: Array<ElementsMarker, u8, SIZE, GuardSegfault>,
    operators: Array<OperatorsMarker, u8, 4096, GuardSegfault>,
}

impl<const SIZE: usize> Builder<SIZE> {
    fn new() -> Self {
        Self {
            elements: Array::new(),
            operators: Array::new(),
        }
    }

    fn build(mut self, input: *const u8) -> Option<RPN<SIZE>> {
        let mut lexer = Lexer::new(input);
        let mut in_character = false;
        let mut in_classes = false;
        let mut in_negation = false;
        let mut classes_depth = 0;

        loop {
            if in_classes {
                let token = lexer.next_in_class();

                match token {
                    (0, _) => break,
                    (b'^', _) => {
                        in_negation = true;
                    }
                    (b']', _) => {
                        while self.operators.stack_size_front() > 0 {
                            match self.operators.stack_peek_front() {
                                b'[' => {
                                    self.operators.stack_pop_front();
                                    break;
                                }
                                _ => break,
                            }
                        }

                        in_negation = false;
                        in_classes = false;
                    }
                    (from, to) => {
                        self.elements.stack_push_front(if in_negation { b'!' } else { b'-' });
                        self.elements.stack_push_front(from);
                        self.elements.stack_push_front(to);
                        classes_depth += 1;

                        if classes_depth > 1 {
                            self.elements.stack_push_front(b'|');
                        }
                    }
                }
            } else {
                let token = if let Some(token) = lexer.next_in_group() {
                    unsafe { (*token.0, token.0, token.1) }
                } else {
                    break;
                };

                match token.0 {
                    b'(' | b'+' | b'?' => {
                        self.operators.stack_push_front(token.0);
                        in_character = false;
                    }
                    b'*' => {
                        self.operators.stack_push_front(b'?');
                        self.operators.stack_push_front(b'+');
                        in_character = false;

                    }
                    b'[' => {
                        self.operators.stack_push_front(token.0);
                        in_character = false;
                        in_classes = true;
                        classes_depth = 0;
                    }
                    b'|' => {
                        while self.operators.stack_size_front() > 0 {
                            match self.operators.stack_peek_front() {
                                b'*' | b'+' | b'?' | b'&' => {
                                    let token = self.operators.stack_pop_front();
                                    self.elements.stack_push_front(token);
                                }
                                b'(' => break,
                                _ => {}
                            }
                        }

                        self.operators.stack_push_front(b'|');
                        in_character = false;
                    }
                    b')' => {
                        while self.operators.stack_size_front() > 0 {
                            match self.operators.stack_peek_front() {
                                b'*' | b'+' | b'?' | b'&' => {
                                    let token = self.operators.stack_pop_front();
                                    self.elements.stack_push_front(token);
                                }
                                b'(' => {
                                    self.operators.stack_pop_front();
                                    break;
                                }
                                _ => break,
                            }
                        }

                        in_character = false;
                    }
                    _ if in_character => {
                        self.elements.stack_push_front(b'l');
                        self.elements.stack_push_front(b'0' + token.2);

                        for off in 0..token.2 {
                            unsafe {
                                self.elements.stack_push_front(*token.1.add(off.into()));
                            }
                        }

                        self.operators.stack_push_front(b'&');
                    }
                    _ => {
                        self.elements.stack_push_front(b'l');
                        self.elements.stack_push_front(b'0' + token.2);

                        for off in 0..token.2 {
                            unsafe {
                                self.elements.stack_push_front(*token.1.add(off.into()));
                            }
                        }

                        in_character = true;
                    }
                }
            }
        }

        while self.operators.stack_size_front() > 0 {
            let val = self.operators.stack_pop_front();
            self.elements.stack_push_front(val);
        }

        Some(RPN(self.elements))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_rpn_from_seq_of_one_character() {
        let regex = b"a\0".as_ptr();
        let builder = Builder::<4096>::new();

        let regex = match builder.build(regex) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l1a");
    }

    #[test]
    fn handles_rpn_from_seq_of_two_characters() {
        let regex = b"ab\0".as_ptr();
        let builder = Builder::<4096>::new();

        let regex = match builder.build(regex) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l2ab");
    }

    #[test]
    fn handles_rpn_from_seq_of_alphabet() {
        let regex = b"abcdefghijklmnopqrstuvwxyz\0".as_ptr();
        let builder = Builder::<4096>::new();

        let regex = match builder.build(regex) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l9abcdefghil9jklmnopqrl8stuvwxyz&&");
    }

    #[test]
    fn handles_rpn_from_either() {
        let regex = b"a|b\0".as_ptr();
        let builder = Builder::<4096>::new();

        let regex = match builder.build(regex) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l1al1b|");
    }

    #[test]
    fn handles_rpn_from_star() {
        let regex = b"ab*\0".as_ptr();
        let builder = Builder::<4096>::new();

        let regex = match builder.build(regex) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l1al1b+?&");
    }

    #[test]
    fn handles_rpn_from_plus() {
        let regex = b"ab+\0".as_ptr();
        let builder = Builder::<4096>::new();

        let regex = match builder.build(regex) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l1al1b+&");
    }

    #[test]
    fn handles_rpn_from_optional() {
        let regex = b"ab?\0".as_ptr();
        let builder = Builder::<4096>::new();

        let regex = match builder.build(regex) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l1al1b?&");
    }

    #[test]
    fn handles_rpn_from_either_and() {
        let regex = b"abcdefghijkl|mnopqrstuvwx\0".as_ptr();
        let builder = Builder::<4096>::new();

        let regex = match builder.build(regex) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l9abcdefghil3jkl&l9mnopqrstul3vwx&|");
    }

    #[test]
    fn handles_rpn_from_group() {
        let regex = b"(ab)*|(cd)+\0".as_ptr();
        let builder = Builder::<4096>::new();

        let regex = match builder.build(regex) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l2ab+?l2cd+|");
    }

    #[test]
    fn handles_rpn_from_class() {
        let regex = b"(ab)*|[a-z]+\0".as_ptr();
        let builder = Builder::<4096>::new();

        let regex = match builder.build(regex) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l2ab+?-az+|");
    }

    #[test]
    fn handles_rpn_from_class_multiple() {
        let regex = b"(ab)*|[a-z0-9]+\0".as_ptr();
        let builder = Builder::<4096>::new();

        let regex = match builder.build(regex) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l2ab+?-az-09|+|");
    }

    #[test]
    fn handles_rpn_from_class_multiple_negated() {
        let regex = b"(ab)*|[^a-z0-9]+\0".as_ptr();
        let builder = Builder::<4096>::new();

        let regex = match builder.build(regex) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l2ab+?!az!09|+|");
    }
}
