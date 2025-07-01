use crate::lang::ast::lexer::LanguageMode;
use crate::lang::ast::lexer::LanguageMode::Command;
use crate::lang::errors::CrushResult;
use crate::lang::state::global_state::GlobalState;
use crate::lang::state::scope::Scope;
use crate::lang::value::Value;
use crate::util::directory_lister::directory_lister;
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::{CmdKind, Highlighter, MatchingBracketHighlighter};
use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::validate::{ValidationResult, Validator};
use rustyline::{Context, validate};
use rustyline_derive::Helper;
use std::borrow::Cow;
use std::borrow::Cow::{Borrowed, Owned};
use std::collections::HashMap;

#[derive(Helper)]
pub struct RustylineHelper {
    state: GlobalState,
    scope: Scope,
    highlighter: MatchingBracketHighlighter,
    hinter: HistoryHinter,
    mode: LanguageMode,
}

impl RustylineHelper {
    pub fn new(state: GlobalState, scope: Scope) -> RustylineHelper {
        RustylineHelper {
            state,
            scope,
            highlighter: MatchingBracketHighlighter::new(),
            hinter: HistoryHinter {},
            mode: Command,
        }
    }

    fn complete_internal(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> CrushResult<(usize, Vec<Pair>)> {
        let mut res = crate::lang::completion::complete(
            line,
            pos,
            &self.scope,
            &self.state.parser(),
            &directory_lister(),
        )?;
        let crunched = res
            .drain(..)
            .map(|c| Pair {
                display: c.display().to_string(),
                replacement: c.replacement().to_string(),
            })
            .collect();
        Ok((pos, crunched))
    }

    fn highlight_internal(&self, line: &str, _cursor: usize) -> CrushResult<String> {
        let map: HashMap<String, String> = if let Ok(Value::Dict(highlight)) =
            self.scope.get_absolute_path(vec![
                "global".to_string(),
                "crush".to_string(),
                "highlight".to_string(),
            ]) {
            highlight
                .elements()
                .into_iter()
                .map(|e| (e.0.to_string(), e.1.to_string()))
                .collect()
        } else {
            HashMap::new()
        };
        crate::util::highlight::syntax_highlight(line, &map)
    }
}

impl Completer for RustylineHelper {
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

impl Hinter for RustylineHelper {
    type Hint = String;
    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        self.hinter.hint(line, pos, ctx)
    }
}

impl Highlighter for RustylineHelper {
    fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
        match self.highlight_internal(line, pos) {
            Ok(s) => Cow::Owned(s),
            Err(_) => Cow::Borrowed(line),
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

    fn highlight_char(&self, _line: &str, _pos: usize, _forced: CmdKind) -> bool {
        true
    }
}

impl Validator for RustylineHelper {
    fn validate(
        &self,
        _ctx: &mut validate::ValidationContext,
    ) -> rustyline::Result<validate::ValidationResult> {
        return Ok(ValidationResult::Valid(None));

        let input = _ctx.input().to_string();
        if input.trim() == "!!" {
            return Ok(ValidationResult::Valid(None));
        }
        if let Ok(closed) = self.state.parser().close_command(&input) {
            match self.state.parser().ast(&closed, self.mode) {
                Ok(_) => Ok(if closed == input {
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
