use rustyline;

use rustyline::error::ReadlineError;
use rustyline::Editor;
use crate::util::file::home;
use std::path::PathBuf;
use crate::lang::data::scope::Scope;
use crate::lang::printer::Printer;
use crate::lang::stream::ValueSender;
use crate::lang::threads::ThreadStore;
use crate::lang::errors::{CrushResult, to_crush_error};
use crate::lang::execute;


fn crush_history_file() -> PathBuf {
    home()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".crush_history")
}

pub fn run_interactive(
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
