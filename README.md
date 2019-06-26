# g-flite [![travis-build-status]][travis]

[travis-build-status]: https://travis-ci.org/golemfactory/g-flite.svg?branch=master
[travis]: https://travis-ci.org/golemfactory/g-flite

`g-flite` is a command-line utility which lets you run [flite](http://www.festvox.org/flite/)
text-to-speech app on Golem Network.

![g_flite GIF demo](http://i.imgur.com/Ji1CdCN.gif)

__Note that `g-flite` currently requires that you have Golem instance running on the same machine
and only testnet is currently supported due to the fact that
[our WASM platform](https://github.com/golemfactory/sp-wasm) is only available on the testnet.__

## Installation
You can grab a precompiled version of the program for each OS, Linux, Mac, and Win, from
[here](https://github.com/golemfactory/g-flite/releases).

### Building from source
If you wish however, you can also build the program from source. To do this, you'll first need
to clone the repo.

```
$ git clone --depth 50 https://github.com/golemfactory/g-flite
$ cd g-flite
```

Afterwards, you need to ensure you have Rust installed in version at least `1.34.0`. A good place
to get your hands on the latest Rust is [rustup website](https://rustup.rs/).

With Rust installed on your OS, you then need to simply run from within `g-flite` dir

```
$ cargo build
```

for debug version, or

```
$ cargo build --release
```

for release version. Your program can then be found in

```
g-flite/target/debug/g_flite
```

for debug version, or

```
g-flite/target/release/g_flite
```

for release version.

## Usage
Typical usage should not differ much or at all from how you would use the original `flite` app

```
$ g_flite some_text_input.txt some_speech_output.wav
```

Note that it is required to specify the name of the output file. All of this assumes that you
have your Golem installed using the default settings

| Setting     | Default value                 |
| ----------- | ----------------------------- |
| datadir     | `$APP_DATA_DIR/golem/default` |
| RPC address | 127.0.0.1                     |
| RPC port    | 61000                         |

`$APP_DATA_DIR` is platform specific:
* on Linux will usually refer to `$HOME/.local/share/<project_path>`
* on Mac will usually refer to `$HOME/Library/Application Support/<project_path>`
* on Windows will usually refer to `{FOLDERID_LocalAppData}/<project_path>/data`

If any of the above information is not correct for your Golem configuration, you can
adjust them directly in the command-line as follows

```
$ g_flite --address 127.0.0.1 --port 61000 --datadir /abs/path/to/golem/datadir some_text_input.txt some_speech_output.wav
```

Finally, by default `g-flite` will split your input text into 6 subtasks and compute them
on Golem Network. You can also adjust this option in the command-line as follows

```
$ g_flite --subtasks 2 some_text_input.txt some_speech_output.wav
```

All of this information can also be extracted from the command-line with the `-h` or `--help` flags

```
$ g_flite -h

g_flite 0.1.0
Golem RnD Team <contact@golem.network>
flite, a text-to-speech program, distributed over Golem network

USAGE:
    g_flite [FLAGS] [OPTIONS] <TEXTFILE> <WAVFILE>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information
    -v, --verbose    Turns verbose logging on

OPTIONS:
        --address <ADDRESS>    Sets RPC address to Golem instance
        --datadir <DATADIR>    Sets path to Golem datadir
        --port <PORT>          Sets RPC port to Golem instance
        --subtasks <NUM>       Sets number of Golem subtasks

ARGS:
    <TEXTFILE>    Input text file
    <WAVFILE>     Output WAV file
```

## Issues
This program is still very much a work-in-progress, so if you find (and you most likely will) any bugs,
please submit them [in our issue tracker](https://github.com/golemfactory/g-flite/issues/new).

## License
Licensed under [GNU General Public License v3.0](LICENSE) with the exception of `flite` WASM binary
which is licensed under [BSD-like License](LICENSE.flite).

