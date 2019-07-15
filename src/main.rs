mod task;
mod timeout;

pub type Result<T> = std::result::Result<T, String>;

use console::style;
use env_logger::{Builder, Env};
use golem_rpc_api::comp::{self, AsGolemComp};
use hound;
use indicatif::ProgressBar;
use std::env;
use std::fs;
use std::io::Read;
use std::path;
use std::process;
use std::time::SystemTime;
use structopt::StructOpt;
use timeout::Timeout;

static TRUCK: &str = "🚚  ";
static CLIP: &str = "🔗  ";
static PAPER: &str = "📃  ";
static HOURGLASS: &str = "⌛  ";

#[derive(Debug, StructOpt)]
#[structopt(
    name = "g_flite",
    author = "Golem RnD Team <contact@golem.network>",
    about = "flite, a text-to-speech program, distributed over Golem network"
)]
struct Opt {
    /// Input text file
    #[structopt(parse(from_os_str))]
    input: path::PathBuf,

    /// Output WAV file
    #[structopt(parse(from_os_str))]
    output: path::PathBuf,

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
    datadir: Option<path::PathBuf>,

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

#[derive(Debug)]
pub struct GolemOpt {
    bid: f64,
    task_timeout: Timeout,
    subtask_timeout: Timeout,
}

impl From<&Opt> for GolemOpt {
    fn from(opt: &Opt) -> Self {
        Self {
            bid: opt.bid,
            task_timeout: opt.task_timeout,
            subtask_timeout: opt.subtask_timeout,
        }
    }
}

fn split_textfile(textfile: &str, num_subtasks: usize) -> Result<Vec<String>> {
    let mut contents = String::new();
    fs::File::open(textfile)
        .and_then(|mut f| f.read_to_string(&mut contents))
        .map_err(|e| format!("reading from '{}': {}", textfile, e))?;

    let word_count = contents.split_whitespace().count();

    log::info!("Input text file has {} words", word_count);

    println!(
        "{} {}Splitting '{}' into {} Golem subtasks...",
        style("[1/4]").bold().dim(),
        PAPER,
        textfile,
        num_subtasks,
    );

    let mut chunks = Vec::with_capacity(num_subtasks);
    let num_words = (word_count as f64 / num_subtasks as f64).ceil() as usize;

    log::info!("Each chunk will have max {} words", num_words);

    let mut acc = Vec::with_capacity(num_words);
    for word in contents.split_whitespace() {
        acc.push(word);

        if acc.len() == num_words {
            chunks.push(acc);
            acc = Vec::with_capacity(num_words);
            continue;
        }
    }

    if !acc.is_empty() {
        chunks.push(acc);
    }

    if log::log_enabled!(log::Level::Info) {
        for (i, chunk) in chunks.iter().enumerate() {
            log::info!("Chunk {} has {} words", i, chunk.len(),);
        }
    }

    Ok(chunks.into_iter().map(|chunk| chunk.join(" ")).collect())
}

fn run_on_golem<S: AsRef<path::Path>>(
    chunks: Vec<String>,
    datadir: S,
    address: &str,
    port: u16,
    golem_opt: GolemOpt,
) -> Result<task::Task> {
    println!(
        "{} {}Sending task to Golem...",
        style("[2/4]").bold().dim(),
        TRUCK
    );

    // prepare workspace
    let mut workspace = env::temp_dir();
    let time_now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| e.to_string())?;
    let subdir = format!("g_flite_{}", time_now.as_secs());
    workspace.push(subdir);
    fs::create_dir(workspace.as_path()).map_err(|e| {
        format!(
            "creating directory '{}': {}",
            workspace.to_string_lossy(),
            e
        )
    })?;

    log::info!("Will prepare task in '{:?}'", workspace);

    // prepare Golem task
    let mut task_builder = task::TaskBuilder::new(workspace, golem_opt);

    for chunk in chunks {
        task_builder.add_subtask(chunk);
    }

    let task = task_builder.build()?;

    // connect to Golem
    let mut sys = actix::System::new("g-flite");
    let endpoint = sys
        .block_on(golem_rpc_api::connect_to_app(
            datadir.as_ref(),
            Some(golem_rpc_api::Net::TestNet),
            Some((address, port)),
        ))
        .map_err(|e| {
            format!(
                "connecting to Golem with datadir='{}', net='{}', address='{}:{}': {}",
                datadir.as_ref().to_string_lossy(),
                golem_rpc_api::Net::TestNet,
                address,
                port,
                e
            )
        })?;

    // TODO check if account is unlocked
    // TODO check if terms are accepted

    let resp = sys
        .block_on(endpoint.as_golem_comp().create_task(task.json.clone()))
        .map_err(|e| format!("creating Golem task '{:#?}': {}", task.json, e))?;
    let task_id = resp.0.ok_or("extracting Golem task's id".to_owned())?;

    // wait
    println!(
        "{} {}Waiting on compute to finish...",
        style("[3/4]").bold().dim(),
        HOURGLASS
    );
    let num_tasks = task.expected_output_paths.len() as u64;
    let bar = ProgressBar::new(num_tasks);
    bar.inc(0);
    let mut old_progress = 0.0;

    loop {
        let resp = sys
            .block_on(endpoint.as_golem_comp().get_task(task_id.clone()))
            .map_err(|e| format!("polling for task '{:#?}': {}", task.json, e))?;
        let task_info = resp.ok_or("parsing task info from Golem".to_owned())?;

        log::info!("Received task info from Golem: {:?}", task_info);

        let progress = task_info
            .progress
            .ok_or("reading task's progress".to_owned())?
            * 100.0;

        if progress != old_progress {
            let delta = (progress - old_progress) / 100.0;
            old_progress = progress;
            bar.inc((delta * num_tasks as f64).round() as u64);
        }

        match task_info.status {
            comp::TaskStatus::Restarted => {
                // reset progress bar
                bar.inc(0);
                old_progress = 0.0;
            }
            comp::TaskStatus::Aborted => {
                return Err("waiting for task to complete: task aborted".to_owned())
            }
            comp::TaskStatus::Timeout => {
                return Err("waiting for task to complete: task timed out".to_owned())
            }
            comp::TaskStatus::Finished => break,
            _ => {}
        }

        std::thread::sleep(std::time::Duration::from_secs(1));
    }

    Ok(task)
}

fn combine_wave(mut task: task::Task, output_wavefile: &str) -> Result<()> {
    let first = task
        .expected_output_paths
        .pop_front()
        .ok_or("combining results: no results received from Golem".to_owned())?;

    println!(
        "{} {}Combining output into '{}'...",
        style("[4/4]").bold().dim(),
        CLIP,
        output_wavefile
    );

    let reader = hound::WavReader::open(&first)
        .map_err(|e| format!("opening WAVE file '{}': {}", first.to_string_lossy(), e))?;
    let spec = reader.spec();

    log::info!("Using Wav spec: {:?}", spec);

    let mut writer = hound::WavWriter::create(output_wavefile, spec)
        .map_err(|e| format!("creating WAVE file '{}': {}", output_wavefile, e))?;
    for sample in reader.into_samples::<i16>() {
        sample
            .and_then(|sample| writer.write_sample(sample))
            .map_err(|e| {
                format!(
                    "reading audio sample from file '{}': {}",
                    first.to_string_lossy(),
                    e
                )
            })?;
    }

    for expected_file in task.expected_output_paths {
        let reader = hound::WavReader::open(&expected_file).map_err(|e| {
            format!(
                "opening WAVE file '{}': {}",
                expected_file.to_string_lossy(),
                e
            )
        })?;
        for sample in reader.into_samples::<i16>() {
            sample
                .and_then(|sample| writer.write_sample(sample))
                .map_err(|e| {
                    format!(
                        "reading audio sample from file '{}': {}",
                        expected_file.to_string_lossy(),
                        e
                    )
                })?;
        }
    }

    Ok(())
}

fn main() {
    let opt = Opt::from_args();

    // unpack config args
    let golem_opt: GolemOpt = (&opt).into();
    let subtasks = opt.subtasks;
    let address = opt.address;
    let port = opt.port;
    let datadir = opt.datadir.unwrap_or_else(|| {
        match appdirs::user_data_dir(Some("golem"), Some("golem"), false) {
            Ok(data_dir) => data_dir.join("default"),
            Err(_) => {
                eprintln!(
                    "No standard project app data dirs available. Are you running a supported OS?"
                );
                process::exit(1);
            }
        }
    });

    if opt.verbose {
        Builder::from_env(Env::default().default_filter_or("info")).init();
    }

    // verify input exists
    let input = opt.input.to_string_lossy().to_owned();
    if !opt.input.is_file() {
        eprintln!(
            "Input file '{}' doesn't exist. Did you make a typo anywhere?",
            input
        );
        process::exit(1);
    }

    // verify output path excluding topmost file exists
    if let Some(parent) = opt.output.parent() {
        let parent_str = parent.to_string_lossy();
        if !parent_str.is_empty() && !parent.exists() {
            eprintln!(
                "Output path '{}' doesn't exist. Did you make a type anywhere?",
                parent_str,
            );
            process::exit(1);
        }
    }
    let output = opt.output.to_string_lossy().to_owned();

    if let Err(err) = split_textfile(&input, subtasks)
        .and_then(|chunks| run_on_golem(chunks, datadir, &address, port, golem_opt))
        .and_then(|task| combine_wave(task, &output))
    {
        eprintln!("An error occurred while {}", err);
        process::exit(1);
    }
}
