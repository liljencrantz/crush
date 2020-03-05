mod replace;
mod lang;
mod lib;
mod util;

use crate::lang::lexer::Lexer;

extern crate rustyline;

use rustyline::error::ReadlineError;
use rustyline::Editor;
use lib::declare;
use crate::lang::errors::{CrushResult, to_crush_error};
use std::error::Error;
use crate::lang::printer::Printer;
use crate::lang::stream::empty_channel;
use crate::lang::stream_printer::spawn_print_thread;
use crate::util::file::home;
use std::path::Path;
use std::fs;
use crate::lang::parser::parse;

fn crush_history_file() -> Box<str> {
    Box::from(
        home()
            .unwrap_or(Box::from(Path::new(".")))
            .join(Path::new(".crush_history"))
            .to_str()
            .unwrap_or(".crush_history"))
}

fn run_interactive(global_env: lang::scope::Scope, printer: &Printer) -> CrushResult<()> {
    let mut rl = Editor::<()>::new();
    rl.load_history(crush_history_file().as_ref());
    loop {
        let readline = rl.readline("crush> ");

        match readline {
            Ok(cmd) => {
                if !cmd.is_empty() {
                    rl.add_history_entry(cmd.as_str());
                    match parse(&mut Lexer::new(&cmd)) {
                        Ok(jobs) => {
                            for job_definition in jobs {
                                let last_output = spawn_print_thread(&printer);
                                match job_definition.invoke(&global_env, printer, empty_channel(), last_output) {
                                    Ok(handle) => {
                                        handle.join(printer);
                                    }
                                    Err(e) => printer.job_error(e),
                                }
                            }
                        }
                        Err(error) => {
                            printer.job_error(error);
                        }
                    }
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
                printer.line(err.description());
                break;
            }
        }
        match rl.save_history(crush_history_file().as_ref()) {
            Ok(_) => {}
            Err(_) => {
                printer.line("Error: Failed to save history.");
            }
        }
    }
    Ok(())
}


fn run_script(global_env: lang::scope::Scope, printer: &Printer, filename: &str) -> CrushResult<()> {
    let cmd = to_crush_error(fs::read_to_string(filename))?;
    match parse(&mut Lexer::new(&cmd)) {
        Ok(jobs) => {
            for job_definition in jobs {
                let last_output = spawn_print_thread(&printer);
                match job_definition.invoke(&global_env, printer, empty_channel(), last_output) {
                    Ok(handle) => {
                        handle.join(printer);
                    }
                    Err(e) => printer.job_error(e),
                }
            }
        }
        Err(error) => {
            printer.job_error(error);
        }
    }
    Ok(())
}

fn run() -> CrushResult<()> {
    let global_env = lang::scope::Scope::new();
    let (printer, printer_handle) = Printer::new();

    declare(&global_env)?;
    let my_scope = global_env.create_child(&global_env, false);

    let mut args = std::env::args().collect::<Vec<String>>();
    match args.len() {
        1 => run_interactive(my_scope, &printer)?,
        2 => run_script(my_scope, &printer, args[1].as_str())?,
        _ => {}
    }
//    std::thread::sleep(Duration::from_secs(1));
//    printer.shutdown();
    drop(printer);
    printer_handle.join();
    Ok(())
}

fn main() {
    match run() {
        Ok(_) => (),
        Err(e) => println!("Error during initialization: {}", e.message),
    }
}
