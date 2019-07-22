use super::task::{Task, TaskBuilder};
use super::timeout::Timeout;
use super::{Opt, Result};
use console::{style, Emoji};
use golem_rpc_api::comp::{self, AsGolemComp};
use hound;
use indicatif::ProgressBar;
use std::convert::TryFrom;
use std::env;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::time::SystemTime;

static TRUCK: Emoji = Emoji("🚚  ", "");
static CLIP: Emoji = Emoji("🔗  ", "");
static PAPER: Emoji = Emoji("📃  ", "");
static HOURGLASS: Emoji = Emoji("⌛  ", "");

#[derive(Debug)]
pub enum CompFragment<T> {
    Success(T),
    CtrlC,
}

#[derive(Debug)]
pub struct App {
    input: PathBuf,
    output: PathBuf,
    datadir: PathBuf,
    address: String,
    port: u16,
    subtasks: usize,
    bid: f64,
    task_timeout: Timeout,
    subtask_timeout: Timeout,
    abort: Arc<AtomicBool>,
}

impl App {
    fn split_input(&self) -> Result<CompFragment<Vec<String>>> {
        let mut contents = String::new();
        fs::File::open(&self.input)
            .and_then(|mut f| f.read_to_string(&mut contents))
            .map_err(|e| format!("reading from '{:?}': {}", self.input, e))?;

        let word_count = contents.split_whitespace().count();

        log::info!("Input text file has {} words", word_count);

        println!(
            "{} {}Splitting '{}' into {} Golem subtasks...",
            style("[1/4]").bold().dim(),
            PAPER,
            self.input.to_string_lossy(),
            self.subtasks,
        );

        let mut chunks = Vec::with_capacity(self.subtasks);
        let num_words = (word_count as f64 / self.subtasks as f64).ceil() as usize;

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

        Ok(CompFragment::Success(
            chunks.into_iter().map(|chunk| chunk.join(" ")).collect(),
        ))
    }

    fn send_to_golem(&self, chunks: CompFragment<Vec<String>>) -> Result<CompFragment<Task>> {
        let chunks = match chunks {
            CompFragment::Success(chunks) => chunks,
            CompFragment::CtrlC => return Ok(CompFragment::CtrlC),
        };

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
        let mut task_builder = TaskBuilder::new(
            workspace,
            self.bid,
            self.task_timeout.to_string(),
            self.subtask_timeout.to_string(),
        );

        for chunk in chunks {
            task_builder.add_subtask(chunk);
        }

        let task = task_builder.build()?;

        // connect to Golem
        let mut sys = actix::System::new("g-flite");
        let endpoint = sys
            .block_on(golem_rpc_api::connect_to_app(
                &self.datadir,
                Some(golem_rpc_api::Net::TestNet),
                Some((&self.address, self.port)),
            ))
            .map_err(|e| {
                format!(
                    "connecting to Golem with datadir='{:?}', net='{}', address='{}:{}': {}",
                    self.datadir,
                    golem_rpc_api::Net::TestNet,
                    self.address,
                    self.port,
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
            if self.abort.load(Ordering::Relaxed) {
                sys.block_on(endpoint.as_golem_comp().delete_task(task_id.clone()))
                    .map_err(|e| format!("cancelling task '{}': {}", task_id.clone(), e))?;
                return Ok(CompFragment::CtrlC);
            }

            let resp = sys
                .block_on(endpoint.as_golem_comp().get_task(task_id.clone()))
                .map_err(|e| format!("polling for task '{}': {}", task_id.clone(), e))?;
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
                    return Err(
                        "waiting for task to complete: task aborted by someone else".to_owned()
                    )
                }
                comp::TaskStatus::Timeout => {
                    return Err("waiting for task to complete: task timed out".to_owned())
                }
                comp::TaskStatus::Finished => break,
                _ => {}
            }

            thread::sleep(Duration::from_secs(1));
        }

        Ok(CompFragment::Success(task))
    }

    fn combine_output(&self, task: CompFragment<Task>) -> Result<CompFragment<()>> {
        let mut task = match task {
            CompFragment::Success(task) => task,
            CompFragment::CtrlC => return Ok(CompFragment::CtrlC),
        };

        let first = task
            .expected_output_paths
            .pop_front()
            .ok_or("combining results: no results received from Golem".to_owned())?;

        println!(
            "{} {}Combining output into '{}'...",
            style("[4/4]").bold().dim(),
            CLIP,
            self.output.to_string_lossy()
        );

        let reader = hound::WavReader::open(&first)
            .map_err(|e| format!("opening WAVE file '{}': {}", first.to_string_lossy(), e))?;
        let spec = reader.spec();

        log::info!("Using Wav spec: {:?}", spec);

        let mut writer = hound::WavWriter::create(&self.output, spec).map_err(|e| {
            format!(
                "creating WAVE file '{}': {}",
                self.output.to_string_lossy(),
                e
            )
        })?;
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

        Ok(CompFragment::Success(()))
    }

    pub fn run(&self) -> Result<CompFragment<()>> {
        // register for ctrl-c events
        let abort = Arc::clone(&self.abort);
        let _ = ctrlc::set_handler(move || {
            abort.store(true, Ordering::Relaxed);
        });

        // run!
        self.split_input()
            .and_then(|chunks| self.send_to_golem(chunks))
            .and_then(|task| self.combine_output(task))
    }
}

impl TryFrom<Opt> for App {
    type Error = String;

    fn try_from(opt: Opt) -> std::result::Result<Self, Self::Error> {
        // verify input exists
        let input = opt.input;
        if !input.is_file() {
            return Err(format!(
                "Input file '{:?}' doesn't exist. Did you make a typo anywhere?",
                input
            ));
        }

        // verify output path excluding topmost file exists
        if let Some(parent) = opt.output.parent() {
            let parent_str = parent.to_string_lossy();
            if !parent_str.is_empty() && !parent.exists() {
                return Err(format!(
                    "Output path '{}' doesn't exist. Did you make a type anywhere?",
                    parent_str,
                ));
            }
        }
        let output = opt.output;

        let datadir = match opt.datadir {
            Some(datadir) => datadir,
            None => match appdirs::user_data_dir(Some("golem"), Some("golem"), false) {
                Ok(datadir) => datadir.join("default"),
                Err(_) => {
                    return Err("
                    No standard project app datadirs available.
                    You'll need to specify path to your Golem datadir manually.
                    "
                    .to_owned())
                }
            },
        };

        let address = opt.address.clone();
        let port = opt.port;
        let subtasks = opt.subtasks;
        let bid = opt.bid;
        let task_timeout = opt.task_timeout;
        let subtask_timeout = opt.subtask_timeout;
        let abort = Arc::new(AtomicBool::new(false));

        Ok(Self {
            input,
            output,
            datadir,
            address,
            port,
            subtasks,
            bid,
            task_timeout,
            subtask_timeout,
            abort,
        })
    }
}
