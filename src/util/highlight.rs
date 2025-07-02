use crate::lang::ast::lexer::{LanguageMode, Lexer, TokenizerMode};
use crate::lang::ast::token::Token;
use crate::lang::command::{Command, ParameterCompletionData};
use crate::lang::command_invocation::resolve_external_command;
use crate::lang::errors::{CrushError, CrushResult};
use crate::lang::state::scope::Scope;
use crate::lang::value::Value;
use std::cmp::min;
use std::collections::HashMap;

pub fn syntax_highlight(
    code: &str,
    colors: &HashMap<String, String>,
    scope: &Option<Scope>,
) -> CrushResult<String> {
    let mut res = String::new();
    let mut pos = 0;
    let mut new_command = true;
    let mut prev = None;

    let l = Lexer::new(code, LanguageMode::Command, TokenizerMode::IncludeComments);
    let tokens = l
        .into_iter()
        .map(|item| item.map(|it| it.1).map_err(|e| CrushError::from(e)))
        .collect::<CrushResult<Vec<Token>>>()?;

    let mut current_command = None;

    for idx in 0..tokens.len() {
        let tok = tokens[idx];
        let ntok = tokens.get(idx + 1);

        res.push_str(&code[pos..min(tok.location().start, code.len())]);
        let mut do_reset = false;

        new_command = match (new_command, tok, prev) {
            (_, Token::BlockStart(_) | Token::Separator(_, _) | Token::Pipe(_), _) => true,
            (
                true,
                Token::String(_, _) | Token::Identifier(_, _),
                Some(Token::String(_, _) | Token::Identifier(_, _)),
            ) => false,
            (true, Token::String(_, _) | Token::Identifier(_, _), _) => true,
            (
                true,
                Token::MemberOperator(_),
                Some(Token::String(_, _) | Token::Identifier(_, _)),
            ) => true,
            _ => false,
        };

        match (new_command, tok, scope) {
            (true, Token::String(name, _), Some(s)) => {
                current_command = match s.get(name) {
                    Ok(Some(Value::Command(c))) => Some(c),
                    _ => None,
                };
            }
            _ => {}
        };

        match get_color(tok, ntok, new_command, colors, scope, &current_command) {
            Some(color) => {
                if !color.is_empty() {
                    do_reset = true;
                    res.push_str(color);
                }
            }
            None => {}
        }

        res.push_str(&code[tok.location().start..min(tok.location().end, code.len())]);

        if do_reset {
            res.push_str("\x1b[0m");
        }
        pos = tok.location().end;
        prev = Some(tok);
    }
    Ok(res)
}

fn get_color<'a>(
    token_type: Token,
    next_token_type: Option<&Token>,
    new_command: bool,
    colors: &'a HashMap<String, String>,
    scope: &Option<Scope>,
    current_command: &Option<Command>,
) -> Option<&'a String> {
    use crate::lang::ast::token::Token::*;
    match token_type {
        String(name, _) => {
            if new_command {
                match scope {
                    None => colors.get("command"),
                    Some(s) => match s.get(name).unwrap_or(None) {
                        Some(_) => colors.get("command"),
                        None => match resolve_external_command(name, s) {
                            Ok(Some(_)) => colors.get("command"),
                            _ => colors.get("error"),
                        },
                    },
                }
            } else {
                match (current_command, next_token_type) {
                    (Some(cmd), Some(Token::Equals(_))) => {
                        if allowed_named_argument(cmd.completion_data(), name) {
                            colors.get("named_argument")
                        } else {
                            colors.get("error")
                        }
                    }
                    _ => colors.get("string_literal"),
                }
            }
        }

        QuotedString(quoted_name, _) => colors.get("string_literal"),
        Flag(name, _) => match (current_command) {
            Some(cmd) => {
                if name.len() > 2 && allowed_named_argument(cmd.completion_data(), &name[2..]) {
                    colors.get("named_argument")
                } else {
                    colors.get("error")
                }
            }
            _ => colors.get("named_argument"),
        },
        Regex(_, _) => colors.get("regex_literal"),
        Glob(_, _) => colors.get("glob_literal"),
        Comment(_, _) => colors.get("comment"),
        File(_, _) | QuotedFile(_, _) => colors.get("file_literal"),
        Float(_, _) | Integer(_, _) => colors.get("numeric_literal"),
        Unnamed(_)
        | Named(_)
        | Pipe(_)
        | LogicalOperator(_, _)
        | UnaryOperator(_, _)
        | ComparisonOperator(_, _)
        | Equals(_)
        | Declare(_)
        | GetItemEnd(_)
        | GetItemStart(_)
        | SubEnd(_)
        | Bang(_)
        | Plus(_)
        | Minus(_)
        | Star(_)
        | Slash(_)
        | MemberOperator(_)
        | ExprModeStart(_)
        | SubStart(_)
        | BlockEnd(_)
        | BlockStart(_) => colors.get("operator"),
        Identifier(name, _) => match scope {
            None => colors.get("identifier"),
            Some(s) => match (s.get(&name[1..]).unwrap_or(None), next_token_type) {
                (Some(_), Some(Declare(_))) => colors.get("error"),
                (Some(_), _) => colors.get("identifier"),
                (None, Some(Declare(_))) => colors.get("identifier"),
                (None, _) => colors.get("error"),
            },
        },
        Separator(_, _) => None,
        For(_) | While(_) | Loop(_) | If(_) | Else(_) | Return(_) | Break(_) | Continue(_) => {
            colors.get("keyword")
        }
    }
}

fn allowed_named_argument(
    parameter_completion_data: &[ParameterCompletionData],
    name: &str,
) -> bool {
    for param in parameter_completion_data {
        if param.named {
            return true;
        }
        if param.name == name {
            return true;
        }
    }
    false
}
