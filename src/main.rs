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
