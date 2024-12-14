#[derive(Debug, PartialEq)]
pub enum Error {
    NotMatched {},
    InvalidRegex {},
    InternallyFailed {},
    NotEnoughHeap {},
}
