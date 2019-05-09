use serde_json::{json, Map, Value};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const FLITE_JS: &[u8] = include_bytes!("../assets/flite.js");
const FLITE_WASM: &[u8] = include_bytes!("../assets/flite.wasm");

pub struct TaskBuilder {
    js_name: String,
    wasm_name: String,
    input_dir_path: PathBuf,
    output_dir_path: PathBuf,
    subtask_count: usize,
    subtasks: HashMap<String, String>,
}

impl TaskBuilder {
    pub fn new<S: AsRef<Path>>(workspace: S) -> Self {
        Self {
            js_name: "flite.js".into(),
            wasm_name: "flite.wasm".into(),
            input_dir_path: workspace.as_ref().join("in"),
            output_dir_path: workspace.as_ref().join("out"),
            subtask_count: 0,
            subtasks: HashMap::new(),
        }
    }

    pub fn add_subtask(&mut self, data: String) {
        self.subtasks
            .insert(format!("subtask{}", self.subtask_count), data);
        self.subtask_count += 1;
    }

    pub fn build(self) -> Task {
        // create input dir
        fs::create_dir(self.input_dir_path.as_path()).unwrap();
        // save JS file
        let mut f = fs::File::create(self.input_dir_path.join(&self.js_name)).unwrap();
        f.write_all(FLITE_JS).unwrap();
        // save WASM file
        let mut f = fs::File::create(self.input_dir_path.join(&self.wasm_name)).unwrap();
        f.write_all(FLITE_WASM).unwrap();
        // create output dir
        fs::create_dir(self.output_dir_path.as_path()).unwrap();

        let mut subtasks_map = Map::new();
        let mut expected_output_paths = VecDeque::new();

        for (subtask_name, subtask_data) in self.subtasks {
            // create input subtask dir
            fs::create_dir(self.input_dir_path.join(&subtask_name)).unwrap();
            // create output subtask dir
            fs::create_dir(self.output_dir_path.join(&subtask_name)).unwrap();
            // save input data file
            let input_name = "in.txt";
            let mut f = fs::File::create(self.input_dir_path.join(&subtask_name).join(&input_name))
                .unwrap();
            f.write_all(subtask_data.as_bytes()).unwrap();

            let output_name = "in.wav";
            expected_output_paths
                .push_back(self.output_dir_path.join(&subtask_name).join(&output_name));

            subtasks_map.insert(
                subtask_name,
                json!({
                    "exec_args": [input_name, output_name],
                    "output_file_paths": [output_name],
                }),
            );
        }

        Task {
            json: json!({
                "type": "wasm",
                "name": "g_flite",
                "bid": 1,
                "subtask_timeout": "00:10:00",
                "timeout": "00:10:00",
                "options": {
                    "js_name": self.js_name,
                    "wasm_name": self.wasm_name,
                    "input_dir": self.input_dir_path.to_str().unwrap(),
                    "output_dir": self.output_dir_path.to_str().unwrap(),
                    "subtasks": subtasks_map,
                }
            }),
            expected_output_paths,
        }
    }
}

pub struct Task {
    pub json: Value,
    pub expected_output_paths: VecDeque<PathBuf>,
}
