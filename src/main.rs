mod databricks;
mod environment;
mod jupyter;
mod kernel;

use log::{error, info};
use structopt::StructOpt;

#[derive(structopt::StructOpt, Debug)]
pub struct KernelStart {
    #[structopt(help = "The absolute path of the Kernel connection file.")]
    pub path: String,
}

fn exit_with_error(error: kernel::KernelError) -> ! {
    error!("{}", error);
    std::process::exit(-1);
}

fn initialize_loging() -> () {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{} [{}] {}",
                chrono::Local::now().format("[%Y-%m-%d %H:%M:%S]"),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .level_for(module_path!{}, log::LevelFilter::Debug)
        .chain(fern::log_file("/workspaces/learning-rust/output.log").unwrap())
        .apply()
        .unwrap();
}

#[tokio::main]
async fn main() {
    initialize_loging();

    let args = KernelStart::from_args();
    info!("Args {:?}", args);

    let mut kernel = match kernel::KernelClient::start(&args.path).await {
        Ok(kernel) => kernel,
        Err(error) => exit_with_error(error),
    };

    loop {
        tokio::select! {
            _ = kernel.recv() => () 
        }
    }
}