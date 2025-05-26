use rustyline::validate::{ValidationResult, Validator};
use rustyline::{validate, Context};
use std::borrow::Cow::{Owned, Borrowed};
use std::borrow::Cow;
use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::highlight::{CmdKind, Highlighter, MatchingBracketHighlighter};
use rustyline::error::ReadlineError;
use rustyline::completion::{Pair, Completer};
use crate::lang::errors::CrushResult;
use std::cmp::min;
use crate::lang::ast::token::Token;
use crate::lang::value::Value;
use crate::util::directory_lister::directory_lister;
use crate::lang::state::scope::Scope;
use rustyline_derive::Helper;
use crate::lang::ast::lexer::{LexerMode, TokenizerMode};
use crate::lang::ast::lexer::LexerMode::Command;
use crate::lang::state::global_state::GlobalState;

#[derive(Helper)]
pub struct RustylineHelper {
    state: GlobalState,
    scope: Scope,
    highlighter: MatchingBracketHighlighter,
    hinter: HistoryHinter,
    mode: LexerMode,
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
            line, pos, &self.scope, &self.state.parser(), &directory_lister())?;
        let crunched = res.drain(..)
            .map(|c| Pair {
                display: c.display().to_string(),
                replacement: c.replacement().to_string(),
            }).collect();
        Ok((pos, crunched))
    }

    fn get_color(&self, token_type: Token, new_command: bool) -> Option<String> {
        if let Ok(Value::Dict(highlight)) = self.scope.get_absolute_path(
            vec!["global".to_string(), "crush".to_string(), "highlight".to_string()]) {
            use Token::*;

            let res =
                match token_type {
                    String(_, _) | QuotedString(_, _) =>
                        if new_command {
                            highlight.get(&Value::from("command"))
                        } else {
                            highlight.get(&Value::from("string_literal"))
                        },
                    Flag(_, _) => highlight.get(&Value::from("string_literal")),
                    Regex(_, _) => highlight.get(&Value::from("regex_literal")),
                    Glob(_, _) => highlight.get(&Value::from("glob_literal")),
                    Comment(_, _) => highlight.get(&Value::from("comment")),
                    File(_, _) | QuotedFile(_, _) => highlight.get(&Value::from("file_literal")),
                    Float(_, _) | Integer(_, _) => highlight.get(&Value::from("numeric_literal")),
                    Unnamed(_) | Named(_) | Pipe(_) | LogicalOperator(_, _) | UnaryOperator(_, _) |
                    ComparisonOperator(_, _) | Equals(_) | Declare(_) | GetItemEnd(_) | GetItemStart(_) | SubEnd(_) |
                    Bang(_) | Plus(_) | Minus(_) | Star(_) | Slash(_) | MemberOperator(_) | ExprModeStart(_) |
                    SubStart(_) | BlockEnd(_) | BlockStart(_) =>
                        highlight.get(&Value::from("operator")),
                    Identifier(_, _) => None,
                    Separator(_, _) => None,
                    For(_) |
                    While(_) |
                    Loop(_) |
                    If(_) |
                    Else(_) |
                    Return(_) |
                    Break(_) |
                    Continue(_) => highlight.get(&Value::from("keyword")),
                };

            match res {
                Some(Value::String(s)) => Some(s.to_string()),
                _ => None,
            }
        } else {
            None
        }
    }

    fn highlight_internal(&self, line: &str, _cursor: usize) -> CrushResult<String> {
        let mut res = String::new();
        let mut pos = 0;
        let mut new_command = true;
        let mut prev = None;
        for tok in self.state.parser().tokenize(
            &self.state.parser().close_token(line),
            self.mode,
            TokenizerMode::IncludeComments)? {
            if pos >= line.len() {
                break;
            }
            if tok.location().start >= line.len() {
                break;
            }
            res.push_str(&line[pos..min(tok.location().start, line.len())]);
            let mut do_reset = false;

            new_command = match (new_command, tok, prev) {
                (_, Token::BlockStart(_) | Token::Separator(_, _) | Token::Pipe(_), _) => true,
                (true, Token::String(_, _) | Token::Identifier(_, _), Some(Token::String(_, _) | Token::Identifier(_, _))) => false,
                (true, Token::String(_, _) | Token::Identifier(_, _), _) => true,
                (true, Token::MemberOperator(_), Some(Token::String(_, _) | Token::Identifier(_, _))) => true,
                _ => false,
            };

            match self.get_color(tok, new_command) {
                Some(color) => {
                    if !color.is_empty() {
                        do_reset = true;
                        res.push_str(&color);
                    }
                }
                None => {}
            }

            res.push_str(&line[tok.location().start..min(tok.location().end, line.len())]);

            if do_reset {
                res.push_str("\x1b[0m");
            }
            pos = tok.location().end;
            prev = Some(tok);
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
    type Hint = String;
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
