use crate::lang::ast::lexer::{LanguageMode, Lexer, TokenizerMode};
use crate::lang::ast::token::Token;
use crate::lang::command::{Command, Parameter};
use crate::lang::command_invocation::resolve_external_command;
use crate::lang::errors::{CrushError, CrushResult};
use crate::lang::state::scope::Scope;
use crate::lang::value::{Value, ValueType};
use std::cmp::min;
use std::collections::HashMap;

enum CommandContext {
    Unknown,
    Known(Value),
}

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

    let mut current_command: Option<Command> = None;
    let mut latest_named_argument_info: Option<(usize, String)> = None;
    let mut command_context = match scope {
        None => CommandContext::Unknown,
        Some(s) => CommandContext::Known(Value::Scope(s.clone())),
    };

    for idx in 0..tokens.len() {
        let tok = tokens[idx];
        let ntok = tokens.get(idx + 1);

        res.push_str(&code[pos..min(tok.location().start, code.len())]);
        let mut do_reset = false;

        new_command = match (new_command, prev, tok, ntok) {
            (_, _, Token::BlockStart(_) | Token::Separator(_, _) | Token::Pipe(_), _) => true,
            (
                true,
                Some(Token::String(_, _) | Token::Identifier(_, _)),
                Token::String(_, _) | Token::Identifier(_, _),
                _,
            ) => false,
            (true, _, Token::String(_, _) | Token::Identifier(_, _), _) => true,
            (true, Some(Token::String(s, _)), Token::MemberOperator(_), _) => {
                match &command_context {
                    CommandContext::Known(v) => match v.field(s) {
                        Ok(Some(v)) => command_context = CommandContext::Known(v),
                        _ => command_context = CommandContext::Unknown,
                    },
                    _ => {}
                }
                true
            }
            (true, Some(Token::Identifier(s, _)), Token::MemberOperator(_), _) => {
                match &command_context {
                    CommandContext::Known(v) => match v.field(&s[1..]) {
                        Ok(Some(v)) => command_context = CommandContext::Known(v),
                        _ => command_context = CommandContext::Unknown,
                    },
                    _ => {}
                }
                true
            }
            _ => false,
        };

        match (new_command, tok, ntok) {
            (true, Token::String(name, _), _) => {
                current_command = match &command_context {
                    CommandContext::Unknown => None,
                    CommandContext::Known(ctx) => ctx.field(name).unwrap_or(None).and_then(|v| {
                        if let Value::Command(cmd) = v {
                            Some(cmd)
                        } else {
                            None
                        }
                    }),
                };
            }
            (false, Token::String(name, _), Some(Token::Equals(_))) => {
                latest_named_argument_info = Some((idx, name.to_string()));
            }
            _ => {}
        };

        let expected_argument_type = if let Some((name_idx, name)) = &latest_named_argument_info
            && idx == name_idx + 2
        {
            match &current_command {
                None => None,
                Some(s) => named_argument_type(s.completion_data(), name),
            }
        } else {
            None
        };

        match get_color(
            tok,
            ntok,
            new_command,
            colors,
            scope,
            &current_command,
            &expected_argument_type,
        ) {
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
    token: Token,
    next_token_type: Option<&Token>,
    new_command: bool,
    colors: &'a HashMap<String, String>,
    scope: &Option<Scope>,
    current_command: &Option<Command>,
    expected_argument_type: &Option<ValueType>,
) -> Option<&'a String> {
    use crate::lang::ast::token::Token::*;

    if let (Some(expected), Some(actual)) = (expected_argument_type, token_type(token, scope)) {
        if *expected != ValueType::Any && *expected != actual {
            return colors.get("error");
        }
    }

    match token {
        String(name, _) => {
            if new_command {
                if current_command.is_some() {
                    colors.get("command")
                } else {
                    match next_token_type {
                        Some(MemberOperator(_)) => colors.get("command"),
                        _ => match scope {
                            None => colors.get("command"),
                            Some(s) => match resolve_external_command(name, s) {
                                Ok(Some(_)) => colors.get("command"),
                                _ => colors.get("error"),
                            },
                        },
                    }
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

        QuotedString(_, _) => colors.get("string_literal"),
        Flag(name, _) => match current_command {
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
        Background(_) => None,
        Separator(_, _) => None,
        For(_) | While(_) | Loop(_) | If(_) | Else(_) | Return(_) | Break(_) | Continue(_) => {
            colors.get("keyword")
        }
    }
}

fn allowed_named_argument(parameter_completion_data: &[Parameter], name: &str) -> bool {
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

fn token_type(token: Token, scope: &Option<Scope>) -> Option<ValueType> {
    match token {
        Token::LogicalOperator(_, _) => None,
        Token::UnaryOperator(_, _) => None,
        Token::ComparisonOperator(_, _) => None,
        Token::Bang(_) => None,
        Token::Plus(_) => None,
        Token::Minus(_) => None,
        Token::Star(_) => None,
        Token::Slash(_) => None,
        Token::QuotedString(_, _) => Some(ValueType::String),
        Token::Comment(_, _) => None,
        Token::Identifier(id, _) => match scope {
            None => None,
            Some(s) => match s.get(&id[1..]) {
                Ok(Some(v)) => Some(v.value_type()),
                _ => None,
            },
        },
        Token::Flag(_, _) => None,
        Token::QuotedFile(_, _) => Some(ValueType::File),
        Token::Glob(_, _) => Some(ValueType::Glob),
        Token::File(_, _) => Some(ValueType::File),
        Token::String(_, _) => Some(ValueType::String),
        Token::Regex(_, _) => Some(ValueType::Regex),
        Token::Integer(_, _) => Some(ValueType::Integer),
        Token::Float(_, _) => Some(ValueType::Float),
        Token::MemberOperator(_) => None,
        Token::Equals(_) => None,
        Token::Declare(_) => None,
        Token::Separator(_, _) => None,
        Token::Background(_) => None,
        Token::SubStart(_) => None,
        Token::SubEnd(_) => None,
        Token::BlockStart(_) => None,
        Token::BlockEnd(_) => None,
        Token::GetItemStart(_) => None,
        Token::GetItemEnd(_) => None,
        Token::Pipe(_) => None,
        Token::Unnamed(_) => None,
        Token::Named(_) => None,
        Token::ExprModeStart(_) => None,
        Token::For(_) => None,
        Token::While(_) => None,
        Token::Loop(_) => None,
        Token::If(_) => None,
        Token::Else(_) => None,
        Token::Return(_) => None,
        Token::Break(_) => None,
        Token::Continue(_) => None,
    }
}

fn named_argument_type(parameter_completion_data: &[Parameter], name: &str) -> Option<ValueType> {
    let mut default = None;
    for param in parameter_completion_data {
        if param.named {
            default = Some(param.value_type.clone());
        }
        if param.name == name {
            return Some(param.value_type.clone());
        }
    }
    default
}
