mod array;
mod dfa;
mod graph;
mod heap;
mod lexer;
mod list;
mod nfa;
mod rpn;

use dfa::*;
use nfa::*;
use rpn::*;

fn main() {
    let regex = b"[a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?.[a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?(.[a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?)*\0".as_ptr();

    let rpn: RPN<4096> = match RPN::build(regex) {
        Some(rpn) => rpn,
        None => return assert!(false),
    };

    let nfa = match NFA::build(rpn) {
        Some(nfa) => nfa,
        None => return assert!(false),
    };

    println!("nfa, transitions={}", nfa.transition_count());
    nfa.print();

    let dfa = match DFA::build(nfa) {
        Some(dfa) => dfa,
        None => return assert!(false),
    };

    println!("dfa, transitions={}", dfa.transition_count());
    dfa.print();

    println!("{:?}", dfa.traverse(b"www.wp.pl", 0));
}
