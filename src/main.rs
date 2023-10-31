mod adler32;
mod bitstream;
mod commands;
mod huffman;
mod inflate;
mod zlib;

use crate::commands::Cli;
use structopt::StructOpt;

fn main() {
    let command = Cli::from_args();

    let result = match command {
        Cli::DecompressSync(command) => command.handle(),
        Cli::BitStream(command) => command.handle(),
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
