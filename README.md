# g-flite [![build-status]][build]

[build-status]: https://github.com/golemfactory/g-flite/workflows/Continuous%20integration/badge.svg
[build]: https://github.com/golemfactory/g-flite/actions

`g-flite` is a command-line utility which lets you run [flite](http://www.festvox.org/flite/)
text-to-speech app on Golem Network.

![g_flite GIF demo](http://i.imgur.com/Ji1CdCN.gif)

## Installation
You can grab a precompiled version of the program for each OS, Linux, Mac, and Win, from
[here](https://github.com/golemfactory/g-flite/releases).

### Building from source
If you wish however, you can also build the program from source. To do this, you'll first need
to clone the repo.

```
git clone --depth 50 https://github.com/golemfactory/g-flite
cd g-flite
```

Afterwards, you need to ensure you have Rust installed in version at least `1.34.0`. A good place
to get your hands on the latest Rust is [rustup website](https://rustup.rs/).

With Rust installed on your OS, you then need to simply run from within `g-flite` dir

```
cargo build
```

for debug version, or

```
cargo build --release
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
Typical usage should not differ much or at all from how you would use the original `flite` app.
So, in order to generate speech from some input text `some_text_input.txt` and save to a WAV file
`some_speech_output.wav` on Golem testnet, you would simply run it as


```
g_flite some_text_input.txt some_speech_output.wav
```

**To run the tasks on Golem mainnet, simply add a `--mainnet` flag like so**

```
g_flite some_text_input.txt some_speech_output.wav --mainnet
```

Note that it is required to specify the name of the output file. To provide a more concrete example,
let's take the "Moby Dick; Or, The Whale" by Herman Melville. Download the entire book in
a text format [here](https://www.gutenberg.org/files/2701/2701-0.txt), and save it as `moby_dick.txt`.
Then, you can convert the book to speech using `g-flite` by simply running

```
g_flite moby_dick.txt moby_dick.wav
```

All of this assumes that you have your Golem installed using the default settings

| Setting     | Default value                 |
| ----------- | ----------------------------- |
| datadir     | `$APP_DATA_DIR/default` |
| RPC address | 127.0.0.1                     |
| RPC port    | 61000                         |

`$APP_DATA_DIR` is platform specific:
* on Linux will usually refer to `$HOME/.local/share/golem`
* on Mac will usually refer to `$HOME/Library/Application Support/golem`
* on Windows will usually refer to `{FOLDERID_LocalAppData}/golem/golem`

If any of the above information is not correct for your Golem configuration, you can
adjust them directly in the command-line as follows

```
g_flite --address 127.0.0.1 --port 61000 --datadir /abs/path/to/golem/datadir some_text_input.txt some_speech_output.wav
```

By default `g-flite` will split your input text into 6 subtasks and compute them
on Golem Network. You can also adjust this option in the command-line as follows

```
g_flite --subtasks 2 some_text_input.txt some_speech_output.wav
```

You can also control the timeout values for the Golem task and subtasks (by default, task timeout is set
to 10 minutes, while subtask timeout to 1 minute) which can be adjusted as follows

```
g_flite --task_timeout 00:20:00 --subtask_timeout 00:05:00 some_text_input.txt some_speech_output.wav
```

Finally, you can also adjust the bid value for the Golem task (which by default is set to `1.0`)

```
g_flite --bid 1.0 some_text_input.txt some_speech_output.wav
```

All of this information can also be extracted from the command-line with the `-h` or `--help` flags

```
g_flite 0.4.0
Golem RnD Team <contact@golem.network>
flite, a text-to-speech program, distributed over Golem network

USAGE:
    g_flite [FLAGS] [OPTIONS] <input> <output>

FLAGS:
    -h, --help       
            Prints help information

        --mainnet    
            Configures golem-client to use mainnet datadir

    -V, --version    
            Prints version information

    -v, --verbose    
            Turns verbose logging on


OPTIONS:
        --address <address>                    
            Sets RPC address to Golem instance [default: 127.0.0.1]

        --bid <bid>                            
            Sets bid value for Golem task [default: 1.0]

        --datadir <datadir>                    
            Sets path to Golem datadir

        --port <port>                          
            Sets RPC port to Golem instance [default: 61000]

        --subtask_timeout <subtask_timeout>    
            Sets Golem's subtask timeout value [default: 00:10:00]

        --subtasks <subtasks>                  
            Sets number of Golem subtasks [default: 6]

        --task_timeout <task_timeout>          
            Sets Golem's task timeout value [default: 00:10:00]

        --workspace <workspace>                
            Sets workspace dir
            
            This option is mainly used for debugging the gWasm task as it allows you to specify the exact path to the
            workspace where the contents of the entire gWasm task will be stored. Note that it will *not* be
            automatically removed after the app finishes successfully; instead, it is your responsibility to clean up
            after yourself.

ARGS:
    <input>     
            Input text file

    <output>    
            Output WAV file

```

## Issues
This program is still very much a work-in-progress, so if you find (and you most likely will) any bugs,
please submit them [in our issue tracker](https://github.com/golemfactory/g-flite/issues/new).

## License
Licensed under [GNU General Public License v3.0](LICENSE) with the exception of `flite` WASM binary
which is licensed under [BSD-like License](LICENSE.flite).

