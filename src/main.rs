mod cli;
mod databricks;
mod environment;
mod helpers;

use std::process;
use cli::CliError;
use structopt::StructOpt;

use self::cli::{handle_dbfs_get, handle_dbfs_list, Dbfs};

fn exit_with_error(error: CliError) -> ! {
    println!("{}", error);
    process::exit(-1);
}

#[tokio::main]
async fn main() {
    let result = match Dbfs::from_args() {
        Dbfs::Get(args) => handle_dbfs_get(args).await,
        Dbfs::List(args) => handle_dbfs_list(args).await,
    };

    match result {
        Err(error) => exit_with_error(error),
        Ok(_) => return,
    }
}
