use super::Opt;
use console::{style, Emoji};
use gwasm_api::prelude::*;
use hound;
use indicatif::ProgressBar;
use std::convert::TryFrom;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::{Builder, TempDir};

static TRUCK: Emoji = Emoji("🚚  ", "");
static CLIP: Emoji = Emoji("🔗  ", "");
static PAPER: Emoji = Emoji("📃  ", "");
static HOURGLASS: Emoji = Emoji("⌛  ", "");

const FLITE_JS: &[u8] = include_bytes!("../assets/flite.js");
const FLITE_WASM: &[u8] = include_bytes!("../assets/flite.wasm");

type Result<T> = std::result::Result<T, String>;

#[derive(Debug)]
enum Workspace {
    UserSpecified(PathBuf),
    Temp(TempDir),
}

impl AsRef<Path> for Workspace {
    fn as_ref(&self) -> &Path {
        match self {
            Workspace::UserSpecified(x) => x.as_ref(),
            Workspace::Temp(x) => x.as_ref(),
        }
    }
}

struct ProgressUpdater {
    bar: ProgressBar,
    progress: f64,
    num_subtasks: u64,
}

impl ProgressUpdater {
    fn new(num_subtasks: u64) -> Self {
        Self {
            bar: ProgressBar::new(num_subtasks),
            progress: 0.0,
            num_subtasks,
        }
    }
}

impl ProgressUpdate for ProgressUpdater {
    fn update(&mut self, progress: f64) {
        if progress > self.progress {
            let delta = progress - self.progress;
            self.progress = progress;
            self.bar
                .inc((delta * self.num_subtasks as f64).round() as u64);
        }
    }

    fn start(&mut self) {
        self.bar.inc(0)
    }

    fn stop(&mut self) {
        self.bar.finish_and_clear()
    }
}

#[derive(Debug)]
pub struct App {
    input: PathBuf,
    output: PathBuf,
    datadir: PathBuf,
    address: String,
    port: u16,
    num_subtasks: u64,
    bid: f64,
    task_timeout: Timeout,
    subtask_timeout: Timeout,
    workspace: Workspace,
    net: Net,
}

impl App {
    fn split_input(&self) -> Result<Vec<String>> {
        let contents =
            fs::read(&self.input).map_err(|e| format!("reading from '{:?}': {}", self.input, e))?;
        let contents =
            String::from_utf8(contents).map_err(|_| format!("converting read bytes to string"))?;
        let word_count = contents.split_whitespace().count();

        log::info!("Input text file has {} words", word_count);

        println!(
            "{} {}Splitting '{}' into {} Golem subtasks...",
            style("[1/4]").bold().dim(),
            PAPER,
            self.input.to_string_lossy(),
            self.num_subtasks,
        );

        let mut chunks = Vec::with_capacity(self.num_subtasks as usize);
        let num_words = (word_count as f64 / self.num_subtasks as f64).ceil() as usize;

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

    fn prepare_task(&self, chunks: impl IntoIterator<Item = String>) -> Result<Task> {
        log::info!("Will prepare task in '{:?}'", self.workspace);

        // prepare Golem task
        let binary = GWasmBinary {
            js: FLITE_JS,
            wasm: FLITE_WASM,
        };
        let mut task_builder = TaskBuilder::new(&self.workspace, binary)
            .name("g_flite")
            .bid(self.bid)
            .timeout(self.task_timeout)
            .subtask_timeout(self.subtask_timeout);

        for chunk in chunks {
            task_builder = task_builder.push_subtask_data(chunk.as_bytes());
        }

        task_builder
            .build()
            .map_err(|e| format!("building gWasm task: {}", e))
    }

    fn combine_output(&self, task: ComputedTask) -> Result<()> {
        println!(
            "{} {}Combining output into '{}'...",
            style("[4/4]").bold().dim(),
            CLIP,
            self.output.to_string_lossy()
        );

        let mut writer: Option<hound::WavWriter<_>> = None;
        for (i, subtask) in task.subtasks.into_iter().enumerate() {
            for (_, reader) in subtask.data.into_iter() {
                let reader = hound::WavReader::new(reader)
                    .map_err(|e| format!("parsing WAVE input: {}", e))?;

                let wrt = writer.get_or_insert_with(|| {
                    hound::WavWriter::create(&self.output, reader.spec()).unwrap()
                });
                let mut wrt = wrt.get_i16_writer(reader.len());

                for sample in reader.into_samples::<i16>() {
                    sample
                        .map(|sample| unsafe { wrt.write_sample_unchecked(sample) })
                        .map_err(|e| format!("reading audio sample from subtask '{}': {}", i, e))?;
                }
            }
        }

        Ok(())
    }

    pub fn run(&self) -> Result<()> {
        let chunks = self.split_input()?;
        let task = self.prepare_task(chunks)?;

        println!(
            "{} {}Sending task to Golem...",
            style("[2/4]").bold().dim(),
            TRUCK
        );

        println!(
            "{} {}Waiting on compute to finish...",
            style("[3/4]").bold().dim(),
            HOURGLASS
        );

        let progress_updater = ProgressUpdater::new(self.num_subtasks);
        let computed_task = compute(
            &self.datadir,
            &self.address,
            self.port,
            self.net.clone(),
            task,
            progress_updater,
        )
        .map_err(|e| format!("computing task on Golem: {}", e))?;

        self.combine_output(computed_task)
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
            Some(datadir) => datadir.canonicalize().map_err(|e| {
                format!(
                    "working out absolute path for the provided datadir '{:?}': {}",
                    datadir, e
                )
            })?,
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

        let address = opt.address;
        let port = opt.port;
        let num_subtasks = opt.subtasks;
        let bid = opt.bid;
        let task_timeout = opt.task_timeout;
        let subtask_timeout = opt.subtask_timeout;
        let net = if opt.mainnet {
            Net::MainNet
        } else {
            Net::TestNet
        };

        let workspace = match opt.workspace {
            Some(workspace) => Workspace::UserSpecified(workspace.canonicalize().map_err(|e| {
                format!(
                    "working out absolute path for provided workspace dir '{:?}': {}",
                    workspace, e
                )
            })?),
            None => Workspace::Temp(
                Builder::new()
                    .prefix("g_flite")
                    .tempdir()
                    .map_err(|e| format!("creating workspace dir in your tmp files: {}", e))?,
            ),
        };

        Ok(Self {
            input,
            output,
            datadir,
            address,
            port,
            num_subtasks,
            bid,
            task_timeout,
            subtask_timeout,
            workspace,
            net,
        })
    }
}
