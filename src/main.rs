use std::fs;
use std::io::Read;

use docopt::Docopt;
use hound;
use serde::Deserialize;

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
const SPLIT_BY_WORDS: usize = 60;

fn split_textfile(textfile: &str) -> Vec<String> {
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

fn run_on_golem(chunks: Vec<String>) -> Vec<String> {
    Vec::new()
}

fn combine_wave(wavefiles: Vec<String>, output_wavefile: &str) {
    if wavefiles.is_empty() {
        return;
    }

    let reader = hound::WavReader::open(&wavefiles[0]).unwrap();
    let spec = reader.spec();
    let mut writer = hound::WavWriter::create(output_wavefile, spec).unwrap();
    for sample in reader.into_samples::<i16>() {
        writer.write_sample(sample.unwrap()).unwrap();
    }

    for wavefile in &wavefiles[1..] {
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
