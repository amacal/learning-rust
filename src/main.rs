use std::env::args;

use chrono::Local;
use rand::RngCore;

#[derive(Debug, Clone, Copy)]
struct DataRow {
    key: u64,
    index: u32,
}

impl Default for DataRow {
    fn default() -> Self {
        DataRow { key: 0, index: 0 }
    }
}

fn log(msg: &str) {
    println!("[{}] {}", Local::now().format("%Y-%m-%d %H:%M:%S%.3f"), msg);
}

fn builtin() {
    const SIZE: usize = 512 * 1024 * 1024;
    let mut rng = rand::rng();

    log("Creating array...");
    let rows: Vec<DataRow> = vec![DataRow::default(); SIZE];
    let mut rows: Box<[DataRow]> = rows.into_boxed_slice();

    log("Generating rows...");
    for (idx, row) in rows.iter_mut().enumerate() {
        row.key = rng.next_u64();
        row.index = idx as u32;
    }

    log("Sorting rows...");
    rows.sort_by_key(|row| row.key);

    unsafe {
        log("Verifying...");
        for idx in 1..SIZE {
            if rows.get_unchecked(idx).key < rows.get_unchecked(idx - 1).key {
                log(format!("Verification failed at {}!", idx).as_str());
                break;
            }
        }
    }

    log("Done");
}

fn radix4() {
    const PASSES: usize = 16;
    const BITS: usize = 4;
    const BUCKETS: usize = 16;

    const SIZE: usize = 512 * 1024 * 1024;
    let mut rng = rand::rng();

    log("Creating array...");
    let keys1: Vec<u64> = vec![0; SIZE];
    let mut keys1: Box<[u64]> = keys1.into_boxed_slice();

    let values1: Vec<u32> = vec![0; SIZE];
    let mut values1: Box<[u32]> = values1.into_boxed_slice();

    let keys2: Vec<u64> = vec![0; SIZE];
    let mut keys2: Box<[u64]> = keys2.into_boxed_slice();

    let values2: Vec<u32> = vec![0; SIZE];
    let mut values2: Box<[u32]> = values2.into_boxed_slice();

    let histograms: Vec<u32> = vec![0; PASSES * BUCKETS];
    let mut histograms: Box<[u32]> = histograms.into_boxed_slice();

    unsafe {
        log("Generating rows...");
        for idx in 0..SIZE {
            *keys1.get_unchecked_mut(idx) = rng.next_u64();
            *values1.get_unchecked_mut(idx) = idx as u32;
        }
    }

    unsafe {
        log("Building histograms...");
        for &key in keys1.iter() {
            let i0 = (key & 0x000000000000000fu64) >> 0;
            let i1 = (key & 0x00000000000000f0u64) >> 4;
            let i2 = (key & 0x0000000000000f00u64) >> 8;
            let i3 = (key & 0x000000000000f000u64) >> 12;
            let i4 = (key & 0x00000000000f0000u64) >> 16;
            let i5 = (key & 0x0000000000f00000u64) >> 20;
            let i6 = (key & 0x000000000f000000u64) >> 24;
            let i7 = (key & 0x00000000f0000000u64) >> 28;
            let i8 = (key & 0x0000000f00000000u64) >> 32;
            let i9 = (key & 0x000000f000000000u64) >> 36;
            let i10 = (key & 0x00000f0000000000u64) >> 40;
            let i11 = (key & 0x0000f00000000000u64) >> 44;
            let i12 = (key & 0x000f000000000000u64) >> 48;
            let i13 = (key & 0x00f0000000000000u64) >> 52;
            let i14 = (key & 0x0f00000000000000u64) >> 56;
            let i15 = (key & 0xf000000000000000u64) >> 60;

            *histograms.get_unchecked_mut(0 * BUCKETS + i0 as usize) += 1;
            *histograms.get_unchecked_mut(1 * BUCKETS + i1 as usize) += 1;
            *histograms.get_unchecked_mut(2 * BUCKETS + i2 as usize) += 1;
            *histograms.get_unchecked_mut(3 * BUCKETS + i3 as usize) += 1;
            *histograms.get_unchecked_mut(4 * BUCKETS + i4 as usize) += 1;
            *histograms.get_unchecked_mut(5 * BUCKETS + i5 as usize) += 1;
            *histograms.get_unchecked_mut(6 * BUCKETS + i6 as usize) += 1;
            *histograms.get_unchecked_mut(7 * BUCKETS + i7 as usize) += 1;
            *histograms.get_unchecked_mut(8 * BUCKETS + i8 as usize) += 1;
            *histograms.get_unchecked_mut(9 * BUCKETS + i9 as usize) += 1;
            *histograms.get_unchecked_mut(10 * BUCKETS + i10 as usize) += 1;
            *histograms.get_unchecked_mut(11 * BUCKETS + i11 as usize) += 1;
            *histograms.get_unchecked_mut(12 * BUCKETS + i12 as usize) += 1;
            *histograms.get_unchecked_mut(13 * BUCKETS + i13 as usize) += 1;
            *histograms.get_unchecked_mut(14 * BUCKETS + i14 as usize) += 1;
            *histograms.get_unchecked_mut(15 * BUCKETS + i15 as usize) += 1;
        }

        for i in 0..PASSES {
            let mut sum = 0;
            let offset = i * BUCKETS;

            for j in 0..BUCKETS {
                let val = *histograms.get_unchecked(offset + j);
                *histograms.get_unchecked_mut(offset + j) = sum;
                sum += val;
            }
        }
    }

    for i in 0..PASSES {
        let offset = i * BUCKETS;
        let mask = 0x000f << (i * BITS);

        unsafe {
            log(format!("Pass {}...", i).as_str());

            for idx in 0..SIZE {
                let key = *keys1.get_unchecked(idx);
                let value = *values1.get_unchecked(idx);

                let bucket = ((key & mask) >> (i * BITS)) as usize;
                let index = histograms.get_unchecked_mut(offset + bucket);

                *keys2.get_unchecked_mut(*index as usize) = key;
                *values2.get_unchecked_mut(*index as usize) = value;
                *index += 1;
            }

            std::mem::swap(&mut keys1, &mut keys2);
            std::mem::swap(&mut values1, &mut values2);
        }
    }

    unsafe {
        log("Verifying...");
        for idx in 1..SIZE {
            if *keys1.get_unchecked(idx) < *keys1.get_unchecked(idx - 1) {
                log(format!("Verification failed at {}!", idx).as_str());
                break;
            }
        }
    }

    log("Done");
}

fn radix8() {
    const BITS: usize = 8;
    const PASSES: usize = 8;
    const BUCKETS: usize = 256;

    const SIZE: usize = 512 * 1024 * 1024;
    let mut rng = rand::rng();

    log("Creating array...");
    let keys1: Vec<u64> = vec![0; SIZE];
    let mut keys1: Box<[u64]> = keys1.into_boxed_slice();

    let values1: Vec<u32> = vec![0; SIZE];
    let mut values1: Box<[u32]> = values1.into_boxed_slice();

    let keys2: Vec<u64> = vec![0; SIZE];
    let mut keys2: Box<[u64]> = keys2.into_boxed_slice();

    let values2: Vec<u32> = vec![0; SIZE];
    let mut values2: Box<[u32]> = values2.into_boxed_slice();

    let histograms: Vec<u32> = vec![0; PASSES * BUCKETS];
    let mut histograms: Box<[u32]> = histograms.into_boxed_slice();

    unsafe {
        log("Generating rows...");
        for idx in 0..SIZE {
            *keys1.get_unchecked_mut(idx) = rng.next_u64();
            *values1.get_unchecked_mut(idx) = idx as u32;
        }
    }

    unsafe {
        log("Building histograms...");
        for &key in keys1.iter() {
            let i0 = (key & 0x00000000000000ffu64) >> 0;
            let i1 = (key & 0x000000000000ff00u64) >> 8;
            let i2 = (key & 0x0000000000ff0000u64) >> 16;
            let i3 = (key & 0x00000000ff000000u64) >> 24;
            let i4 = (key & 0x000000ff00000000u64) >> 32;
            let i5 = (key & 0x0000ff0000000000u64) >> 40;
            let i6 = (key & 0x00ff000000000000u64) >> 48;
            let i7 = (key & 0xff00000000000000u64) >> 56;

            *histograms.get_unchecked_mut(0 * BUCKETS + i0 as usize) += 1;
            *histograms.get_unchecked_mut(1 * BUCKETS + i1 as usize) += 1;
            *histograms.get_unchecked_mut(2 * BUCKETS + i2 as usize) += 1;
            *histograms.get_unchecked_mut(3 * BUCKETS + i3 as usize) += 1;
            *histograms.get_unchecked_mut(4 * BUCKETS + i4 as usize) += 1;
            *histograms.get_unchecked_mut(5 * BUCKETS + i5 as usize) += 1;
            *histograms.get_unchecked_mut(6 * BUCKETS + i6 as usize) += 1;
            *histograms.get_unchecked_mut(7 * BUCKETS + i7 as usize) += 1;
        }

        for i in 0..PASSES {
            let mut sum = 0;
            let offset = i * BUCKETS;

            for j in 0..BUCKETS {
                let val = *histograms.get_unchecked(offset + j);
                *histograms.get_unchecked_mut(offset + j) = sum;
                sum += val;
            }
        }
    }

    for i in 0..PASSES {
        let offset = i * BUCKETS;
        let mask = 0x00ff << (i * BITS);

        unsafe {
            log(format!("Pass {}...", i).as_str());

            for idx in 0..SIZE {
                let key = *keys1.get_unchecked(idx);
                let value = *values1.get_unchecked(idx);

                let bucket = ((key & mask) >> (i * BITS)) as usize;
                let index = histograms.get_unchecked_mut(offset + bucket);

                *keys2.get_unchecked_mut(*index as usize) = key;
                *values2.get_unchecked_mut(*index as usize) = value;
                *index += 1;
            }

            std::mem::swap(&mut keys1, &mut keys2);
            std::mem::swap(&mut values1, &mut values2);
        }
    }

    unsafe {
        log("Verifying...");
        for idx in 1..SIZE {
            if *keys1.get_unchecked(idx) < *keys1.get_unchecked(idx - 1) {
                log(format!("Verification failed at {}!", idx).as_str());
                break;
            }
        }
    }

    log("Done");
}

fn radix12() {
    const PASSES: usize = 6;
    const BITS: usize = 12;
    const BUCKETS: usize = 16 * 256;

    const SIZE: usize = 512 * 1024 * 1024;
    let mut rng = rand::rng();

    log("Creating array...");
    let keys1: Vec<u64> = vec![0; SIZE];
    let mut keys1: Box<[u64]> = keys1.into_boxed_slice();

    let values1: Vec<u32> = vec![0; SIZE];
    let mut values1: Box<[u32]> = values1.into_boxed_slice();

    let keys2: Vec<u64> = vec![0; SIZE];
    let mut keys2: Box<[u64]> = keys2.into_boxed_slice();

    let values2: Vec<u32> = vec![0; SIZE];
    let mut values2: Box<[u32]> = values2.into_boxed_slice();

    let histograms: Vec<u32> = vec![0; PASSES * BUCKETS];
    let mut histograms: Box<[u32]> = histograms.into_boxed_slice();

    unsafe {
        log("Generating rows...");
        for idx in 0..SIZE {
            *keys1.get_unchecked_mut(idx) = rng.next_u64();
            *values1.get_unchecked_mut(idx) = idx as u32;
        }
    }

    unsafe {
        log("Building histograms...");
        for &key in keys1.iter() {
            let i0 = (key & 0x0000000000000fffu64) >> 0;
            let i1 = (key & 0x0000000000fff000u64) >> 12;
            let i2 = (key & 0x0000000fff000000u64) >> 24;
            let i3 = (key & 0x0000fff000000000u64) >> 36;
            let i4 = (key & 0x0fff000000000000u64) >> 48;
            let i5 = (key & 0xf000000000000000u64) >> 60;

            *histograms.get_unchecked_mut(0 * BUCKETS + i0 as usize) += 1;
            *histograms.get_unchecked_mut(1 * BUCKETS + i1 as usize) += 1;
            *histograms.get_unchecked_mut(2 * BUCKETS + i2 as usize) += 1;
            *histograms.get_unchecked_mut(3 * BUCKETS + i3 as usize) += 1;
            *histograms.get_unchecked_mut(4 * BUCKETS + i4 as usize) += 1;
            *histograms.get_unchecked_mut(5 * BUCKETS + i5 as usize) += 1;
        }

        for i in 0..PASSES {
            let mut sum = 0;
            let offset = i * BUCKETS;

            for j in 0..BUCKETS {
                let val = *histograms.get_unchecked(offset + j);
                *histograms.get_unchecked_mut(offset + j) = sum;
                sum += val;
            }
        }
    }

    for i in 0..PASSES {
        let offset = i * BUCKETS;
        let mask = 0x0fff << (i * BITS);

        unsafe {
            log(format!("Pass {}...", i).as_str());

            for idx in 0..SIZE {
                let key = *keys1.get_unchecked(idx);
                let value = *values1.get_unchecked(idx);

                let bucket = ((key & mask) >> (i * BITS)) as usize;
                let index = histograms.get_unchecked_mut(offset + bucket);

                *keys2.get_unchecked_mut(*index as usize) = key;
                *values2.get_unchecked_mut(*index as usize) = value;
                *index += 1;
            }

            std::mem::swap(&mut keys1, &mut keys2);
            std::mem::swap(&mut values1, &mut values2);
        }
    }

    unsafe {
        log("Verifying...");
        for idx in 1..SIZE {
            if *keys1.get_unchecked(idx) < *keys1.get_unchecked(idx - 1) {
                log(format!("Verification failed at {}!", idx).as_str());
                break;
            }
        }
    }

    log("Done");
}

fn radix16() {
    const PASSES: usize = 4;
    const BITS: usize = 16;
    const BUCKETS: usize = 256 * 256;

    const SIZE: usize = 512 * 1024 * 1024;
    let mut rng = rand::rng();

    log("Creating array...");
    let keys1: Vec<u64> = vec![0; SIZE];
    let mut keys1: Box<[u64]> = keys1.into_boxed_slice();

    let values1: Vec<u32> = vec![0; SIZE];
    let mut values1: Box<[u32]> = values1.into_boxed_slice();

    let keys2: Vec<u64> = vec![0; SIZE];
    let mut keys2: Box<[u64]> = keys2.into_boxed_slice();

    let values2: Vec<u32> = vec![0; SIZE];
    let mut values2: Box<[u32]> = values2.into_boxed_slice();

    let histograms: Vec<u32> = vec![0; 4 * BUCKETS];
    let mut histograms: Box<[u32]> = histograms.into_boxed_slice();

    unsafe {
        log("Generating rows...");
        for idx in 0..SIZE {
            *keys1.get_unchecked_mut(idx) = rng.next_u64();
            *values1.get_unchecked_mut(idx) = idx as u32;
        }
    }

    unsafe {
        log("Building histograms...");
        for &key in keys1.iter() {
            let i0 = (key & 0x000000000000ffffu64) >> 0;
            let i1 = (key & 0x00000000ffff0000u64) >> 16;
            let i2 = (key & 0x0000ffff00000000u64) >> 32;
            let i3 = (key & 0xffff000000000000u64) >> 48;

            *histograms.get_unchecked_mut(0 * BUCKETS + i0 as usize) += 1;
            *histograms.get_unchecked_mut(1 * BUCKETS + i1 as usize) += 1;
            *histograms.get_unchecked_mut(2 * BUCKETS + i2 as usize) += 1;
            *histograms.get_unchecked_mut(3 * BUCKETS + i3 as usize) += 1;
        }

        for i in 0..PASSES {
            let mut sum = 0;
            let offset = i * BUCKETS;

            for j in 0..BUCKETS {
                let val = *histograms.get_unchecked(offset + j);
                *histograms.get_unchecked_mut(offset + j) = sum;
                sum += val;
            }
        }
    }

    for i in 0..PASSES {
        let offset = i * BUCKETS;
        let mask = 0xffff << (i * BITS);

        unsafe {
            log(format!("Pass {}...", i).as_str());

            for idx in 0..SIZE {
                let key = *keys1.get_unchecked(idx);
                let value = *values1.get_unchecked(idx);

                let bucket = ((key & mask) >> (i * BITS)) as usize;
                let index = histograms.get_unchecked_mut(offset + bucket);

                *keys2.get_unchecked_mut(*index as usize) = key;
                *values2.get_unchecked_mut(*index as usize) = value;
                *index += 1;
            }

            std::mem::swap(&mut keys1, &mut keys2);
            std::mem::swap(&mut values1, &mut values2);
        }
    }

    unsafe {
        log("Verifying...");
        for idx in 1..SIZE {
            if *keys1.get_unchecked(idx) < *keys1.get_unchecked(idx - 1) {
                log(format!("Verification failed at {}!", idx).as_str());
                break;
            }
        }
    }

    log("Done");
}

fn main() {
    if args().len() > 1 {
        let mode = args().nth(1).unwrap();
        match mode.as_str() {
            "builtin" => builtin(),
            "radix4" => radix4(),
            "radix8" => radix8(),
            "radix12" => radix12(),
            "radix16" => radix16(),
            _ => eprintln!("Unknown mode: {}", mode),
        }
    } else {
        eprintln!("Usage: {} <mode>", args().nth(0).unwrap());
    }
}
