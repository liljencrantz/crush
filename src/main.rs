#[macro_use]
extern crate lalrpop_util;

mod lang;
mod lib;
mod util;

use crate::lang::errors::{to_crush_error, CrushResult, argument_error_legacy};
use crate::lang::pretty::create_pretty_printer;
use crate::lang::{execute, printer};
use lib::declare;
use std::io::Read;
use std::path::PathBuf;
use lang::data;
use crate::lang::interactive;
use crate::lang::global_state::GlobalState;
use crate::lang::printer::Printer;

#[derive(PartialEq, Eq)]
enum Mode {
    Interactive,
    Pup,
    File(PathBuf),
    Help,
}

struct Config {
    mode: Mode,
}

fn parse_args() -> CrushResult<Config> {
    let args = std::env::args().collect::<Vec<_>>();
    let mut mode = Mode::Interactive;
    let mut all_files = false;
    for arg in &args[1..] {
        if all_files {
            mode = Mode::File(PathBuf::from(arg))
        } else {
            match arg.as_str() {
                "--pup" | "-p" => mode = Mode::Pup,
                "--interactive" | "-i" => mode = Mode::Interactive,
                "--help" | "-h" => mode = Mode::Help,
                "--" => all_files = true,
                file => {
                    if file.starts_with("-") {
                        return argument_error_legacy(format!("Unknown argument {}", file));
                    }
                    mode = Mode::File(PathBuf::from(file))
                }
            }
        }
    }
    Ok(Config { mode })
}

fn print_help(printer: &Printer) {
    printer.line("Usage: crush [OPTION]... [FILE]...");
    printer.line("Run the Crush shell");
    printer.line("");
    printer.line("  -h, --help        Print this message and exit");
    printer.line("  -i --interactive  Run in interactive mode (this is the default)");
    printer.line("  -p --pup          Read pup-serialized closure from standard input,");
    printer.line("                      execute, and serialize output to pup-format");
    printer.line("                      and send to standard output");
    printer.line("");
    printer.line("Crush can be run in three modes.");
    printer.line("");
    printer.line("- With no arguments, Crush starts in interactive mode, and commands will be read from");
    printer.line("  standard input.");
    printer.line("- With a filename as the only argument, that file will be executed in non-interactive");
    printer.line("  mode.");
    printer.line("- With the argument \"--pup\", a closure serialized to pup format will be read from");
    printer.line("  standard input, and executed. The output of the closure will be written in pup-format");
    printer.line("  to standard output. This third mode is used by e.g. sudo and remote:exec to run");
    printer.line("  closures in a different process.");
}

fn run() -> CrushResult<i32> {
    let config = parse_args()?;

    let root_scope = data::scope::Scope::create_root();
    let local_scope = root_scope.create_child(&root_scope, false);

    let (printer, print_handle) = if config.mode == Mode::Pup {
        printer::noop()
    } else {
        printer::init()
    };

    let global_state = GlobalState::new(printer)?;
    let pretty_printer = create_pretty_printer(global_state.printer().clone(), &global_state);

    declare(&root_scope, &global_state, &pretty_printer)?;

    match config.mode {
        Mode::Interactive => interactive::run(
            local_scope,
            &pretty_printer,
            &global_state,
        )?,

        Mode::Pup => {
            let mut buff = Vec::new();
            to_crush_error(std::io::stdin().read_to_end(&mut buff))?;
            execute::pup(
                local_scope,
                &buff,
                &global_state,
            )?;
        }

        Mode::File(f) => {
            execute::file(
                local_scope,
                f.as_path(),
                &pretty_printer,
                &global_state,
            )?
        }

        Mode::Help => {
            print_help(&global_state.printer())
        }
    }
    let status = global_state.exit_status().unwrap_or(0);
    global_state.threads().join(global_state.printer());
    drop(pretty_printer);
    drop(global_state);
    root_scope.clear()?;
    drop(root_scope);
    let _ = print_handle.join();
    Ok(status)
}

fn main() {
    let status = match run() {
        Ok(status) => status,
        Err(err) => {
            eprintln!("Error during initialization or shutdown: {}", err.message());
            1
        }
    };
    std::process::exit(status);
}
