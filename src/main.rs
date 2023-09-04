mod bitstream;
mod commands;
mod huffman;
mod inflate;

use structopt::StructOpt;
use crate::commands::Cli;

fn main() {
    let command = Cli::from_args();
    let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();

    let result = runtime.block_on(async {
        match command {
            Cli::Decompress(command) => command.handle().await,
            Cli::Block(command) => command.handle().await,
        }
    });

    if let Err(error) = result {
        println!("{}", error);
    }
}
