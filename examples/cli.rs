use std::{collections::HashMap, io::Write};

use argh::FromArgs;
use compiler_rs_lib::compile;
use log::{LevelFilter, Log, info};

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

#[derive(FromArgs)]
/// Compiler CLI.
struct Cli {
    #[argh(subcommand)]
    command: Command,
}

#[derive(FromArgs)]
#[argh(subcommand)]
enum Command {
    Ast(AstCmd),
    Fmt(FmtCmd),
    Lsp(LspCmd),
}

#[derive(FromArgs)]
/// Print the AST and diagnostics for a file.
#[argh(subcommand, name = "ast")]
struct AstCmd {
    #[argh(positional)]
    file: String,
}

#[derive(FromArgs)]
/// Format a file and write the result to stdout.
#[argh(subcommand, name = "fmt")]
struct FmtCmd {
    #[argh(positional)]
    file: String,
}

#[derive(FromArgs)]
/// Run the language server over stdio.
#[argh(subcommand, name = "lsp")]
struct LspCmd {}

fn init_logger(level: LevelFilter) {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(level))
        .unwrap();
}

fn main() {
    let cli: Cli = argh::from_env();

    match cli.command {
        Command::Ast(AstCmd { file }) => {
            init_logger(LevelFilter::Trace);

            let file_content = std::fs::read_to_string(&file).unwrap();
            let mut file_id_to_name = HashMap::new();
            file_id_to_name.insert(1, file.clone());

            let compiled = compile(&file_content, 1);

            for err in &compiled.errors {
                err.write(&mut std::io::stderr(), &file_content, &file_id_to_name)
                    .unwrap();
                eprintln!()
            }
            for ctrl in &compiled.control_directives {
                ctrl.log(&file_id_to_name);
            }
            for comm in &compiled.comments {
                comm.log(&file_id_to_name);
            }
            for attr in &compiled.attributes {
                info!("{}: attribute", attr.origin.display(&file_id_to_name));
            }
            for (name, decl) in &compiled.declarations {
                info!(
                    "{}: declaration: name={} kind={:?}",
                    decl.origin.display(&file_id_to_name),
                    name,
                    decl.kind
                );
            }
            if let Some(root) = compiled.ast_root {
                compiler_rs_lib::ast::log(&compiled.ast_nodes, root, 0, &file_id_to_name);
            } else {
                info!("no root node, nothing to log: {:#?}", &compiled);
            }
            if !compiled.errors.is_empty() {
                std::process::exit(1)
            };
        }
        Command::Fmt(FmtCmd { file }) => {
            init_logger(LevelFilter::Trace);

            let file_content = std::fs::read_to_string(&file).unwrap();
            let mut file_id_to_name = HashMap::new();
            file_id_to_name.insert(1, file.clone());

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
                    &compiled.comments,
                    &compiled.control_directives,
                    &compiled.attributes,
                    &file_content,
                )
                .unwrap();
                stdout.flush().unwrap();
            }
        }
        Command::Lsp(_) => {
            init_logger(LevelFilter::Error);
            let mut stdout = std::io::stdout().lock();
            let mut stdin = std::io::stdin().lock();
            compiler_rs_lib::lsp::run(&mut stdin, &mut stdout);
        }
    }
}
