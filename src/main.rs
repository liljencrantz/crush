#[macro_use]
extern crate lalrpop_util;

mod lang;
mod lib;
mod util;

use rustyline;

use rustyline::error::ReadlineError;
use rustyline::Editor;
use lib::declare;
use crate::lang::errors::{CrushResult, to_crush_error};
use crate::lang::{printer, execute};
use crate::lang::pretty_printer::create_pretty_printer;
use crate::util::file::home;
use std::path::{PathBuf, Path};
use crate::lang::scope::Scope;
use crate::lang::printer::Printer;
use crate::lang::stream::ValueSender;
use std::io::Read;

fn crush_history_file() -> String {
    home()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(Path::new(".crush_history"))
        .to_str()
        .unwrap_or(".crush_history")
        .to_string()
}

fn run_interactive(global_env: Scope, printer: &Printer, pretty_printer: &ValueSender) -> CrushResult<()> {
    printer.line("Welcome to Crush");
    printer.line(r#"Type "help" for... help."#);

    let mut rl = Editor::<()>::new();
    let _ = rl.load_history(&crush_history_file());
    loop {
        let readline = rl.readline("crush# ");

        match readline {
            Ok(cmd) => {
                if !cmd.is_empty() {
                    rl.add_history_entry(cmd.as_str());
                    execute::string(global_env.clone(), &cmd.as_str(), &printer, pretty_printer);
                }
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
        match rl.save_history(&crush_history_file()) {
            Ok(_) => {}
            Err(_) => {
                printer.line("Error: Failed to save history.");
            }
        }
    }
    Ok(())
}

fn run() -> CrushResult<()> {
    let global_env = lang::scope::Scope::create_root();
    let (printer, print_handle) = printer::init();
    let pretty_printer = create_pretty_printer(printer.clone());
    declare(&global_env, &printer, &pretty_printer)?;
    let my_scope = global_env.create_child(&global_env, false);

    let args = std::env::args().collect::<Vec<String>>();
    match args.len() {
        1 => run_interactive(
            my_scope,
            &printer,
            &pretty_printer)?,
        2 =>
            if args[1] == "--pup" {
                let mut buff = Vec::new();
                to_crush_error(std::io::stdin().read_to_end(&mut buff))?;
                execute::pup(my_scope, &buff, &printer)?;
            } else {
                execute::file(
                    my_scope,
                    PathBuf::from(&args[1]).as_path(),
                    &printer,
                    &pretty_printer)?
            },
        _ => {}
    }
    drop(pretty_printer);
    drop(printer);
    global_env.clear();
    drop(global_env);
    let _ = print_handle.join();
    Ok(())
}

fn main() {
    match run() {
        Ok(_) => (),
        Err(e) => println!("Error during initialization: {}", e.message),
    }
}
