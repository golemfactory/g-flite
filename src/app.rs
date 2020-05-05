use super::Opt;
use anyhow::{anyhow, bail, Context, Result};
use console::{style, Emoji};
use gwasm_api::prelude::*;
use hound;
use indicatif::ProgressBar;
use std::cell::Cell;
use std::convert::TryFrom;
use std::path::{Path, PathBuf};
use std::{fmt, fs};
use tempfile::{Builder, TempDir};

static TRUCK: Emoji = Emoji("🚚  ", "");
static CLIP: Emoji = Emoji("🔗  ", "");
static PAPER: Emoji = Emoji("📃  ", "");
static HOURGLASS: Emoji = Emoji("⌛  ", "");

const FLITE_JS: &[u8] = include_bytes!("../assets/flite.js");
const FLITE_WASM: &[u8] = include_bytes!("../assets/flite.wasm");

#[derive(Debug)]
enum Workspace {
    UserSpecified(PathBuf),
    Temp(TempDir),
}

impl fmt::Display for Workspace {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Workspace::UserSpecified(path) => write!(f, "{}", path.display()),
            Workspace::Temp(dir) => write!(f, "{}", dir.path().display()),
        }
    }
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
    progress: Cell<f64>,
    num_subtasks: u64,
}

impl ProgressUpdater {
    fn new(num_subtasks: u64) -> Self {
        Self {
            bar: ProgressBar::new(num_subtasks),
            progress: Cell::new(0.0),
            num_subtasks,
        }
    }
}

impl ProgressUpdate for ProgressUpdater {
    fn update(&self, progress: f64) {
        let old_progress = self.progress.get();
        if progress > old_progress {
            let delta = progress - old_progress;
            self.progress.set(progress);
            self.bar
                .inc((delta * self.num_subtasks as f64).round() as u64);
        }
    }

    fn start(&self) {
        self.bar.inc(0)
    }

    fn stop(&self) {
        self.bar.finish_and_clear()
    }
}

#[derive(Debug)]
pub struct App {
    input: PathBuf,
    output_dir: PathBuf,
    output_filename: PathBuf,
    datadir: PathBuf,
    address: String,
    port: u16,
    num_subtasks: u64,
    bid: f64,
    budget: Option<f64>,
    task_timeout: Timeout,
    subtask_timeout: Timeout,
    workspace: Workspace,
    net: Net,
}

impl App {
    fn split_input(&self) -> Result<Vec<String>> {
        let contents = fs::read(&self.input)
            .with_context(|| format!("reading from '{}'", self.input.display()))?;
        let contents = String::from_utf8(contents).context("converting read bytes to string")?;
        let word_count = contents.split_whitespace().count();

        if (word_count as u64) < self.num_subtasks {
            bail!(
                "splitting input into Golem subtasks: cannot split input of {} words into {} subtasks",
                word_count, self.num_subtasks
            );
        }

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
        log::info!("Will prepare task in '{}'", self.workspace);

        // prepare Golem task
        let binary = GWasmBinary {
            js: FLITE_JS,
            wasm: FLITE_WASM,
        };
        // get expected output dir (if any)
        let mut task_builder = TaskBuilder::new(&self.workspace, binary)
            .name("g_flite")
            .bid(self.bid)
            .timeout(self.task_timeout)
            .subtask_timeout(self.subtask_timeout)
            .output_path(&self.output_dir);

        if let Some(budget) = self.budget {
            task_builder = task_builder.budget(budget);
        }

        for chunk in chunks {
            task_builder = task_builder.push_subtask_data(chunk.as_bytes());
        }

        task_builder.build().context("building gWasm task")
    }

    fn combine_output(&self, task: ComputedTask) -> Result<()> {
        let output = self.output_dir.join(&self.output_filename);
        println!(
            "{} {}Combining output into '{}'...",
            style("[4/4]").bold().dim(),
            CLIP,
            output.display()
        );

        let mut writer: Option<hound::WavWriter<_>> = None;

        log::info!("Computed task = {:?}", task);

        for (i, subtask) in task.subtasks.into_iter().enumerate() {
            for (_, reader) in subtask.data.into_iter() {
                let reader = hound::WavReader::new(reader).context("parsing WAVE input")?;

                if writer.is_none() {
                    writer = Some(
                        hound::WavWriter::create(&output, reader.spec()).with_context(|| {
                            format!("creating output WAVE file '{}'", output.display())
                        })?,
                    );
                }

                let mut wrt = writer.as_mut().unwrap().get_i16_writer(reader.len());
                for sample in reader.into_samples::<i16>() {
                    sample
                        .map(|sample| unsafe { wrt.write_sample_unchecked(sample) })
                        .with_context(|| format!("reading audio sample from subtask '{}'", i))?;
                }
                wrt.flush().with_context(|| {
                    format!("writing audio samples to file '{}'", output.display(),)
                })?;
            }
        }

        Ok(())
    }

    pub fn run(&self) -> Result<()> {
        let chunks = self.split_input()?;
        let task = self.prepare_task(chunks)?;

        log::debug!("g_flite run task = {:?}", task);

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
            self.datadir.clone(),
            self.address.clone(),
            self.port,
            self.net.clone(),
            task,
            progress_updater,
        )
        .context("computing task on Golem: {}")?;

        self.combine_output(computed_task)
    }
}

impl TryFrom<Opt> for App {
    type Error = anyhow::Error;

    fn try_from(opt: Opt) -> std::result::Result<Self, Self::Error> {
        // verify input exists
        let input = opt.input;
        if !input.is_file() {
            bail!(
                "Input file '{}' doesn't exist. Did you make a typo anywhere?",
                input.display()
            );
        }

        // verify output path excluding topmost file exists
        let output = if opt.output.is_relative() {
            Path::new(".").join(opt.output)
        } else {
            opt.output.into()
        };
        let (output_dir, output_filename) = {
            let parent = output.parent().unwrap(); // guaranteed not to fail
            let filename = output.file_name().ok_or(anyhow!(
                "working out the expected output filename from '{}'",
                output.display()
            ))?;
            (parent.to_path_buf(), PathBuf::from(filename))
        };
        let output_dir = output_dir.canonicalize().with_context(|| {
            format!(
                "working out absolute path for the expected output path '{}'",
                output.display(),
            )
        })?;

        let datadir = match opt.datadir {
            Some(datadir) => datadir.canonicalize().with_context(|| {
                format!(
                    "working out absolute path for the provided datadir '{}'",
                    datadir.display(),
                )
            })?,
            None => match appdirs::user_data_dir(Some("golem"), Some("golem"), false) {
                Ok(datadir) => datadir.join("default"),
                Err(_) => bail!(
                    "
                    No standard project app datadirs available.
                    You'll need to specify path to your Golem datadir manually.
                    "
                ),
            },
        };

        let address = opt.address;
        let port = opt.port;
        let num_subtasks = opt.subtasks;
        let bid = opt.bid;
        let budget = opt.budget;
        let task_timeout = opt.task_timeout;
        let subtask_timeout = opt.subtask_timeout;
        let net = if opt.mainnet {
            Net::MainNet
        } else {
            Net::TestNet
        };

        let workspace = match opt.workspace {
            Some(workspace) => {
                Workspace::UserSpecified(workspace.canonicalize().with_context(|| {
                    format!(
                        "working out absolute path for provided workspace dir '{}'",
                        workspace.display(),
                    )
                })?)
            }
            None => Workspace::Temp(
                Builder::new()
                    .prefix("g_flite")
                    .tempdir()
                    .context("creating workspace dir in your tmp files")?,
            ),
        };

        Ok(Self {
            input,
            output_dir,
            output_filename,
            datadir,
            address,
            port,
            num_subtasks,
            bid,
            budget,
            task_timeout,
            subtask_timeout,
            workspace,
            net,
        })
    }
}
