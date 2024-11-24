mod json;
mod regex;

use json::Lexer;
use std::fs::File;
use std::io::Read;

fn main() {
    let mut data = [0u8; 1024];

    let mut lexer = match Lexer::new(data.as_ptr(), 1023) {
        None => return,
        Some(lexer) => lexer,
    };

    let mut file = match File::open("test.json") {
        Err(_) => return,
        Ok(file) => file,
    };

    let mut prev = 0;
    let mut total = 0;
    let mut completed = false;
    let mut counters = [0; 16];

    loop {
        let next = if total % 1024 >= prev % 1024 { total % 1024..1024 } else { 0..prev % 1024 };
        let read = match file.read(&mut data[next.clone()]) {
            Err(_) => return,
            Ok(0) => {
                data[next][0] = 0;
                completed = true;
                1
            }
            Ok(n) => n,
        };

        total = total.wrapping_add(read);
        let mut length = total.wrapping_sub(prev);

        while let Some((token, size)) = lexer.process(length) {
            print!("{:08x} ", prev);

            match (token, size) {
                (1, _) => println!("open-object"),
                (2, _) => println!("close-object"),
                (3, _) => println!("open-array"),
                (4, _) => println!("close-array"),
                (5, _) => println!("comma"),
                (6, _) => println!("colon"),
                (7, len) => println!("white, len={}", len),
                (8, len) => println!(
                    "string, len={}, val=[{:?} {:?}]",
                    len,
                    std::str::from_utf8(&data[prev % 1024..std::cmp::min(1024, (prev % 1024) + len)]),
                    std::str::from_utf8(&data[0..std::cmp::max(0, ((prev % 1024) + len) as isize - 1023) as usize])
                ),
                (9, len) => println!("number, len={}", len),
                (10, _) => println!("false"),
                (11, _) => println!("true"),
                (12, _) => println!("null"),
                (_, _) => panic!("unknown"),
            }

            prev = lexer.offset();
            length = length.wrapping_sub(size);
            counters[token as usize] += 1;
        }

        if completed {
            break;
        }
    }

    if total == prev + 1 {
        println!();
        for (idx, &val) in counters.iter().enumerate() {
            println!("{idx:02x} {val}");
        }
    }
}
