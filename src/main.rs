#[macro_use]
extern crate lalrpop_util;

mod lang;
mod lib;
mod util;

use rustyline;

use crate::lang::errors::{to_crush_error, CrushResult, argument_error};
use crate::lang::pretty::create_pretty_printer;
use crate::lang::printer::Printer;
use lang::data::scope::Scope;
use crate::lang::stream::ValueSender;
use crate::lang::{execute, printer};
use crate::util::file::home;
use lib::declare;
use rustyline::error::ReadlineError;
use rustyline::Editor;
use std::io::Read;
use std::path::PathBuf;
use crate::lang::threads::ThreadStore;
use lang::data;

fn crush_history_file() -> PathBuf {
    home()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".crush_history")
}

fn run_interactive(
    global_env: Scope,
    printer: &Printer,
    pretty_printer: &ValueSender,
    threads: &ThreadStore,
) -> CrushResult<()> {
    printer.line("Welcome to Crush");
    printer.line(r#"Type "help" for... help."#);

    let mut rl = Editor::<()>::new();
    let _ = rl.load_history(&crush_history_file());
    loop {
        let readline = rl.readline("crush# ");

        match readline {
            Ok(cmd) if cmd.is_empty() => threads.reap(&printer),
            Ok(cmd) => {
                rl.add_history_entry(&cmd);
                threads.reap(&printer);
                execute::string(global_env.clone(), &cmd, &printer, pretty_printer, threads);
                threads.reap(&printer);
            }
            Err(ReadlineError::Interrupted) => {
                printer.line("^C");
            }
            Err(ReadlineError::Eof) => {
                printer.line("exit");
                break;
            }
            Err(err) => {
                printer.handle_error::<()>(to_crush_error(Err(err)));
                break;
            }
        }

        if let Err(err) = rl.save_history(&crush_history_file()) {
            printer.line(&format!("Error: Failed to save history: {}", err))
        }
    }
    Ok(())
}

#[derive(PartialEq, Eq)]
enum Mode {
    Interactive,
    Pup,
    File(PathBuf),
}

struct Config {
    mode: Mode,
}

fn parse_args() -> CrushResult<Config> {
    let args = std::env::args().collect::<Vec<_>>();

    let mode = match &args[..] {
        [_exe] => Mode::Interactive,
        [_exe, arg] => {
            if arg == "--pup" {
                Mode::Pup
            } else {
                Mode::File(PathBuf::from(&arg))
            }
        }
        _ => return argument_error("Invalid input parameters"),
    };
    Ok(Config { mode })
}

fn run() -> CrushResult<()> {
    let global_env = data::scope::Scope::create_root();
    let threads = ThreadStore::new();

    let my_scope = global_env.create_child(&global_env, false);

    let config = parse_args()?;

    let (mut printer, mut print_handle) = if config.mode == Mode::Pup { printer::noop() } else { printer::init() };
    let pretty_printer = create_pretty_printer(printer.clone());
    declare(&global_env, &printer, &threads, &pretty_printer)?;

    match config.mode {
        Mode::Interactive => run_interactive(my_scope, &printer, &pretty_printer, &threads)?,
        Mode::Pup => {
            let mut buff = Vec::new();
            to_crush_error(std::io::stdin().read_to_end(&mut buff))?;
            execute::pup(my_scope, &buff, &printer, &threads)?;
        }
        Mode::File(f) => {
            execute::file(
                my_scope,
                f.as_path(),
                &printer,
                &pretty_printer,
                &threads,
            )?
        }
    }

    threads.join(&printer);
    drop(pretty_printer);
    drop(printer);
    drop(threads);
    global_env.clear();
    drop(global_env);
    let _ = print_handle.join();
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error during initialization: {}", err.message());
    }
}
