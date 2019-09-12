mod app;

use app::App;
use colored::Colorize;
use env_logger::{Builder, Env};
use gwasm_api::prelude::Timeout;
use std::{convert::TryInto, path::PathBuf};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "g_flite",
    author = "Golem RnD Team <contact@golem.network>",
    about = "flite, a text-to-speech program, distributed over Golem network"
)]
struct Opt {
    /// Input text file
    #[structopt(parse(from_os_str))]
    input: PathBuf,

    /// Output WAV file
    #[structopt(parse(from_os_str))]
    output: PathBuf,

    /// Sets number of Golem subtasks
    #[structopt(long = "subtasks", default_value = "6")]
    subtasks: u64,

    /// Sets bid value for Golem task
    #[structopt(long = "bid", default_value = "1.0")]
    bid: f64,

    /// Sets Golem's task timeout value
    #[structopt(long = "task_timeout", parse(try_from_str), default_value = "00:10:00")]
    task_timeout: Timeout,

    /// Sets Golem's subtask timeout value
    #[structopt(
        long = "subtask_timeout",
        parse(try_from_str),
        default_value = "00:01:00"
    )]
    subtask_timeout: Timeout,

    /// Sets path to Golem datadir
    #[structopt(long = "datadir", parse(from_os_str))]
    datadir: Option<PathBuf>,

    /// Sets RPC address to Golem instance
    #[structopt(long = "address", default_value = "127.0.0.1")]
    address: String,

    /// Sets RPC port to Golem instance
    #[structopt(long = "port", default_value = "61000")]
    port: u16,

    /// Sets workspace dir
    ///
    /// This option is mainly used for debugging the gWasm task as it allows
    /// you to specify the exact path to the workspace where the contents of
    /// the entire gWasm task will be stored. Note that it will *not* be
    /// automatically removed after the app finishes successfully; instead,
    /// it is your responsibility to clean up after yourself.
    #[structopt(long = "workspace", parse(from_os_str))]
    workspace: Option<PathBuf>,

    /// Turns verbose logging on
    #[structopt(short = "v", long = "verbose")]
    verbose: bool,

    /// Configures golem-client to use mainnet datadir
    #[structopt(long)]
    mainnet: bool,
}

fn main() {
    let opt = Opt::from_args();

    if opt.verbose {
        Builder::from_env(Env::default().default_filter_or("info")).init();
    }

    if let Err(e) = opt.try_into().and_then(|app: App| app.run()) {
        eprintln!("{}", format!("An error occurred while {}", e).red())
    }
}
