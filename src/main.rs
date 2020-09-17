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
use crate::lang::threads::ThreadStore;
use lang::data;
use crate::lang::interactive::run_interactive;
use crate::lang::global_state::GlobalState;

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
        _ => return argument_error_legacy("Invalid input parameters"),
    };
    Ok(Config { mode })
}

fn run() -> CrushResult<()> {
    let global_env = data::scope::Scope::create_root();
    let my_scope = global_env.create_child(&global_env, false);
    let config = parse_args()?;

    let (printer, print_handle) = if config.mode == Mode::Pup { printer::noop() } else { printer::init() };
    let global_state = GlobalState::new(printer)?;
    let pretty_printer = create_pretty_printer(global_state.printer().clone(), &global_state);

    declare(&global_env, &global_state, &pretty_printer)?;

    match config.mode {
        Mode::Interactive => run_interactive(
            my_scope,
            &pretty_printer,
            &global_state,
        )?,
        Mode::Pup => {
            let mut buff = Vec::new();
            to_crush_error(std::io::stdin().read_to_end(&mut buff))?;
            execute::pup(
                my_scope,
                &buff,
                &global_state,
            )?;
        }
        Mode::File(f) => {
            execute::file(
                my_scope,
                f.as_path(),
                &pretty_printer,
                &global_state,
            )?
        }
    }

    global_state.threads().join(global_state.printer());
    drop(pretty_printer);
    drop(global_state);
    global_env.clear()?;
    drop(global_env);
    let _ = print_handle.join();
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error during initialization or shutdown: {}", err.message());
    }
}
