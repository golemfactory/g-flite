#![allow(dead_code)]

pub use failure::Error;
use serde::Serialize;
use std::path::PathBuf;

pub struct ResponseTable {
    pub columns: Vec<String>,
    pub values: Vec<serde_json::Value>,
}

impl ResponseTable {
    pub fn sort_by(mut self, arg_key: &Option<impl AsRef<str>>) -> Self {
        let key = match arg_key {
            None => return self,
            Some(k) => k.as_ref(),
        };
        let idx =
            match self
                .columns
                .iter()
                .enumerate()
                .find_map(|(idx, v)| if v == key { Some(idx) } else { None })
            {
                None => return self,
                Some(idx) => idx,
            };
        self.values
            .sort_by_key(|v| Some(v.as_array()?.get(idx)?.to_string()));
        self
    }
}

#[derive(Debug)]
pub enum CommandResponse {
    NoOutput,
    Object(serde_json::Value),
    Table {
        columns: Vec<String>,
        values: Vec<serde_json::Value>,
    },
}

impl CommandResponse {
    pub fn object<T: Serialize>(value: T) -> Result<Self, Error> {
        Ok(CommandResponse::Object(serde_json::to_value(value)?))
    }
}

impl From<ResponseTable> for CommandResponse {
    fn from(table: ResponseTable) -> Self {
        CommandResponse::Table {
            columns: table.columns,
            values: table.values,
        }
    }
}

pub struct CliCtx {
    pub rpc_addr: (String, u16),
    pub data_dir: PathBuf,
    pub json_output: bool,
}

impl CliCtx {
    pub fn connect_to_app(
        &mut self,
    ) -> Result<(actix::SystemRunner, impl actix_wamp::RpcEndpoint + Clone), Error> {
        let mut sys = actix::System::new("golemcli");

        let data_dir = self.data_dir.clone();

        let auth_method =
            actix_wamp::challenge_response_auth(move |auth_id| -> Result<_, std::io::Error> {
                let secret_file_path = data_dir.join(format!("crossbar/secrets/{}.tck", auth_id));
                log::debug!("reading secret from: {}", secret_file_path.display());
                Ok(std::fs::read(secret_file_path)?)
            });

        let (address, port) = &self.rpc_addr;

        let endpoint = sys.block_on(
            actix_wamp::SessionBuilder::with_auth("golem", "golemcli", auth_method)
                .create_wss(address, *port),
        )?;

        Ok((sys, endpoint))
    }

    pub fn message(&mut self, message: &str) {
        eprintln!("{}", message);
    }

    pub fn output(&self, resp: CommandResponse) {
        match resp {
            CommandResponse::NoOutput => {}
            CommandResponse::Table { columns, values } => {
                if self.json_output {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "headers": columns,
                            "values": values
                        }))
                        .unwrap()
                    )
                } else {
                    print_table(columns, values);
                }
            }
            CommandResponse::Object(v) => {
                if self.json_output {
                    println!("{}", serde_json::to_string_pretty(&v).unwrap())
                } else {
                    match v {
                        serde_json::Value::String(s) => {
                            println!("{}", s);
                        }
                        v => println!("{}", serde_yaml::to_string(&v).unwrap()),
                    }
                }
            }
        }
    }
}

fn print_table(columns: Vec<String>, values: Vec<serde_json::Value>) {
    use prettytable::*;
    let mut table = Table::new();
    //table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);
    table.set_format(*FORMAT_BASIC);

    table.set_titles(Row::new(
        columns
            .iter()
            .map(|c| {
                Cell::new(c)
                    .with_style(Attr::Bold)
                    .with_style(Attr::ForegroundColor(color::GREEN))
            })
            .collect(),
    ));
    if values.is_empty() {
        let _ = table.add_row(columns.iter().map(|_| Cell::new("")).collect());
    }
    for row in values {
        if let Some(row_items) = row.as_array() {
            use serde_json::Value;

            let row_strings = row_items
                .iter()
                .map(|v| match v {
                    Value::String(s) => s.to_string(),
                    Value::Null => "".into(),
                    v => v.to_string(),
                })
                .collect();
            table.add_row(row_strings);
        }
    }
    let _ = table.printstd();
}

use prettytable::{format, format::TableFormat};
lazy_static::lazy_static! {

    pub static ref FORMAT_BASIC: TableFormat = format::FormatBuilder::new()
        .column_separator('│')
        .borders('│')
        .separators(
            &[format::LinePosition::Top],
            format::LineSeparator::new('─', '┬', '┌', '┐')
        )
        .separators(
            &[format::LinePosition::Title],
            format::LineSeparator::new('─', '┼', '├', '┤')
        )
        .separators(
            &[format::LinePosition::Bottom],
            format::LineSeparator::new('─', '┴', '└', '┘')
        )
        .padding(2, 2)
        .build();
}
