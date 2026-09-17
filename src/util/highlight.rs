use crate::lang::ast::lexer::{LanguageMode, Lexer, TokenizerMode};
use crate::lang::ast::token::Token;
use crate::lang::command::{Command, Parameter};
use crate::lang::command_invocation::resolve_external_command;
use crate::lang::errors::{CrushError, CrushResult};
use crate::lang::state::scope::Scope;
use crate::lang::value::{Value, ValueType};
use crate::util::html_escape;
use std::cmp::min;
use std::collections::HashMap;

enum CommandContext {
    Unknown,
    Known(Value),
}

pub fn highlight_colors(scope: &Scope) -> HashMap<String, String> {
    if let Ok(Value::Dict(highlight)) = scope.get_absolute_path(vec![
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
    }
}

/// Every token of `code`, in order, paired with the presentation category
/// (matching a key of the `crush:highlight` color map, or a `tok-<name>`
/// CSS class for HTML output) that applies to it, if any. Shared by both
/// output modes below so the (fairly involved) command/argument-tracking
/// state machine only needs to live in one place.
fn classify_tokens<'a>(
    code: &'a str,
    scope: &Option<Scope>,
) -> CrushResult<Vec<(Token<'a>, Option<&'static str>)>> {
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

    let mut result = Vec::with_capacity(tokens.len());

    for idx in 0..tokens.len() {
        let tok = tokens[idx];
        let ntok = tokens.get(idx + 1);

        new_command = match (new_command, prev, tok, ntok) {
            // $(...) always opens a fresh job, exactly like `{` or a `;`/
            // pipe separator -- the token right after it is a command
            // start, not a continuation of whatever came before the `$(`.
            // Without this, something like `$(files --recurse /)` colored
            // `files` as a plain string, since the state machine only
            // reset on BlockStart/Separator/Pipe.
            (
                _,
                _,
                Token::BlockStart(_) | Token::Separator(_, _) | Token::Pipe(_) | Token::SubStart(_),
                _,
            ) => true,
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

        let category = get_category(
            tok,
            ntok,
            new_command,
            scope,
            &current_command,
            &expected_argument_type,
        );
        result.push((tok, category));

        prev = Some(tok);
    }
    Ok(result)
}

pub fn syntax_highlight(
    code: &str,
    colors: &HashMap<String, String>,
    scope: &Option<Scope>,
) -> CrushResult<String> {
    let classified = classify_tokens(code, scope)?;
    let mut res = String::new();
    let mut pos = 0;

    for (tok, category) in classified {
        res.push_str(&code[pos..min(tok.location().start, code.len())]);
        let color = category.and_then(|c| colors.get(c));
        let do_reset = match color {
            Some(color) if !color.is_empty() => {
                res.push_str(color);
                true
            }
            _ => false,
        };

        res.push_str(&code[tok.location().start..min(tok.location().end, code.len())]);

        if do_reset {
            res.push_str("\x1b[0m");
        }
        pos = tok.location().end;
    }
    Ok(res)
}

/// Renders `code` (a snippet of crush source, e.g. one command's example
/// block) as HTML: each token wrapped in `<span class="tok-<category>">`.
/// Unlike [`syntax_highlight`], this isn't tied to a live scope -- example
/// text in documentation has no running interpreter behind it, so `scope`
/// is always `None` here, which also means every bare leading-position
/// token is optimistically classified as a "command" (see `get_category`'s
/// `None`-scope branch) rather than flagged as unresolvable. A command
/// token additionally gets a `data-ref="<token text>"` attribute so a
/// downstream pass that *does* know the full set of documented command
/// paths (see `generate_docs.crush`) can turn genuine matches into links,
/// leaving anything that merely looks like a command alone.
pub fn syntax_highlight_html(code: &str) -> CrushResult<String> {
    let classified = classify_tokens(code, &None)?;
    let mut res = String::new();
    let mut pos = 0;
    let mut i = 0;

    while i < classified.len() {
        let (tok, category) = classified[i];
        res.push_str(&html_escape(
            &code[pos..min(tok.location().start, code.len())],
        ));

        if category == Some("command") {
            // A multi-segment command path (dns:query_reverse, io:base64:to)
            // lexes as separate String/MemberOperator/String/... tokens --
            // there's nothing at the lexer level distinguishing it from a
            // $value:method access. Stitch a whole chain of them back into
            // one link candidate, since that's the one thing a reader (or
            // the downstream doc-path lookup in generate_docs.crush) can
            // actually resolve; a bare "dns" or "query_reverse" half isn't
            // a real path on its own.
            let start = tok.location().start;
            let mut end = tok.location().end;
            let mut j = i + 1;
            while j + 1 < classified.len()
                && matches!(classified[j].0, Token::MemberOperator(_))
                && classified[j + 1].1 == Some("command")
            {
                end = classified[j + 1].0.location().end;
                j += 2;
            }
            let escaped = html_escape(&code[start..min(end, code.len())]);
            res.push_str(&format!(
                "<span class=\"tok-command\" data-ref=\"{escaped}\">{escaped}</span>"
            ));
            pos = end;
            i = j;
            continue;
        }

        let text = &code[tok.location().start..min(tok.location().end, code.len())];
        let escaped = html_escape(text);
        match category {
            // A bare identifier ($float, $one_of, ...) is how a type
            // that's just being referenced -- an argument to one_of, a
            // column type, the right-hand side of `:like` -- gets
            // written, as opposed to invoked; it never chains through
            // MemberOperator into a multi-segment path the way a command
            // reference does, so this just needs the one token, minus its
            // leading `$` (documented paths never include the sigil).
            Some(cat @ "identifier") => {
                let name = html_escape(&text[1..]);
                res.push_str(&format!(
                    "<span class=\"tok-{cat}\" data-ref=\"{name}\">{escaped}</span>"
                ));
            }
            Some(cat) => res.push_str(&format!("<span class=\"tok-{cat}\">{escaped}</span>")),
            None => res.push_str(&escaped),
        }
        pos = tok.location().end;
        i += 1;
    }
    Ok(res)
}

fn get_category(
    token: Token,
    next_token_type: Option<&Token>,
    new_command: bool,
    scope: &Option<Scope>,
    current_command: &Option<Command>,
    expected_argument_type: &Option<ValueType>,
) -> Option<&'static str> {
    use crate::lang::ast::token::Token::*;

    if let (Some(expected), Some(actual)) = (expected_argument_type, token_type(token, scope)) {
        if *expected != ValueType::Any && *expected != actual {
            return Some("error");
        }
    }

    match token {
        String(name, _) => {
            if new_command {
                if current_command.is_some() {
                    Some("command")
                } else {
                    match next_token_type {
                        Some(MemberOperator(_)) => Some("command"),
                        _ => match scope {
                            None => Some("command"),
                            Some(_) => match resolve_external_command(name) {
                                Ok(Some(_)) => Some("command"),
                                _ => Some("error"),
                            },
                        },
                    }
                }
            } else {
                match (current_command, next_token_type) {
                    (Some(cmd), Some(Token::Equals(_))) => {
                        if allowed_named_argument(cmd.completion_data(), name) {
                            Some("named_argument")
                        } else {
                            Some("error")
                        }
                    }
                    _ => Some("string_literal"),
                }
            }
        }

        QuotedString(_, _) => Some("string_literal"),
        Flag(name, _) => match current_command {
            Some(cmd) => {
                if name.len() > 2 && allowed_named_argument(cmd.completion_data(), &name[2..]) {
                    Some("named_argument")
                } else {
                    Some("error")
                }
            }
            _ => Some("named_argument"),
        },
        Regex(_, _) => Some("regex_literal"),
        Glob(_, _) => Some("glob_literal"),
        Comment(_, _) => Some("comment"),
        File(_, _) | QuotedFile(_, _) => Some("file_literal"),
        Float(_, _) | Integer(_, _) | Duration(_, _) => Some("numeric_literal"),
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
        | BlockStart(_) => Some("operator"),
        Identifier(name, _) => match scope {
            None => Some("identifier"),
            Some(s) => match (s.get(&name[1..]).unwrap_or(None), next_token_type) {
                (Some(_), Some(Declare(_))) => Some("error"),
                (Some(_), _) => Some("identifier"),
                (None, Some(Declare(_))) => Some("identifier"),
                (None, _) => Some("error"),
            },
        },
        Background(_) => None,
        Separator(_, _) => None,
        For(_) | While(_) | Loop(_) | If(_) | Else(_) | Match(_) | Default(_) | Return(_)
        | Break(_) | Continue(_) => Some("keyword"),
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
        Token::Duration(_, _) => Some(ValueType::Duration),
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
        Token::Match(_) => None,
        Token::Default(_) => None,
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
