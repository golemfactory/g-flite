use std::collections::VecDeque;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path;
use std::process::Command;
use std::time::SystemTime;

use console::{style, Emoji};
use docopt::Docopt;
use hound;
use indicatif::ProgressBar;
use serde::Deserialize;
use serde_json::{json, Map};

const USAGE: &str = "
g_flite: flite distributed over Golem network

Usage:
    g_flite <textfile> <wavefile>
    g_flite (-h | --help)

Options:
    -h --help   Show this screen.
";

#[derive(Debug, Deserialize)]
struct Args {
    arg_textfile: String,
    arg_wavefile: String,
}

const FLITE_JS: &[u8] = include_bytes!("../assets/flite.js");
const FLITE_WASM: &[u8] = include_bytes!("../assets/flite.wasm");
const SPLIT_BY_WORDS: usize = 100;

static TRUCK: Emoji = Emoji("🚚  ", "");
static CLIP: Emoji = Emoji("🔗  ", "");
static PAPER: Emoji = Emoji("📃  ", "");
static HOURGLASS: Emoji = Emoji("⌛ ", "");

fn split_textfile(textfile: &str) -> Vec<String> {
    println!(
        "{} {}Splitting '{}' into chunks...",
        style("[1/5]").bold().dim(),
        PAPER,
        textfile,
    );

    let mut reader = fs::File::open(textfile).unwrap();
    let mut contents = String::new();
    reader.read_to_string(&mut contents).unwrap();

    let mut chunks: Vec<String> = Vec::new();

    let mut acc: String = String::new();
    for (i, word) in contents.split_whitespace().enumerate() {
        acc.push_str(word);
        acc.push(' ');

        if (i + 1) % SPLIT_BY_WORDS == 0 {
            chunks.push(acc);
            acc = String::new();
            continue;
        }
    }

    if !acc.is_empty() {
        chunks.push(acc);
    }

    chunks
}

fn run_on_golem(chunks: Vec<String>) -> VecDeque<String> {
    // prepare workspace
    println!(
        "{} {}Preparing Golem task...",
        style("[2/5]").bold().dim(),
        HOURGLASS
    );

    let mut workspace = env::temp_dir();
    let time_now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let subdir = format!("g_flite_{}", time_now.as_secs());
    workspace.push(subdir);
    fs::create_dir(workspace.as_path()).unwrap();

    let mut input_dir = path::PathBuf::from(workspace.as_path());
    input_dir.push("in");
    fs::create_dir(input_dir.as_path()).unwrap();

    let mut output_dir = path::PathBuf::from(workspace.as_path());
    output_dir.push("out");
    fs::create_dir(output_dir.as_path()).unwrap();

    let mut js = path::PathBuf::from(input_dir.as_path());
    js.push("flite.js");
    let mut f = fs::File::create(js).unwrap();
    f.write_all(FLITE_JS).unwrap();

    let mut wasm = path::PathBuf::from(input_dir.as_path());
    wasm.push("flite.wasm");
    let mut f = fs::File::create(wasm).unwrap();
    f.write_all(FLITE_WASM).unwrap();

    let mut subtasks_map = Map::new();
    let mut wavefiles = VecDeque::new();

    for (i, chunk) in chunks.into_iter().enumerate() {
        let mut subtask_input = path::PathBuf::from(input_dir.as_path());
        let subtask_name = format!("subtask{}", i);

        subtask_input.push(&subtask_name);
        fs::create_dir(subtask_input.as_path()).unwrap();

        subtask_input.push("in.txt");
        let mut f = fs::File::create(subtask_input).unwrap();
        f.write_all(chunk.as_bytes()).unwrap();

        let mut subtask_output = path::PathBuf::from(output_dir.as_path());
        subtask_output.push(&subtask_name);
        fs::create_dir(subtask_output.as_path()).unwrap();

        subtasks_map.insert(
            subtask_name.clone(),
            json!({
                "exec_args": ["in.txt", "in.wav"],
                "output_file_paths": ["in.wav"],
            }),
        );

        subtask_output.push("in.wav");
        wavefiles.push_back(subtask_output.to_str().unwrap().to_string());
    }

    let task_json = json!({
        "type": "wasm",
        "name": "g_flite",
        "bid": 1,
        "subtask_timeout": "00:10:00",
        "timeout": "00:10:00",
        "options": {
            "js_name": "flite.js",
            "wasm_name": "flite.wasm",
            "input_dir": input_dir.to_str().unwrap(),
            "output_dir": output_dir.to_str().unwrap(),
            "subtasks": subtasks_map,
        }
    });

    let mut input_json = path::PathBuf::from(workspace.as_path());
    input_json.push("task.json");
    let f = fs::File::create(input_json.as_path()).unwrap();
    serde_json::to_writer_pretty(f, &task_json).unwrap();

    // send to Golem
    println!(
        "{} {}Sending task to Golem...",
        style("[3/5]").bold().dim(),
        TRUCK
    );

    let output = Command::new("/bin/sh")
        .arg("-c")
        .arg(format!("$HOME/.virtualenvs/golem/bin/python $HOME/dev/golem/golemcli.py --datadir=$HOME/datadir1 --port=61001 tasks create {}", input_json.to_str().unwrap()))
        .output()
        .unwrap();
    let task_id = String::from_utf8(output.stdout).unwrap();

    // wait
    println!(
        "{} {}Waiting on compute to finish...",
        style("[4/5]").bold().dim(),
        HOURGLASS
    );
    let num_tasks = wavefiles.len() as u64;
    let bar = ProgressBar::new(num_tasks);
    bar.inc(0);
    let mut old_progress = 0.0;

    loop {
        let output = Command::new("/bin/sh")
                        .arg("-c")
                        .arg(format!("$HOME/.virtualenvs/golem/bin/python $HOME/dev/golem/golemcli.py --datadir=$HOME/datadir1 --port=61001 tasks show {}", task_id))
                        .output()
                        .unwrap();
        let output = String::from_utf8(output.stdout).unwrap();
        let status_idx = output.find("status: ").unwrap();
        let status = output[(status_idx + 8)..].split('\n').next().unwrap();
        let progress_idx = output.find("progress: ").unwrap();
        let progress = output[(progress_idx + 10)..].split('\n').next().unwrap();
        let progress = progress[0..(progress.len() - 2)].parse::<f64>().unwrap();

        if progress != old_progress {
            let delta = (progress - old_progress) / 100.0;
            old_progress = progress;
            bar.inc((delta * num_tasks as f64).round() as u64);
        }

        if status == "Finished" {
            break;
        }
    }

    wavefiles
}

fn combine_wave(mut wavefiles: VecDeque<String>, output_wavefile: &str) {
    if wavefiles.is_empty() {
        return;
    }

    println!(
        "{} {}Combining output into '{}'...",
        style("[5/5]").bold().dim(),
        CLIP,
        output_wavefile
    );

    let first = wavefiles.pop_front().unwrap();
    let reader = hound::WavReader::open(first).unwrap();
    let spec = reader.spec();
    let mut writer = hound::WavWriter::create(output_wavefile, spec).unwrap();
    for sample in reader.into_samples::<i16>() {
        writer.write_sample(sample.unwrap()).unwrap();
    }

    for wavefile in wavefiles {
        let reader = hound::WavReader::open(wavefile).unwrap();
        for sample in reader.into_samples::<i16>() {
            writer.write_sample(sample.unwrap()).unwrap();
        }
    }
}

fn main() {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    let chunks = split_textfile(&args.arg_textfile);
    let wavefiles = run_on_golem(chunks);
    combine_wave(wavefiles, &args.arg_wavefile);
}
