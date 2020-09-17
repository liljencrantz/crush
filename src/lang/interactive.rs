use rustyline;

use rustyline::error::ReadlineError;
use rustyline::{Editor, Context, validate, Config, CompletionType, EditMode, OutputStreamType};
use crate::util::file::home;
use std::path::PathBuf;
use crate::lang::data::scope::Scope;
use crate::lang::printer::Printer;
use crate::lang::stream::ValueSender;
use crate::lang::threads::ThreadStore;
use crate::lang::errors::{CrushResult, to_crush_error};
use crate::lang::execute;

use rustyline::completion::{Completer, Pair};
use rustyline_derive::Helper;
use rustyline::highlight::{MatchingBracketHighlighter, Highlighter};
use rustyline::validate::{Validator, ValidationResult};
use rustyline::hint::{HistoryHinter, Hinter};
use std::borrow::Cow::{Borrowed, Owned};
use std::borrow::Cow;
use crate::util::directory_lister::directory_lister;
use crate::lang::parser::{ast, close_command};
use crate::lang::global_state::GlobalState;

#[derive(Helper)]
struct MyHelper {
    scope: Scope,
    highlighter: MatchingBracketHighlighter,
    hinter: HistoryHinter,
    colored_prompt: String,
}

impl MyHelper {
    fn complete_internal(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> CrushResult<(usize, Vec<Pair>)> {
        let mut res = crate::lang::completion::complete(
            line, pos, &self.scope, &directory_lister())?;
        let crunched = res.drain(..)
            .map(|c| Pair {
                display: c.display().to_string(),
                replacement: c.replacement().to_string(),
            }).collect();
        Ok((pos, crunched))
    }

}

impl Completer for MyHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &Context<'_>,
    ) -> Result<(usize, Vec<Pair>), ReadlineError> {
        match self.complete_internal(line, pos, ctx) {
            Ok(res) => Ok(res),
            Err(err) => {
                println!("Error! {}", err.message());
                Err(ReadlineError::Interrupted)
            },
        }
    }
}

impl Hinter for MyHelper {
    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        self.hinter.hint(line, pos, ctx)
    }
}

impl Highlighter for MyHelper {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        default: bool,
    ) -> Cow<'b, str> {
        if default {
            Borrowed(&self.colored_prompt)
        } else {
            Borrowed(prompt)
        }
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Owned("\x1b[1m".to_owned() + hint + "\x1b[m")
    }

    fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
        self.highlighter.highlight(line, pos)
    }

    fn highlight_char(&self, line: &str, pos: usize) -> bool {
        self.highlighter.highlight_char(line, pos)
    }
}

impl Validator for MyHelper {
    fn validate(
        &self,
        ctx: &mut validate::ValidationContext,
    ) -> rustyline::Result<validate::ValidationResult> {
        let input = ctx.input().to_string();
        if let Ok(closed) = close_command(&input) {
            match ast(&closed) {
                Ok(_) => Ok(
                    if closed == input {
                        ValidationResult::Valid(None)
                    } else {
                        ValidationResult::Incomplete
                    }),
                Err(_) => Ok(ValidationResult::Invalid(None)),
            }
        } else {
            Ok(ValidationResult::Invalid(None))
        }
    }

    fn validate_while_typing(&self) -> bool {
        true
    }
}

fn crush_history_file() -> PathBuf {
    home()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".crush_history")
}

pub fn run_interactive(
    global_env: Scope,
    pretty_printer: &ValueSender,
    global_state: &GlobalState,
) -> CrushResult<()> {
    global_state.printer().line("Welcome to Crush");
    global_state.printer().line(r#"Type "help" for... help."#);

    let config = Config::builder()
        .history_ignore_space(true)
        .completion_type(CompletionType::List)
        .edit_mode(EditMode::Emacs)
        .output_stream(OutputStreamType::Stdout)
        .build();

    let h = MyHelper {
        scope: global_env.clone(),
        highlighter: MatchingBracketHighlighter::new(),
        hinter: HistoryHinter {},
        colored_prompt: "crush# ".to_owned(),
    };

    let mut rl = Editor::with_config(config);
    rl.set_helper(Some(h));
    let _ = rl.load_history(&crush_history_file());
    loop {
        let readline = rl.readline("crush# ");

        match readline {
            Ok(cmd) if cmd.is_empty() => global_state.threads().reap(global_state.printer()),
            Ok(cmd) => {
                rl.add_history_entry(&cmd);
                global_state.threads().reap(global_state.printer());
                execute::string(
                    global_env.clone(),
                    &cmd,
                    pretty_printer,
                    global_state,
                );
                global_state.threads().reap(global_state.printer());
            }
            Err(ReadlineError::Interrupted) => {
                global_state.printer().line("^C");
            }
            Err(ReadlineError::Eof) => {
                global_state.printer().line("exit");
                break;
            }
            Err(err) => {
                global_state.printer().handle_error::<()>(to_crush_error(Err(err)));
                break;
            }
        }

        if let Err(err) = rl.save_history(&crush_history_file()) {
            global_state.printer().line(&format!("Error: Failed to save history: {}", err))
        }
    }
    Ok(())
}
