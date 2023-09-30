mod adler32;
mod bitstream;
mod commands;
mod huffman;
mod inflate;
mod memory;
mod zlib;

use crate::commands::Cli;
use structopt::StructOpt;

use std::arch::x86_64::*;

fn main() {
    let mask = unsafe { _mm256_set_epi8(
        -128, 64, 32, 16, 8, 4, 2, 1,
        -128, 64, 32, 16, 8, 4, 2, 1,
        -128, 64, 32, 16, 8, 4, 2, 1,
        -128, 64, 32, 16, 8, 4, 2, 1,
    ) };

    let shuffle = unsafe { _mm256_set_epi8(
        3, 3, 3, 3, 3, 3, 3, 3, 2, 2, 2, 2, 2, 2, 2, 2,
        1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0
    ) };

    
    let input = [0, 1, 2, 4];
    let mut output = [0; 32];

    byte_to_bit_simd(&input, &mut output, &mask, &shuffle);
    println!("{:?}", output);

    let command = Cli::from_args();

    let result = match command {
        Cli::DecompressSync(command) => command.handle(),
        command => {
            let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();

            runtime.block_on(async {
                match command {
                    Cli::DecompressAsync(command) => command.handle().await,
                    Cli::Block(command) => command.handle().await,
                    _ => Ok(()),
                }
            })
        }
    };

    if let Err(error) = result {
        println!("{}", error);
    }
}

fn byte_to_bit_simd(input: &[u8], output: &mut [u8], mask: &__m256i, shuffle: &__m256i) {
    unsafe {
        let four_bytes = _mm_set_epi8(
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            input[3] as i8, input[2] as i8, input[1] as i8, input[0] as i8,
        );

        let broadcasted = _mm256_broadcastsi128_si256(four_bytes);
        let repeated_bytes = _mm256_shuffle_epi8(broadcasted, *shuffle);
        
        let and_result = _mm256_and_si256(repeated_bytes, *mask);    
        let comparison_result = _mm256_cmpeq_epi8(and_result, _mm256_setzero_si256());
        let shifted_bits = _mm256_xor_si256(comparison_result, _mm256_set1_epi8(-1i8));

        _mm256_storeu_si256(output.as_mut_ptr() as *mut __m256i, shifted_bits);
    }
}