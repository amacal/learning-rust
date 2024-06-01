pub const APP_DELAY_FAILED: &'static [u8] = b"Timout: Delay Failed.";
pub const APP_MEMORY_ALLOC_FAILED: &'static [u8] = b"Memory: Cannot Allocate Memory.";
pub const APP_MEMORY_SLICE_FAILED: &'static [u8] = b"Memory: Slicing Failed.";
pub const APP_INTERNALLY_FAILED: &'static [u8] = b"Panic: Internally Failed.";

pub const APP_STDOUT_FAILED: &'static [u8] = b"Stdout: Cannot Write.";
pub const APP_STDOUT_INCOMPLETE: &'static [u8] = b"Stdout: Incomplete.";

pub const APP_PIPE_CREATING_FAILED: &'static [u8] = b"Pipe: Cannot Create Pipe.";
pub const APP_PIPE_WRITING_FAILED: &'static [u8] = b"Pipe: Cannot Write Pipe.";
pub const APP_PIPE_READING_FAILED: &'static [u8] = b"Pipe: Cannot Read Pipe.";

pub const APP_ARGS_FAILED: &'static [u8] = b"Args: Not enough Argument.";
pub const APP_SELECT_FAILED: &'static [u8] = b"Select: Selection Failed.";

pub const APP_FILE_OPENING_FAILED: &'static [u8] = b"File: Cannot Open File.";
pub const APP_FILE_READING_FAILED: &'static [u8] = b"File: Cannot Read File.";
pub const APP_FILE_WRITING_FAILED: &'static [u8] = b"File: Cannot Write File.";
pub const APP_FILE_CLOSING_FAILED: &'static [u8] = b"File: Cannot Close File.";
