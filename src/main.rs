use std::fs::File;
use std::io::Read;

use gearhash::Hasher;
use ring::digest;

fn main() {
    let mut context = digest::Context::new(&digest::SHA512);
    let mut hasher = Hasher::default();
    let mut file = File::open("/home/vscode/enwiki-20230701-pages-meta-history1.xml-p1p844").unwrap();

    let mask = 0x0000_0000_0000_ffff;
    let mut buffer = vec![0; 1048576];

    let mut total = 0;
    let mut offset = 0;
    let mut previous = 0;

    while let Ok(count) = file.read(&mut buffer) {
        while count > offset {
            match hasher.next_match(&buffer[offset..count], mask) {
                None => {
                    context.update(&buffer[offset..count]);
                    offset = count;
                }
                Some(boundary) => {
                    let next = total + offset + boundary;
                    let diff = next - previous;

                    context.update(&buffer[offset..offset + boundary]);

                    if diff >= 1048576 {
                        let digest = format!("{:?}", context.finish());
                        println!("{:>16x} {:.20} {:>10}", hasher.get_hash(), digest, diff);

                        context = digest::Context::new(&digest::SHA512);
                        hasher.set_hash(0);
                        previous = next;
                    }

                    offset += boundary;
                }
            }
        }

        if count == 0 {
            break;
        }

        total += count;
        offset = 0;
    }

    if offset > 0 {
        let digest = format!("{:?}", context.finish());
        println!("{:>16x} {:.20} {:>10}", hasher.get_hash(), digest, total - previous);
    }
}
