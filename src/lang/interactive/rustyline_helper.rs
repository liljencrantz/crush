use rustyline::validate::{ValidationResult, Validator};
use rustyline::{validate, Context};
use std::borrow::Cow::{Owned, Borrowed};
use std::borrow::Cow;
use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::highlight::{Highlighter, MatchingBracketHighlighter};
use rustyline::error::ReadlineError;
use rustyline::completion::{Pair, Completer};
use crate::lang::errors::CrushResult;
use std::cmp::min;
use crate::lang::ast::TokenType;
use crate::lang::value::Value;
use crate::util::directory_lister::directory_lister;
use crate::lang::data::scope::Scope;
use rustyline_derive::Helper;
use crate::lang::global_state::GlobalState;

#[derive(Helper)]
pub struct RustylineHelper {
    state: GlobalState,
    scope: Scope,
    highlighter: MatchingBracketHighlighter,
    hinter: HistoryHinter,
}

impl RustylineHelper {
    pub fn new(state: GlobalState, scope: Scope) -> RustylineHelper {
        RustylineHelper {
            state,
            scope,
            highlighter: MatchingBracketHighlighter::new(),
            hinter: HistoryHinter {},
        }
    }

    fn complete_internal(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> CrushResult<(usize, Vec<Pair>)> {
        let mut res = crate::lang::completion::complete(
            line, pos, &self.scope, &self.state.parser(),&directory_lister())?;
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
            use TokenType::*;
            let res = match token_type {
                QuotedString => highlight.get(&Value::string("string_literal")),
                Regex => highlight.get(&Value::string("string_literal")),
                QuotedFile => highlight.get(&Value::string("file_literal")),
                StringOrWildcard => highlight.get(&Value::string("label")),
                Integer => highlight.get(&Value::string("numeric_literal")),
                Float => highlight.get(&Value::string("numeric_literal")),
                Field => highlight.get(&Value::string("field")),
                Pipe | LogicalOperator | UnaryOperator | TermOperator | FactorOperator |
                ComparisonOperator | AssignmentOperator | GetItemEnd | GetItemStart | SubEnd |
                SubStart | JobEnd | JobStart =>
                    highlight.get(&Value::string("operator")),
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

    fn highlight_internal(&self, line: &str, _cursor: usize) -> CrushResult<String> {
        let mut res = String::new();
        let mut pos = 0;
        for tok in self.state.parser().tokenize(
            &self.state.parser().close_token(line))? {
            if tok.start >= line.len() {
                break;
            }
            res.push_str(&line[pos..tok.start]);
            let mut do_reset = false;
            match self.get_color(tok.token_type) {
                Some(color) => {
                    if !color.is_empty() {
                        do_reset = true;
                        res.push_str(&color);
                    }
                }
                None => {}
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
    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        self.hinter.hint(line, pos, ctx)
    }
}

impl Highlighter for RustylineHelper {
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

    fn highlight_char(&self, _line: &str, _pos: usize) -> bool {
        true
    }
}

impl Validator for RustylineHelper {
    fn validate(
        &self,
        ctx: &mut validate::ValidationContext,
    ) -> rustyline::Result<validate::ValidationResult> {
        let input = ctx.input().to_string();
        if let Ok(closed) = self.state.parser().close_command(&input) {
            match self.state.parser().ast(&closed) {
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
