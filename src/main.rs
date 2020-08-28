#[macro_use]
extern crate lalrpop_util;

mod lang;
mod lib;
mod util;

use rustyline;

use crate::lang::errors::{to_crush_error, CrushResult};
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
use crate::util::identity_arc::Identity;
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
            Ok(cmd) if cmd.is_empty() => {}
            Ok(cmd) => {
                rl.add_history_entry(&cmd);
                execute::string(global_env.clone(), &cmd, &printer, pretty_printer, threads);
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

fn run() -> CrushResult<()> {
    let global_env = data::scope::Scope::create_root();
    let (printer, print_handle) = printer::init();
    let pretty_printer = create_pretty_printer(printer.clone());
    let threads = ThreadStore::new();
    declare(&global_env, &printer, &threads, &pretty_printer)?;

    let my_scope = global_env.create_child(&global_env, false);

    let args = std::env::args().collect::<Vec<_>>();
    match &args[..] {
        [_exe] => run_interactive(my_scope, &printer, &pretty_printer, &threads)?,
        [_exe, arg] => {
            if arg == "--pup" {
                let mut buff = Vec::new();
                to_crush_error(std::io::stdin().read_to_end(&mut buff))?;
                execute::pup(my_scope, &buff, &printer, &threads)?;
            } else {
                execute::file(
                    my_scope,
                    PathBuf::from(&arg).as_path(),
                    &printer,
                    &pretty_printer,
                    &threads,
                )?
            }
        }
        _ => {}
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
