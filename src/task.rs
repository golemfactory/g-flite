use super::{GolemOpt, Result};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const FLITE_JS: &[u8] = include_bytes!("../assets/flite.js");
const FLITE_WASM: &[u8] = include_bytes!("../assets/flite.wasm");

pub struct TaskBuilder {
    bid: f64,
    task_timeout: String,
    subtask_timeout: String,
    js_name: String,
    wasm_name: String,
    input_dir_path: PathBuf,
    output_dir_path: PathBuf,
    subtask_count: usize,
    subtasks: BTreeMap<String, String>,
}

impl TaskBuilder {
    pub fn new<S: AsRef<Path>>(workspace: S, opt: GolemOpt) -> Self {
        Self {
            bid: opt.bid,
            task_timeout: opt.task_timeout.to_string(),
            subtask_timeout: opt.subtask_timeout.to_string(),
            js_name: "flite.js".into(),
            wasm_name: "flite.wasm".into(),
            input_dir_path: workspace.as_ref().join("in"),
            output_dir_path: workspace.as_ref().join("out"),
            subtask_count: 0,
            subtasks: BTreeMap::new(),
        }
    }

    pub fn add_subtask(&mut self, data: String) {
        self.subtasks
            .insert(format!("subtask{}", self.subtask_count), data);
        self.subtask_count += 1;
    }

    pub fn build(self) -> Result<Task> {
        // create input dir
        fs::create_dir(self.input_dir_path.as_path()).map_err(|e| {
            format!(
                "creating directory '{}': {}",
                self.input_dir_path.to_string_lossy(),
                e
            )
        })?;
        // save JS file
        let js_filename = self.input_dir_path.join(&self.js_name);
        fs::File::create(&js_filename)
            .and_then(|mut f| f.write_all(FLITE_JS))
            .map_err(|e| format!("writing to file '{}': {}", js_filename.to_string_lossy(), e))?;

        // save WASM file
        let wasm_filename = self.input_dir_path.join(&self.wasm_name);
        fs::File::create(&wasm_filename)
            .and_then(|mut f| f.write_all(FLITE_WASM))
            .map_err(|e| {
                format!(
                    "writing to file '{}': {}",
                    wasm_filename.to_string_lossy(),
                    e
                )
            })?;

        // create output dir
        fs::create_dir(self.output_dir_path.as_path()).map_err(|e| {
            format!(
                "creating directory '{}': {}",
                self.output_dir_path.to_string_lossy(),
                e
            )
        })?;

        let mut subtasks_map = Map::new();
        let mut expected_output_paths = VecDeque::new();

        for (subtask_name, subtask_data) in self.subtasks {
            // create input subtask dir
            let input_dir_path = self.input_dir_path.join(&subtask_name);
            fs::create_dir(&input_dir_path).map_err(|e| {
                format!(
                    "creating directory '{}': {}",
                    input_dir_path.to_string_lossy(),
                    e
                )
            })?;
            // create output subtask dir
            let output_dir_path = self.output_dir_path.join(&subtask_name);
            fs::create_dir(&output_dir_path).map_err(|e| {
                format!(
                    "creating directory '{}': {}",
                    output_dir_path.to_string_lossy(),
                    e
                )
            })?;
            // save input data file
            let input_name = "in.txt";
            let input_filename = input_dir_path.join(&input_name);
            fs::File::create(&input_filename)
                .and_then(|mut f| f.write_all(subtask_data.as_bytes()))
                .map_err(|e| {
                    format!(
                        "writing to file '{}': {}",
                        input_filename.to_string_lossy(),
                        e
                    )
                })?;

            let output_name = "in.wav";
            expected_output_paths.push_back(output_dir_path.join(&output_name));

            subtasks_map.insert(
                subtask_name,
                json!({
                    "exec_args": [input_name, output_name],
                    "output_file_paths": [output_name],
                }),
            );
        }

        let json = json!({
            "type": "wasm",
            "name": "g_flite",
            "bid": self.bid,
            "subtask_timeout": self.subtask_timeout,
            "timeout": self.task_timeout,
            "options": {
                "js_name": self.js_name,
                "wasm_name": self.wasm_name,
                "input_dir": self.input_dir_path.to_string_lossy(),
                "output_dir": self.output_dir_path.to_string_lossy(),
                "subtasks": subtasks_map,
            }
        });

        log::info!("Created json manifest for task: {}", json);

        Ok(Task {
            json,
            expected_output_paths,
        })
    }
}

pub struct Task {
    pub json: Value,
    pub expected_output_paths: VecDeque<PathBuf>,
}
