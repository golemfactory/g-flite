mod task;

pub type Result<T> = std::result::Result<T, String>;

use clap::{value_t, App, Arg};
use console::style;
use directories::ProjectDirs;
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

const DEFAULT_NUM_SUBTASKS: usize = 6;

static TRUCK: &str = "🚚  ";
static CLIP: &str = "🔗  ";
static PAPER: &str = "📃  ";
static HOURGLASS: &str = "⌛  ";

fn split_textfile(textfile: &str, num_subtasks: usize) -> Result<Vec<String>> {
    let mut contents = String::new();
    fs::File::open(textfile)
        .and_then(|mut f| f.read_to_string(&mut contents))
        .map_err(|e| format!("reading from '{}': {}", textfile, e))?;

    let word_count = contents.split_whitespace().count();

    log::info!("Each chunk will have max of {} words", word_count);

    println!(
        "{} {}Splitting '{}' into {} Golem subtasks...",
        style("[1/4]").bold().dim(),
        PAPER,
        textfile,
        num_subtasks,
    );

    let mut chunks: Vec<String> = Vec::with_capacity(num_subtasks);
    let num_words = (word_count as f64 / num_subtasks as f64).round() as usize;

    let mut acc: String = String::new();
    for (i, word) in contents.split_whitespace().enumerate() {
        acc.push_str(word);
        acc.push(' ');

        if (i + 1) % num_words == 0 {
            if log::log_enabled!(log::Level::Info) {
                log::info!(
                    "Chunk {} has {} words",
                    chunks.len(),
                    acc.split_whitespace().count()
                );
            }

            chunks.push(acc);
            acc = String::new();
            continue;
        }
    }

    if !acc.is_empty() {
        if log::log_enabled!(log::Level::Info) {
            log::info!(
                "Chunk {} has {} words",
                chunks.len(),
                acc.split_whitespace().count()
            );
        }

        chunks.push(acc);
    }

    Ok(chunks)
}

fn run_on_golem<S: AsRef<path::Path>>(
    chunks: Vec<String>,
    datadir: S,
    address: &str,
    port: u16,
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
    let mut task_builder = task::TaskBuilder::new(workspace);

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
    let matches = App::new("g_flite")
        .version("0.1.0")
        .author("Golem RnD Team <contact@golem.network>")
        .about("flite, a text-to-speech program, distributed over Golem network")
        .arg(
            Arg::with_name("TEXTFILE")
                .help("Input text file")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::with_name("WAVFILE")
                .help("Output WAV file")
                .required(true)
                .index(2),
        )
        .arg(
            Arg::with_name("subtasks")
                .long("subtasks")
                .value_name("NUM")
                .help("Sets number of Golem subtasks (defaults to 6)")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("datadir")
                .long("datadir")
                .value_name("DATADIR")
                .help("Sets path to Golem datadir")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("address")
                .long("address")
                .value_name("ADDRESS")
                .help("Sets RPC address to Golem instance")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("port")
                .long("port")
                .value_name("PORT")
                .help("Sets RPC port to Golem instance")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("verbose")
                .long("verbose")
                .short("v")
                .help("Turns verbose logging on")
                .takes_value(false),
        )
        .get_matches();

    let subtasks = value_t!(matches.value_of("subtasks"), usize).unwrap_or(DEFAULT_NUM_SUBTASKS);
    let address = matches.value_of("address").unwrap_or("127.0.0.1");
    let port = value_t!(matches.value_of("port"), u16).unwrap_or(61000);

    let datadir = value_t!(matches.value_of("datadir"), path::PathBuf).unwrap_or_else(|_| {
        match ProjectDirs::from("", "", "golem") {
            Some(project_dirs) => project_dirs.data_local_dir().join("default"),
            None => {
                eprintln!(
                    "No standard project app data dirs available. Are you running a supported OS?"
                );
                process::exit(1);
            }
        }
    });

    if matches.is_present("verbose") {
        Builder::from_env(Env::default().default_filter_or("info")).init();
    }

    if let Err(err) = split_textfile(matches.value_of("TEXTFILE").unwrap(), subtasks)
        .and_then(|chunks| run_on_golem(chunks, datadir, address, port))
        .and_then(|task| combine_wave(task, matches.value_of("WAVFILE").unwrap()))
    {
        eprintln!("An error occurred while {}", err);
        process::exit(1);
    }
}
