use std::{collections::HashMap, io::Write};

use argh::FromArgs;
use compiler_rs_lib::{compile, origin::LineIndex};
use log::{LevelFilter, Log, info};
use markdown::mdast::Node;
use walkdir::WalkDir;

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
#[argh(help_triggers("-h", "--help", "help"))]
struct Cli {
    #[argh(subcommand)]
    command: Command,
}

#[derive(FromArgs)]
#[argh(subcommand)]
enum Command {
    Ast(AstCmd),
    Fmt(FmtCmd),
    FmtMd(FmtMdCmd),
    Lsp(LspCmd),
}

#[derive(FromArgs)]
/// Print the AST and diagnostics for a file.
#[argh(subcommand, name = "ast", help_triggers("-h", "--help", "help"))]
struct AstCmd {
    #[argh(positional)]
    file: String,
}

#[derive(FromArgs)]
/// Format a file and write the result to stdout, or back to the file with `-i`.
#[argh(subcommand, name = "fmt", help_triggers("-h", "--help", "help"))]
struct FmtCmd {
    #[argh(positional)]
    file: String,

    /// format the file in-place instead of writing to stdout.
    #[argh(switch, short = 'i')]
    in_place: bool,
}

#[derive(FromArgs)]
/// Format dtrace code blocks inside a markdown file, leaving the rest unchanged.
#[argh(subcommand, name = "fmt-md", help_triggers("-h", "--help", "help"))]
struct FmtMdCmd {
    #[argh(positional)]
    file: String,

    /// format the file in-place instead of writing to stdout.
    #[argh(switch, short = 'i')]
    in_place: bool,
}

#[derive(FromArgs)]
/// Run the language server over stdio.
#[argh(subcommand, name = "lsp", help_triggers("-h", "--help", "help"))]
struct LspCmd {}

fn init_logger(level: LevelFilter) {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(level))
        .unwrap();
}

/// Compile `source` and write the formatted output to `out`. Returns `false`
/// if compilation produced errors (which are written to stderr); in that case
/// `out` is not written to.
///
/// Diagnostics are reported against `enclosing`, in which `source` begins at
/// byte `source_offset`. For a `.d` file the two are the same text at offset
/// zero. For a fenced block inside a markdown file they differ, and shifting
/// the origins is what lets a diagnostic name a line — and show an excerpt —
/// in terms of the markdown file rather than the block.
fn format_dtrace(
    out: &mut impl Write,
    source: &str,
    enclosing: &str,
    source_offset: usize,
    file_name: &str,
) -> bool {
    let mut file_id_to_name = HashMap::new();
    file_id_to_name.insert(1, file_name.to_owned());

    let mut compiled = compile(source, 1);

    let line_index = LineIndex::new(enclosing);
    let shift = source_offset as u32;
    for err in &mut compiled.errors {
        err.origin.start += shift;
        err.origin.end += shift;
        if let Some(related) = &mut err.related_origin {
            related.start += shift;
            related.end += shift;
        }
        err.write(&mut std::io::stderr(), &line_index, &file_id_to_name)
            .unwrap();
        eprintln!()
    }
    if !compiled.errors.is_empty() {
        return false;
    }
    compiler_rs_lib::fmt::format(
        out,
        compiled.ast_root,
        &compiled.ast_nodes,
        &compiled.comments,
        &compiled.control_directives,
        &compiled.attributes,
        source,
    )
    .unwrap();
    true
}

/// Recursively collect `(start_offset, end_offset, value)` for every fenced
/// code block whose info string starts with `dtrace`.
fn collect_dtrace_blocks<'a>(node: &'a Node, out: &mut Vec<(usize, usize, &'a str)>) {
    if let Node::Code(code) = node
        && code.lang.as_deref() == Some("dtrace")
        && let Some(pos) = &code.position
    {
        out.push((pos.start.offset, pos.end.offset, code.value.as_str()));
        return;
    }
    if let Some(children) = node.children() {
        for child in children {
            collect_dtrace_blocks(child, out);
        }
    }
}

/// Replace every `dtrace` fenced code block in `content` with its formatted
/// equivalent, preserving the surrounding markdown. Returns `None` if any
/// block failed to compile (diagnostics are written to stderr).
fn format_markdown(content: &str, file_name: &str) -> Option<String> {
    let tree = markdown::to_mdast(content, &markdown::ParseOptions::default()).unwrap();
    let mut blocks = Vec::new();
    collect_dtrace_blocks(&tree, &mut blocks);
    blocks.sort_by_key(|(start, _, _)| *start);

    let mut out = String::with_capacity(content.len());
    let mut cursor = 0;
    let mut had_error = false;
    for (start, end, value) in blocks {
        out.push_str(&content[cursor..start]);
        let block_text = &content[start..end];
        // `value` is the inner code text, present verbatim in the block —
        // splicing it (rather than rebuilding the fence) preserves the
        // original fence style (` ``` ` vs `~~~`, info string, indentation).
        let Some(value_offset) = block_text.find(value) else {
            out.push_str(block_text);
            cursor = end;
            continue;
        };
        let mut formatted = Vec::new();
        if !format_dtrace(
            &mut formatted,
            value,
            content,
            start + value_offset,
            file_name,
        ) {
            had_error = true;
            out.push_str(block_text);
            cursor = end;
            continue;
        }
        let mut formatted = String::from_utf8(formatted).unwrap();
        // The formatter emits a trailing newline; in markdown, the final
        // newline before the closing fence is part of the fence syntax, not
        // the code value — so drop it to avoid a blank line.
        if formatted.ends_with('\n') {
            formatted.pop();
        }
        out.push_str(&block_text[..value_offset]);
        out.push_str(&formatted);
        out.push_str(&block_text[value_offset + value.len()..]);
        cursor = end;
    }
    out.push_str(&content[cursor..]);

    if had_error { None } else { Some(out) }
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
            let line_index = LineIndex::new(&file_content);

            for err in &compiled.errors {
                err.write(&mut std::io::stderr(), &line_index, &file_id_to_name)
                    .unwrap();
                eprintln!()
            }
            for ctrl in &compiled.control_directives {
                ctrl.log(&file_id_to_name, &line_index);
            }
            for comm in &compiled.comments {
                comm.log(&file_id_to_name, &line_index);
            }
            for attr in &compiled.attributes {
                info!(
                    "{}: attribute",
                    attr.origin.display(&file_id_to_name, &line_index)
                );
            }
            for (name, decl) in &compiled.declarations {
                info!(
                    "{}: declaration: name={} kind={:?}",
                    decl.origin.display(&file_id_to_name, &line_index),
                    name,
                    decl.kind
                );
            }
            if let Some(root) = compiled.ast_root {
                compiler_rs_lib::ast::log(
                    &compiled.ast_nodes,
                    root,
                    0,
                    &file_id_to_name,
                    &line_index,
                );
            } else {
                info!("no root node, nothing to log: {:#?}", &compiled);
            }
            if !compiled.errors.is_empty() {
                std::process::exit(1)
            };
        }
        Command::Fmt(FmtCmd {
            file: file_path,
            in_place,
        }) => {
            init_logger(LevelFilter::Trace);

            match std::fs::read_to_string(&file_path) {
                Ok(file_content) => {
                    fmt_file(&file_path, in_place, file_content);
                }
                Err(err) if err.kind() == std::io::ErrorKind::IsADirectory => {
                    for entry in WalkDir::new(&file_path) {
                        match entry {
                            Ok(file)
                                if file.file_type().is_file()
                                    && file.path().extension() == Some("d".as_ref()) =>
                            {
                                match std::fs::read_to_string(file.path()) {
                                    Ok(file_content) => fmt_file(
                                        &file.into_path().to_string_lossy().to_string(),
                                        in_place,
                                        file_content,
                                    ),
                                    Err(err) => {
                                        eprintln!(
                                            "failed to read {}: {}",
                                            file.path().display(),
                                            err
                                        );
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Err(err) => panic!("{}", err),
            };
        }
        Command::FmtMd(FmtMdCmd {
            file: file_path,
            in_place,
        }) => {
            init_logger(LevelFilter::Trace);

            match std::fs::read_to_string(&file_path) {
                Ok(file_content) => {
                    fmt_md_file(file_path, in_place, &file_content);
                }
                Err(err) if err.kind() == std::io::ErrorKind::IsADirectory => {
                    for entry in WalkDir::new(&file_path) {
                        match entry {
                            Ok(file)
                                if file.file_type().is_file()
                                    && file.path().extension() == Some("md".as_ref()) =>
                            {
                                match std::fs::read_to_string(file.path()) {
                                    Ok(file_content) => fmt_md_file(
                                        file.into_path().to_string_lossy().to_string(),
                                        in_place,
                                        &file_content,
                                    ),
                                    Err(err) => {
                                        eprintln!(
                                            "failed to read {}: {}",
                                            file.path().display(),
                                            err
                                        );
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Err(err) => panic!("{}", err),
            };
        }
        Command::Lsp(_) => {
            init_logger(LevelFilter::Error);
            // Safe to buffer: `Message::write_payload` flushes after every
            // response, so the client never waits on bytes sitting in the
            // buffer.
            let mut stdout = std::io::BufWriter::new(std::io::stdout().lock());
            let mut stdin = std::io::stdin().lock();
            compiler_rs_lib::lsp::run(&mut stdin, &mut stdout);
        }
    }
}

/// Stdout for bulk output.
///
/// `StdoutLock` is line-buffered, so the formatter's many small writes cost
/// one `write(2)` per output line. A 1.1 MB file spent more time in the
/// kernel than in the formatter.
fn buffered_stdout() -> std::io::BufWriter<std::io::StdoutLock<'static>> {
    std::io::BufWriter::with_capacity(1 << 16, std::io::stdout().lock())
}

fn fmt_md_file(file: String, in_place: bool, file_content: &str) {
    let Some(output) = format_markdown(&file_content, &file) else {
        std::process::exit(1);
    };
    if in_place {
        std::fs::write(&file, output.as_bytes()).unwrap();
    } else {
        let mut stdout = buffered_stdout();
        stdout.write_all(output.as_bytes()).unwrap();
        stdout.flush().unwrap();
    }
}

/// Re-compiles formatted output before it is allowed to replace the file it
/// came from.
///
/// Overwriting the user's file is not reversible, so a formatter bug must
/// cost a refused write rather than the file. Without this, an input the
/// lexer accepted but mis-modelled came back as output that no longer
/// parses, with the original already gone.
fn verify_formatted(formatted: &[u8], file_path: &str) -> Result<(), String> {
    let text = std::str::from_utf8(formatted)
        .map_err(|err| format!("the formatted output is not valid UTF-8: {err}"))?;

    let compiled = compile(text, 1);
    if compiled.errors.is_empty() {
        return Ok(());
    }

    let mut file_id_to_name = HashMap::new();
    file_id_to_name.insert(1, file_path.to_owned());
    let line_index = LineIndex::new(text);
    for err in &compiled.errors {
        err.write(&mut std::io::stderr(), &line_index, &file_id_to_name)
            .unwrap();
        eprintln!();
    }
    Err(String::from("the formatted output does not parse"))
}

fn fmt_file(file_path: &String, in_place: bool, file_content: String) {
    if in_place {
        let mut buf = Vec::new();
        if !format_dtrace(&mut buf, &file_content, &file_content, 0, file_path) {
            std::process::exit(1);
        }
        if let Err(err) = verify_formatted(&buf, file_path) {
            eprintln!("{file_path}: refusing to overwrite the file: {err}");
            std::process::exit(1);
        }
        std::fs::write(file_path, &buf).unwrap();
    } else {
        let mut stdout = buffered_stdout();
        if !format_dtrace(&mut stdout, &file_content, &file_content, 0, file_path) {
            std::process::exit(1);
        }
        stdout.flush().unwrap();
    }
}
