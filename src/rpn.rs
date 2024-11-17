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
    pub fn build(input: *const u8) -> Option<Self> {
        Builder::new().build(input)
    }

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

enum BuilderState {
    Completed,
    InGroups { concatenation: bool },
    InClasses { alternation: bool, negation: bool },
}

impl BuilderState {
    fn handle_in_groups<const SIZE: usize>(builder: &mut Builder<SIZE>, lexer: &mut Lexer, concatenation: bool) -> BuilderState {
        let token = if let Some(token) = lexer.next_in_group() {
            unsafe { (*token.0, token.0, token.1) }
        } else {
            return BuilderState::Completed;
        };

        match token.0 {
            b'(' => {
                builder.operators.stack_push_front(if concatenation { 1 } else { 0 });
                builder.operators.stack_push_front(b'(');

                BuilderState::InGroups { concatenation: false }
            }
            b'+' | b'?' => {
                builder.elements.stack_push_front(token.0);
                BuilderState::InGroups { concatenation }
            }
            b'*' => {
                builder.elements.stack_push_front(b'+');
                builder.elements.stack_push_front(b'?');
                BuilderState::InGroups { concatenation }
            }
            b'[' => {
                builder.operators.stack_push_front(if concatenation { 1 } else { 0 });
                builder.operators.stack_push_front(token.0);

                BuilderState::InClasses {
                    alternation: false,
                    negation: false,
                }
            }
            b'|' => {
                while builder.operators.stack_size_front() > 0 {
                    match builder.operators.stack_peek_front() {
                        b'*' | b'+' | b'?' | b'&' => {
                            let token = builder.operators.stack_pop_front();
                            builder.elements.stack_push_front(token);
                        }
                        _ => break,
                    }
                }

                builder.operators.stack_push_front(b'|');
                BuilderState::InGroups { concatenation: false }
            }
            b')' => {
                let mut concatenation = false;

                while builder.operators.stack_size_front() > 0 {
                    match builder.operators.stack_pop_front() {
                        b'(' => {
                            concatenation = builder.operators.stack_pop_front() == 1;
                            break;
                        }
                        value => {
                            builder.elements.stack_push_front(value);
                        }
                    }
                }

                if concatenation {
                    builder.operators.stack_push_front(b'&');
                }

                BuilderState::InGroups { concatenation: true }
            }
            _ => {
                builder.elements.stack_push_front(b'l');
                builder.elements.stack_push_front(b'0' + token.2);

                for off in 0..token.2 {
                    unsafe {
                        builder.elements.stack_push_front(*token.1.add(off.into()));
                    }
                }

                if concatenation {
                    builder.operators.stack_push_front(b'&');
                }

                BuilderState::InGroups { concatenation: true }
            }
        }
    }

    fn handle_in_classes<const SIZE: usize>(builder: &mut Builder<SIZE>, lexer: &mut Lexer, alternation: bool, negation: bool) -> BuilderState {
        let token = lexer.next_in_class();

        match token {
            (0, _) => BuilderState::Completed,
            (b'^', _) => BuilderState::InClasses {
                alternation: alternation,
                negation: true,
            },
            (b']', _) => {
                loop {
                    match builder.operators.stack_pop_front() {
                        b'[' => break,
                        value => builder.elements.stack_push_front(value),
                    }
                }

                BuilderState::InGroups {
                    concatenation: builder.operators.stack_pop_front() == 1,
                }
            }
            (from, to) => {
                builder.elements.stack_push_front(if negation { b'!' } else { b'-' });
                builder.elements.stack_push_front(from);
                builder.elements.stack_push_front(to);

                if alternation {
                    builder.elements.stack_push_front(b'|');
                }

                BuilderState::InClasses {
                    alternation: true,
                    negation: negation,
                }
            }
        }
    }

    fn handle<const SIZE: usize>(self, builder: &mut Builder<SIZE>, lexer: &mut Lexer) -> BuilderState {
        match self {
            BuilderState::Completed => BuilderState::Completed,
            BuilderState::InGroups { concatenation } => Self::handle_in_groups(builder, lexer, concatenation),
            BuilderState::InClasses { alternation, negation } => Self::handle_in_classes(builder, lexer, alternation, negation),
        }
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
        let mut state = BuilderState::InGroups { concatenation: false };

        loop {
            state = match state.handle(&mut self, &mut lexer) {
                BuilderState::Completed => break,
                state => state,
            };
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
    fn handles_rpn_from_optional_concatenated() {
        let regex = b"(st)?op\0".as_ptr();
        let builder = Builder::<4096>::new();

        let regex = match builder.build(regex) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l2st?l2op&");
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

    #[test]
    fn handles_rpn_from_nested_groups() {
        let regex = b"(0|(0+1+)+)+\0".as_ptr();
        let builder = Builder::<4096>::new();

        let regex = match builder.build(regex) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l10l10+l11+&+|+");
    }

    #[test]
    fn handles_rpn_from_dividing_by_three_regex() {
        let regex = b"(0|(1(01*(00)*0)*1)*)*\0".as_ptr();
        let builder = Builder::<4096>::new();

        let regex = match builder.build(regex) {
            Some(regex) => regex,
            None => return assert!(false),
        };

        assert_eq!(regex.as_bytes(), b"l10l11l10l11+?l200+?l10&&&+?l11&&+?|+?");
    }
}
