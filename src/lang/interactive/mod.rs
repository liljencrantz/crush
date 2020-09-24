use rustyline;

use rustyline::error::ReadlineError;
use rustyline::{Editor, Context, validate, Config, CompletionType, EditMode, OutputStreamType};
use crate::util::file::home;
use std::path::PathBuf;
use crate::lang::data::scope::Scope;
use crate::lang::pipe::{ValueSender, empty_channel, pipe, black_hole};
use crate::lang::errors::{CrushResult, to_crush_error, data_error, error};
use crate::lang::execute;

use rustyline::completion::{Completer, Pair};
use rustyline_derive::Helper;
use rustyline::highlight::{MatchingBracketHighlighter, Highlighter};
use rustyline::validate::{Validator, ValidationResult};
use rustyline::hint::{HistoryHinter, Hinter};
use std::borrow::Cow::{Borrowed, Owned};
use std::borrow::Cow;
use crate::util::directory_lister::directory_lister;
use crate::lang::parser::{ast, close_command, tokenize};
use crate::lang::global_state::GlobalState;
use crate::lang::command::Command;
use crate::lang::command_invocation::CommandInvocation;
use crate::lang::value::{ValueDefinition, Value};
use crate::lang::ast::{Location, TokenType};
use crate::lang::execution_context::JobContext;
use std::cmp::min;

const DEFAULT_PROMPT: &'static str = "crush# ";

#[derive(Helper)]
struct MyHelper {
    scope: Scope,
    highlighter: MatchingBracketHighlighter,
    hinter: HistoryHinter,
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

    fn get_color(&self, token_type: TokenType) -> Option<String> {
        if let Ok(Value::Dict(highlight)) = self.scope.get_absolute_path(
            vec!["global".to_string(), "crush".to_string(), "highlight".to_string()]) {
            let res = match token_type {
                TokenType::QuotedString => highlight.get(&Value::string("string_literal")),
                TokenType::Regex => highlight.get(&Value::string("string_literal")),
                TokenType::QuotedLabel => highlight.get(&Value::string("file_literal")),
                TokenType::Label => highlight.get(&Value::string("label")),
                TokenType::Integer => highlight.get(&Value::string("numeric_literal")),
                TokenType::Float => highlight.get(&Value::string("numeric_literal")),
                TokenType::Pipe |
                TokenType::LogicalOperator |
                TokenType::UnaryOperator |
                TokenType::TermOperator |
                TokenType::FactorOperator |
                TokenType::ComparisonOperator |
                TokenType::AssignmentOperator |
                TokenType::GetItemEnd |
                TokenType::GetItemStart |
                TokenType::SubEnd |
                TokenType::SubStart |
                TokenType::JobEnd |
                TokenType::JobStart => highlight.get(&Value::string("operator")),
                _ => None,
            };
            match res {
                Some(Value::String(s)) => Some(s),
                _ => None,
            }
        } else {
            None
        }
    }

    fn highlight_internal(&self, line: &str, cursor: usize) -> CrushResult<String> {
        let mut res = String::new();
        let mut pos = 0;
        for tok in tokenize(&close_command(line)?)? {
            if tok.start >= line.len() {
                break;
            }
            res.push_str(&line[pos..tok.start]);
            let mut do_reset = true;
            match self.get_color(tok.token_type) {
                Some(color) => {
                    if color.is_empty() {
                        do_reset = false;
                    } else {
                        res.push_str(&color);
                    }
                }
                None => { do_reset = false; }
            }

            res.push_str(&line[tok.start..min(tok.end, line.len())]);

            if do_reset {
                res.push_str("\x1b[0m");
            }
            pos = tok.end;
        }
        Ok(res)
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
            }
        }
    }
}

impl Hinter for MyHelper {
    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        self.hinter.hint(line, pos, ctx)
    }
}

impl Highlighter for MyHelper {
    fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
        match self.highlight_internal(line, pos) {
            Ok(s) => Cow::Owned(s),
            Err(_) => Cow::Borrowed(line)
        }
    }

    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        _default: bool,
    ) -> Cow<'b, str> {
        Borrowed(prompt)
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Owned("\x1b[1m".to_owned() + hint + "\x1b[m")
    }

    fn highlight_char(&self, line: &str, pos: usize) -> bool {
        true
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

pub fn execute_prompt(
    prompt: Option<Command>,
    env: &Scope,
    global_state: &GlobalState,
) -> CrushResult<Option<String>> {
    match prompt {
        None => Ok(None),
        Some(prompt) => {
            let cmd = CommandInvocation::new(
                ValueDefinition::Value(Value::Command(prompt), Location::new(0, 0)),
                vec![]);
            let (snd, recv) = pipe();
            cmd.invoke(JobContext::new(
                empty_channel(),
                snd,
                env.clone(),
                global_state.clone(),
            ))?;
            let v = recv.recv()?;
            match v {
                Value::String(s) => Ok(Some(s)),
                _ => data_error("Wrong output type of prompt command"),
            }
        }
    }
}

pub fn load_init(
    env: &Scope,
    global_state: &GlobalState,
) -> CrushResult<()> {
    let config_dir = to_crush_error(std::env::var("XDG_CONFIG_HOME"))
        .or_else(|_| match home() {
            Ok(home) => match home.to_str() {
                Some(home) => Ok(format!("{}/.config", home)),
                None => data_error(""),
            },
            Err(e) => Err(e),
        })?;
    let config = format!("{}/crush/config.crush", config_dir);
    let file = PathBuf::from(config);
    if file.exists() {
        execute::file(env.clone(), &file, &black_hole(), global_state)
    } else {
        Ok(())
    }
}

pub fn run(
    global_env: Scope,
    pretty_printer: &ValueSender,
    global_state: &GlobalState,
) -> CrushResult<()> {
    let printer = global_state.printer().clone();
    printer.handle_error(load_init(&global_env, global_state));

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
    };

    let mut rl = Editor::with_config(config);
    rl.set_helper(Some(h));
    let _ = rl.load_history(&crush_history_file());
    loop {
        let prompt = match execute_prompt(global_state.prompt(), &global_env, global_state) {
            Ok(s) => s,
            Err(e) => {
                global_state.printer().crush_error(e);
                None
            }
        }.unwrap_or_else(|| DEFAULT_PROMPT.to_string());
        let readline = rl.readline(&prompt);

        match readline {
            Ok(cmd) if cmd.is_empty() => global_state.threads().reap(global_state.printer()),
            Ok(cmd) => {
                rl.add_history_entry(&cmd);
                global_state.threads().reap(global_state.printer());
                global_state.printer().handle_error(
                    execute::string(
                        global_env.clone(),
                        &cmd,
                        pretty_printer,
                        global_state,
                    ));
                global_state.threads().reap(global_state.printer());
                if global_state.exit_status().is_some() {
                    break;
                }
                global_state.printer().ping();
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
    if let Err(err) = rl.save_history(&crush_history_file()) {
        global_state.printer().line(&format!("Error: Failed to save history: {}", err))
    }
    Ok(())
}
