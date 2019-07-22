mod app;
mod task;
mod timeout;

pub type Result<T> = std::result::Result<T, String>;

use self::app::{App, CompFragment};
use self::timeout::Timeout;
use env_logger::{Builder, Env};
use std::convert::TryInto;
use std::path::PathBuf;
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
    subtasks: usize,

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
        default_value = "00:10:00"
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

    /// Turns verbose logging on
    #[structopt(short = "v", long = "verbose")]
    verbose: bool,
}

fn main() {
    let opt = Opt::from_args();

    if opt.verbose {
        Builder::from_env(Env::default().default_filter_or("info")).init();
    }

    match opt.try_into().and_then(|app: App| app.run()) {
        Ok(CompFragment::Success(_)) => {
            // computation finished uninterrupted and results were pooled together successfully
            println!("Success")
        }
        Ok(CompFragment::CtrlC) => {
            // computation was cancelled by the user
            println!("Aborted by user (did you press ctrl-c?)")
        }
        Err(err) => {
            // unexpected error occurred
            eprintln!("An error occurred while {}", err)
        }
    }
}
