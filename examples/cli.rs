use std::{collections::HashMap, io::Write};

use compiler_rs_lib::compile;
use log::{LevelFilter, Log};

struct Logger {}

impl Log for Logger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        eprintln!("{} {} ", record.level(), record.args());
    }

    fn flush(&self) {}
}

static LOGGER: Logger = Logger {};

fn main() {
    let mut args = std::env::args().skip(1);
    let cmd = args.next().unwrap();

    match cmd.as_str() {
        "ast" => {
            log::set_logger(&LOGGER)
                .map(|()| log::set_max_level(LevelFilter::Trace))
                .unwrap();

            let file_name = args.next().unwrap();
            let file_content = std::fs::read_to_string(&file_name).unwrap();
            let mut file_id_to_name = HashMap::new();
            file_id_to_name.insert(1, file_name.clone());

            let compiled = compile(&file_content, 1);

            for err in &compiled.errors {
                err.write(&mut std::io::stderr(), &file_content, &file_id_to_name)
                    .unwrap();
                eprintln!()
            }
            if !compiled.errors.is_empty() {
                std::process::exit(1)
            };
        }
        "fmt" => {
            log::set_logger(&LOGGER)
                .map(|()| log::set_max_level(LevelFilter::Trace))
                .unwrap();

            let file_name = args.next().unwrap();
            let file_content = std::fs::read_to_string(&file_name).unwrap();
            let mut file_id_to_name = HashMap::new();
            file_id_to_name.insert(1, file_name.clone());

            let compiled = compile(&file_content, 1);

            for err in &compiled.errors {
                err.write(&mut std::io::stderr(), &file_content, &file_id_to_name)
                    .unwrap();
                eprintln!()
            }
            if !compiled.errors.is_empty() {
                std::process::exit(1)
            };
            if let Some(root) = compiled.ast_root {
                let mut stdout = std::io::stdout().lock();
                compiler_rs_lib::fmt::format(
                    &mut stdout,
                    root,
                    &compiled.ast_nodes,
                    &file_content,
                    0,
                    true,
                    true,
                )
                .unwrap();
                stdout.flush().unwrap();
            }
        }
        "lsp" => {
            log::set_logger(&LOGGER)
                .map(|()| log::set_max_level(LevelFilter::Error))
                .unwrap();
            let mut stdout = std::io::stdout().lock();
            let mut stdin = std::io::stdin().lock();
            compiler_rs_lib::lsp::run(&mut stdin, &mut stdout);
        }
        other => {
            eprintln!("unknown command: {}", other);
            std::process::exit(1);
        }
    }
}
