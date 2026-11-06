use crate::lang::argument::{ArgumentValue, SwitchStyle, ValidationArgument};
use crate::lang::ast::expand_user;
use crate::lang::ast::lexer::{LanguageMode, Lexer, TokenizerMode};
use crate::lang::ast::source::{Source, SourceType};
use crate::lang::ast::token::Token;
use crate::lang::command::{Command, OutputType, Parameter, PipeInputType};
use crate::lang::command_invocation::{is_runnable_as_command, resolve_external_command};
use crate::lang::errors::{CrushError, CrushResult};
use crate::lang::state::scope::Scope;
use crate::lang::value::{Value, ValueType};
use crate::util::escape::{unescape, unescape_file};
use crate::util::html_escape;
use std::cmp::min;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

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

/// Classifies a single argument-value token into what a command's `dynamic_output`
/// function sees for it (see `ArgumentValue`): a literal gives the real `Value`
/// directly, a `$variable` reference gives whatever it currently holds in scope (or
/// `Unknown` if unbound), and anything this highlighter doesn't resolve to a concrete
/// value (a nested `$(...)`/`{...}`, a glob, a regex, ...) falls back to `Unknown` --
/// never a guess, since an `Unknown` argument is always a safe, non-blocking input for a
/// `dynamic_output` function to receive.
fn classify_argument_value(tok: Token, scope: &Option<Scope>) -> ArgumentValue {
    match tok {
        Token::String(s, _) => ArgumentValue::Value(Value::from(s)),
        Token::QuotedString(s, _) => match unescape(s) {
            Ok(v) => ArgumentValue::Value(Value::from(v)),
            Err(_) => ArgumentValue::Unknown,
        },
        Token::Integer(s, _) => match s.replace('_', "").parse::<i128>() {
            Ok(i) => ArgumentValue::Value(Value::Integer(i)),
            Err(_) => ArgumentValue::Unknown,
        },
        Token::Float(s, _) => match s.replace('_', "").parse::<f64>() {
            Ok(f) => ArgumentValue::Value(Value::Float(f)),
            Err(_) => ArgumentValue::Unknown,
        },
        Token::Identifier(name, _) => match scope {
            Some(s) => match s.get(&name[1..]) {
                Ok(Some(v)) => ArgumentValue::Value(v),
                _ => ArgumentValue::Unknown,
            },
            None => ArgumentValue::Unknown,
        },
        _ => ArgumentValue::Unknown,
    }
}

/// Whether `tok` is a token that can stand as one unnamed argument value on its own --
/// structural tokens (operators, brackets, keywords, `=` itself, ...) never are, so they
/// never contribute a spurious `Unknown`-valued argument to a command's argument list.
fn is_argument_value_token(tok: Token) -> bool {
    matches!(
        tok,
        Token::String(_, _)
            | Token::QuotedString(_, _)
            | Token::Integer(_, _)
            | Token::Float(_, _)
            | Token::Duration(_, _)
            | Token::Glob(_, _)
            | Token::Regex(_, _)
            | Token::File(_, _)
            | Token::QuotedFile(_, _)
            | Token::Identifier(_, _)
    )
}

/// Resolves a command's declared output (whether it's the static `output = ...` or
/// whatever a `dynamic_output` function just returned) against the type that was piped
/// into it, mirroring `OutputType::calculate` -- which isn't reachable from here since
/// it's private to `crate::lang::command`.
fn resolve_output(output: &OutputType, input: &PipeInputType) -> PipeInputType {
    match output {
        OutputType::Unknown => PipeInputType::Unknown,
        OutputType::Known(t) => PipeInputType::Known(t.clone()),
        OutputType::Passthrough => match input {
            PipeInputType::Known(t) => PipeInputType::Known(t.clone()),
            _ => PipeInputType::Unknown,
        },
    }
}

/// Evaluates the command that was just scanned (if any) against the arguments collected
/// for it so far, to find out what flows into the *next* pipeline stage -- using its
/// `dynamic_output` function when it declared one, falling back to its static output
/// type (via `output_type`) otherwise. Returns `Some("error")` only when a
/// `dynamic_output` function actively rejects what's been typed so far; a command with
/// no `dynamic_output`, or one whose arguments aren't fully resolvable yet, is never
/// recolored just for that.
fn finalize_command(
    command: &Option<Command>,
    args: &[ValidationArgument],
    pipe_input: &PipeInputType,
    literal_job_output: &Option<ValueType>,
) -> (PipeInputType, Option<&'static str>) {
    let Some(cmd) = command else {
        // Not every job starts with a command invocation -- `3 | sort` pipes a bare
        // literal value into `sort` -- so a literal job head's own type still needs to
        // flow forward even though there's no `Command` to call `dynamic_output` on.
        return (
            match literal_job_output {
                Some(t) => PipeInputType::Known(t.clone()),
                None => PipeInputType::Unknown,
            },
            None,
        );
    };
    match cmd.dynamic_output() {
        Some(f) => match f(pipe_input.clone(), args) {
            Ok(output) => (resolve_output(&output, pipe_input), None),
            Err(_) => (PipeInputType::Unknown, Some("error")),
        },
        None => {
            let as_output = match pipe_input {
                PipeInputType::None | PipeInputType::Unknown => OutputType::Unknown,
                PipeInputType::Known(t) => OutputType::Known(t.clone()),
            };
            let next = match cmd.output_type(&as_output) {
                Some(t) => PipeInputType::Known(t.clone()),
                None => PipeInputType::Unknown,
            };
            (next, None)
        }
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
    // Whether the current command-position name resolves to *anything* in scope, not
    // just a `Command` -- a namespace like `fs` resolves to a `Value::Scope`, which
    // `current_command` (narrowed to `Value::Command` only, for completion-data lookups
    // like allowed argument names) can't represent. Without this, a bare namespace name
    // used as a command (e.g. `fs` alone, which is valid -- it just prints the
    // namespace's members, like `help fs` would) fell through to `resolve_external_command`,
    // which only knows about OS executables, and got colored as an error.
    let mut current_reference_exists = false;
    let mut latest_named_argument_info: Option<(usize, String)> = None;
    let mut command_context = match scope {
        None => CommandContext::Unknown,
        Some(s) => CommandContext::Known(Value::Scope(s.clone())),
    };

    // What's flowing into the command currently being scanned, the arguments collected
    // for it so far, and the `result` index of its own token (so a `dynamic_output`
    // rejection can recolor it after the fact, once its whole argument list is known).
    // Reset at every job boundary (`finalize_command`'s call sites below); like
    // `command_context` above, a nested `{...}`/`$(...)` used as an argument value is
    // (im)precisely treated as its own fresh job rather than tracked with real nesting
    // depth -- the same simplification this whole state machine already makes.
    let mut pipe_input = PipeInputType::None;
    let mut current_args: Vec<ValidationArgument> = Vec::new();
    let mut current_command_result_idx: Option<usize> = None;
    // A job doesn't have to start with a command invocation -- `3 | sort` pipes the
    // bare literal `3` into `sort` -- so this tracks a literal job head's own type,
    // separately from `current_command`, for `finalize_command` to fall back on.
    let mut literal_job_output: Option<ValueType> = None;
    let argument_source = Source::new(SourceType::Input, Arc::from(code));

    let mut result: Vec<(Token<'a>, Option<&'static str>)> = Vec::with_capacity(tokens.len());

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
            //
            // Also resets `command_context` back to the root scope: a `:` earlier in
            // the previous command (e.g. `host:procs`) narrows it into that namespace
            // so a chained path like `io:base64:to` resolves each segment correctly,
            // but a fresh job's own command name must be looked up from the top level
            // again, not as a member of whatever namespace the last command happened to
            // live in. Without this, `host:procs|count` colored `count` as an error --
            // it was looked up as a (nonexistent) member of `host`, not as the
            // ordinary, top-level command it actually is.
            (
                _,
                _,
                Token::BlockStart(_) | Token::Separator(_, _) | Token::Pipe(_) | Token::SubStart(_),
                _,
            ) => {
                let (next_input, error_category) = finalize_command(
                    &current_command,
                    &current_args,
                    &pipe_input,
                    &literal_job_output,
                );
                if let (Some(cat), Some(i)) = (error_category, current_command_result_idx) {
                    result[i].1 = Some(cat);
                }
                // A pipe carries the previous command's resolved output into this new
                // one; every other boundary (a fresh statement, block, or substitution)
                // starts a job with nothing piped in at all.
                pipe_input = match tok {
                    Token::Pipe(_) => next_input,
                    _ => PipeInputType::None,
                };
                current_args = Vec::new();
                current_command_result_idx = None;
                current_command = None;
                literal_job_output = None;
                command_context = match scope {
                    None => CommandContext::Unknown,
                    Some(s) => CommandContext::Known(Value::Scope(s.clone())),
                };
                true
            }
            (
                true,
                Some(Token::String(_, _) | Token::Identifier(_, _)),
                Token::String(_, _) | Token::Identifier(_, _),
                _,
            ) => false,
            (true, _, Token::String(_, _) | Token::Identifier(_, _), _) => true,
            // A literal path (`./build.rs`, `'my file'`) can be the head of a job too --
            // `./mydir` cd's into a directory, and any other path is spawned as an
            // executable -- exactly like a bare command name. Without these two arms
            // (mirroring the String/Identifier pair just above), a File/QuotedFile token
            // always fell through to the catch-all `_ => false` below, so `get_category`
            // never saw `new_command == true` for it and always colored it as a plain
            // `file_literal`, even when it was unrunnable and should have been an error.
            (
                true,
                Some(Token::String(_, _) | Token::Identifier(_, _)),
                Token::File(_, _) | Token::QuotedFile(_, _),
                _,
            ) => false,
            (true, _, Token::File(_, _) | Token::QuotedFile(_, _), _) => true,
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
                let resolved = match &command_context {
                    CommandContext::Unknown => None,
                    CommandContext::Known(ctx) => ctx.field(name).unwrap_or(None),
                };
                current_reference_exists = resolved.is_some();
                current_command = resolved.and_then(|v| {
                    if let Value::Command(cmd) = v {
                        Some(cmd)
                    } else {
                        None
                    }
                });
                current_command_result_idx = Some(idx);
            }
            (false, Token::String(name, _), Some(Token::Equals(_))) => {
                latest_named_argument_info = Some((idx, name.to_string()));
            }
            _ => {}
        };

        // A job's head doesn't have to be a command invocation -- `3 | sort` pipes the
        // bare literal `3` into `sort`. `new_command`'s own computation above is tuned
        // specifically for recognizing a *command name* candidate (String/Identifier/
        // File/QuotedFile) and doesn't fire for these token kinds at all, so job-start is
        // detected directly from `prev` here instead: exactly one token ever follows a
        // boundary token (or starts the whole script).
        if prev.is_none()
            || matches!(
                prev,
                Some(
                    Token::BlockStart(_)
                        | Token::Separator(_, _)
                        | Token::Pipe(_)
                        | Token::SubStart(_)
                )
            )
        {
            literal_job_output = match tok {
                Token::Integer(s, _) => s
                    .replace('_', "")
                    .parse::<i128>()
                    .ok()
                    .map(|_| ValueType::Integer),
                Token::Float(s, _) => s
                    .replace('_', "")
                    .parse::<f64>()
                    .ok()
                    .map(|_| ValueType::Float),
                Token::Duration(_, _) => Some(ValueType::Duration),
                Token::QuotedString(_, _) => Some(ValueType::String),
                Token::Glob(_, _) => Some(ValueType::Glob),
                Token::Regex(_, _) => Some(ValueType::Regex),
                _ => None,
            };
        }

        // Collect this token into the current command's argument list, for whatever
        // `dynamic_output` function it's eventually evaluated against -- skipped
        // entirely while `tok` is itself the command's own name.
        if !new_command {
            let is_named_value_slot = latest_named_argument_info
                .as_ref()
                .is_some_and(|(name_idx, _)| idx == name_idx + 2);
            let is_named_name_slot =
                matches!(tok, Token::String(_, _)) && matches!(ntok, Some(Token::Equals(_)));

            if is_named_value_slot {
                if let Some((_, name)) = &latest_named_argument_info {
                    current_args.push(ValidationArgument {
                        argument_type: Some(name.clone()),
                        switch_style: SwitchStyle::None,
                        value: classify_argument_value(tok, scope),
                        source: argument_source.clone(),
                    });
                }
            } else if let Token::Flag(name, _) = tok {
                // A flag can be `--long` or `-x` (see the lexer's `-`-prefixed arm), and
                // while the user is still mid-keystroke in an interactive session it can
                // even be a lone `-` with nothing after it -- strip however many leading
                // dashes are actually there instead of assuming exactly two.
                let flag_name = name
                    .strip_prefix("--")
                    .or_else(|| name.strip_prefix('-'))
                    .unwrap_or(name);
                current_args.push(ValidationArgument {
                    argument_type: Some(flag_name.to_string()),
                    switch_style: SwitchStyle::None,
                    value: ArgumentValue::Value(Value::Bool(true)),
                    source: argument_source.clone(),
                });
            } else if !is_named_name_slot && is_argument_value_token(tok) {
                current_args.push(ValidationArgument {
                    argument_type: None,
                    switch_style: SwitchStyle::None,
                    value: classify_argument_value(tok, scope),
                    source: argument_source.clone(),
                });
            }
        }

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
            current_reference_exists,
            &expected_argument_type,
        );
        result.push((tok, category));

        prev = Some(tok);
    }

    // The very last command in `code` has no trailing boundary token to trigger its
    // own `finalize_command` call, so it needs one explicitly here.
    let (_, error_category) =
        finalize_command(&current_command, &current_args, &pipe_input, &literal_job_output);
    if let (Some(cat), Some(i)) = (error_category, current_command_result_idx) {
        result[i].1 = Some(cat);
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
    current_reference_exists: bool,
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
                if current_command.is_some() || current_reference_exists {
                    Some("command")
                } else {
                    match next_token_type {
                        Some(MemberOperator(_)) => Some("command"),
                        _ => match scope {
                            None => Some("command"),
                            Some(_) => match resolve_external_command(name) {
                                Ok(Some(path)) if is_runnable_as_command(&path) => Some("command"),
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
        File(name, _) | QuotedFile(name, _) => {
            // As an argument (the ordinary case), a file literal's validity doesn't
            // matter for highlighting -- `cat ./nonexistent` should highlight the same
            // whether or not the file exists. Only in command position does it need the
            // same runnability check as a bare command name gets, below.
            if new_command && scope.is_some() {
                let path = if matches!(token, QuotedFile(_, _)) {
                    unescape_file(name).ok()
                } else {
                    expand_user(name).ok().map(PathBuf::from)
                };
                match path {
                    Some(p) if is_runnable_as_command(&p) => Some("file_literal"),
                    _ => Some("error"),
                }
            } else {
                Some("file_literal")
            }
        }
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
        | Mod(_)
        | Rem(_)
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
        For(_) | While(_) | Loop(_) | If(_) | Else(_) | Match(_) | Default(_) | Try(_)
        | Catch(_) | Return(_) | Break(_) | Continue(_) => Some("keyword"),
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
        Token::Mod(_) => None,
        Token::Rem(_) => None,
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
        Token::Try(_) => None,
        Token::Catch(_) => None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::state::contexts::CommandContext;
    use signature::signature;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Mutex;

    fn my_cmd(_context: CommandContext) -> CrushResult<()> {
        Ok(())
    }

    #[signature(my_cmd)]
    struct MyCmdSignature {}

    // Serializes every test below that points `$PATH` at a scratch directory --
    // `resolve_external_command` reads the real process-wide `PATH`, which `cargo test`'s
    // default multi-threaded runner would otherwise let two such tests race on.
    static PATH_TEST_LOCK: Mutex<()> = Mutex::new(());

    /// Points `$PATH` at `dir` for as long as the guard is alive, restoring whatever
    /// `$PATH` held before on drop (even if the test body panics) so one test's PATH
    /// override can never leak into another.
    struct PathGuard {
        original: Option<String>,
    }

    impl PathGuard {
        fn set(dir: &std::path::Path) -> PathGuard {
            let original = std::env::var("PATH").ok();
            unsafe {
                std::env::set_var("PATH", dir);
            }
            PathGuard { original }
        }
    }

    impl Drop for PathGuard {
        fn drop(&mut self) {
            unsafe {
                match &self.original {
                    Some(p) => std::env::set_var("PATH", p),
                    None => std::env::remove_var("PATH"),
                }
            }
        }
    }

    /// A fresh scratch directory under the OS temp dir, unique per test (by test name),
    /// so parallel tests never collide on the same path.
    fn scratch_dir(test_name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "crush_highlight_test_{}_{}",
            test_name,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A root scope with a `fs` namespace holding one command -- mirrors the shape of
    /// the real, builtin `fs` namespace (`fs:files`, `fs:mounts`, ...) closely enough to
    /// reproduce the bug without pulling in the whole builtins tree.
    fn scope_with_fs_namespace() -> Scope {
        let root = Scope::create_root();
        root.create_namespace(
            "fs",
            "bla",
            None,
            Box::new(|env| {
                MyCmdSignature::declare(env)?;
                Ok(())
            }),
        )
        .unwrap();
        root
    }

    /// A bare namespace name used as a command (`fs`, with no `:member` following it) is
    /// a real, valid reference -- it resolves to the namespace's own scope value, and
    /// running it just lists that scope's members, exactly like `help fs` would. It
    /// should never be highlighted as an error just because it isn't itself invocable.
    #[test]
    fn bare_namespace_used_as_command_is_not_an_error() {
        let scope = Some(scope_with_fs_namespace());
        let classified = classify_tokens("fs", &scope).unwrap();
        assert_eq!(classified.len(), 1);
        assert_ne!(
            classified[0].1,
            Some("error"),
            "a bare namespace name in command position should not be highlighted as an error"
        );
    }

    /// Same namespace, referenced the ordinary way (`fs:my_cmd`) -- this already worked
    /// before the fix and must keep working after it.
    #[test]
    fn namespace_member_access_is_still_a_command() {
        let scope = Some(scope_with_fs_namespace());
        let classified = classify_tokens("fs:my_cmd", &scope).unwrap();
        assert_eq!(classified[0].1, Some("command"));
        assert_eq!(classified[2].1, Some("command"));
    }

    /// A plain, top-level command piped after a namespaced one (`fs:my_cmd|count`, e.g.
    /// the reported `host:procs|count`) used to be highlighted as an error. Resolving
    /// `my_cmd` as a member of `fs` narrows the lookup context used for the *next*
    /// command-position name into the `fs` namespace too (so a chain like
    /// `io:base64:to` resolves each segment correctly) -- but nothing ever reset that
    /// context back to the root scope for the job that starts after the `|`, so `count`
    /// was looked up as a member of `fs` instead of the top-level scope, found nothing,
    /// and was colored as an error even though it's an ordinary, valid command.
    #[test]
    fn command_after_namespaced_command_and_pipe_is_not_an_error() {
        let root = scope_with_fs_namespace();
        root.declare("count", Value::Command(MyCmdSignature::create_command()))
            .unwrap();
        let scope = Some(root);

        let classified = classify_tokens("fs:my_cmd|count", &scope).unwrap();
        assert_eq!(classified[0].1, Some("command"), "fs");
        assert_eq!(classified[2].1, Some("command"), "my_cmd");
        assert_eq!(
            classified[4].1,
            Some("command"),
            "count, a plain top-level command, should not be highlighted as an error \
             just because it follows a namespaced command and a pipe"
        );
    }

    /// A name that resolves to nothing at all in scope is still a genuine error.
    #[test]
    fn unresolvable_bare_word_is_still_an_error() {
        let scope = Some(scope_with_fs_namespace());
        let classified = classify_tokens("not_a_real_command_or_namespace", &scope).unwrap();
        assert_eq!(classified[0].1, Some("error"));
    }

    /// A bare word that names a real, `$PATH`-visible entry that is neither executable
    /// nor a directory (a plain data file) can never actually run as a command --
    /// spawning it fails with EACCES/"Permission denied" -- so it should be highlighted
    /// as an error, not optimistically colored as a valid command just because
    /// `resolve_external_command`'s plain `.exists()` check found it on `$PATH`.
    #[test]
    fn non_executable_plain_file_used_as_command_is_an_error() {
        let _lock = PATH_TEST_LOCK.lock().unwrap();
        let dir = scratch_dir("non_executable_plain_file");
        let file = dir.join("just_a_data_file");
        std::fs::write(&file, b"not a program").unwrap();
        let mut perms = std::fs::metadata(&file).unwrap().permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(&file, perms).unwrap();
        let _guard = PathGuard::set(&dir);

        let scope = Some(Scope::create_root());
        let classified = classify_tokens("just_a_data_file", &scope).unwrap();
        assert_eq!(
            classified[0].1,
            Some("error"),
            "a $PATH entry that exists but isn't executable or a directory should be an error"
        );
    }

    /// A bare word naming a real, executable `$PATH` entry is a genuinely valid command
    /// -- this already worked before the fix and must keep working after it.
    #[test]
    fn executable_file_used_as_command_is_still_a_command() {
        let _lock = PATH_TEST_LOCK.lock().unwrap();
        let dir = scratch_dir("executable_file");
        let file = dir.join("a_real_program");
        std::fs::write(&file, b"#!/bin/sh\necho hi\n").unwrap();
        let mut perms = std::fs::metadata(&file).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&file, perms).unwrap();
        let _guard = PathGuard::set(&dir);

        let scope = Some(Scope::create_root());
        let classified = classify_tokens("a_real_program", &scope).unwrap();
        assert_eq!(classified[0].1, Some("command"));
    }

    /// A bare word naming a directory on `$PATH` is also a meaningful command reference
    /// (the same way an explicit `./some_dir` path is a valid `cd` target), so it
    /// shouldn't be flagged as an error just because it isn't itself executable.
    #[test]
    fn directory_used_as_command_is_not_an_error() {
        let _lock = PATH_TEST_LOCK.lock().unwrap();
        let dir = scratch_dir("directory_as_command");
        let sub_dir = dir.join("a_directory");
        std::fs::create_dir_all(&sub_dir).unwrap();
        let _guard = PathGuard::set(&dir);

        let scope = Some(Scope::create_root());
        let classified = classify_tokens("a_directory", &scope).unwrap();
        assert_ne!(
            classified[0].1,
            Some("error"),
            "a directory on $PATH should not be highlighted as an error"
        );
    }

    /// A literal path (`./build.rs`) pointing at a real, non-executable, non-directory
    /// file, used as the command itself, is exactly as unrunnable as a bare `$PATH`
    /// word pointing at the same kind of file -- it fails with EACCES/"Permission
    /// denied" -- so it should be highlighted as an error too, not unconditionally
    /// "file_literal" just because it's a well-formed path to a file that exists.
    #[test]
    fn file_literal_used_as_command_pointing_at_non_executable_file_is_an_error() {
        let dir = scratch_dir("file_literal_non_executable");
        let file = dir.join("build.rs");
        std::fs::write(&file, b"fn main() {}").unwrap();
        let mut perms = std::fs::metadata(&file).unwrap().permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(&file, perms).unwrap();

        let scope = Some(Scope::create_root());
        let code = format!("{}", file.display());
        let classified = classify_tokens(&code, &scope).unwrap();
        assert_eq!(
            classified[0].1,
            Some("error"),
            "a non-executable file literal used as a command should be an error"
        );
    }

    /// A literal path pointing at a directory, used as the command itself, is a valid
    /// `cd` target -- must not regress alongside the fix above.
    #[test]
    fn file_literal_used_as_command_pointing_at_directory_is_not_an_error() {
        let dir = scratch_dir("file_literal_directory");
        let sub_dir = dir.join("some_dir");
        std::fs::create_dir_all(&sub_dir).unwrap();

        let scope = Some(Scope::create_root());
        let code = format!("{}", sub_dir.display());
        let classified = classify_tokens(&code, &scope).unwrap();
        assert_ne!(classified[0].1, Some("error"));
    }

    /// A literal path pointing at a genuinely executable file, used as the command
    /// itself, is still "file_literal" -- the fix must not recolor an already-valid
    /// case as an error or otherwise change how a working command highlights.
    #[test]
    fn file_literal_used_as_command_pointing_at_executable_file_is_still_file_literal() {
        let dir = scratch_dir("file_literal_executable");
        let file = dir.join("a_real_program");
        std::fs::write(&file, b"#!/bin/sh\necho hi\n").unwrap();
        let mut perms = std::fs::metadata(&file).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&file, perms).unwrap();

        let scope = Some(Scope::create_root());
        let code = format!("{}", file.display());
        let classified = classify_tokens(&code, &scope).unwrap();
        assert_eq!(classified[0].1, Some("file_literal"));
    }

    /// A file literal's validity is only checked in command position -- as an
    /// ordinary argument (e.g. `cat ./nonexistent`), it should highlight exactly as it
    /// always has, regardless of whether the file exists or is runnable.
    #[test]
    fn file_literal_used_as_argument_ignores_runnability() {
        let root = Scope::create_root();
        root.declare("my_cmd", Value::Command(MyCmdSignature::create_command()))
            .unwrap();

        let classified = classify_tokens("my_cmd ./this_file_does_not_exist_anywhere", &Some(root)).unwrap();
        assert_eq!(classified[0].1, Some("command"));
        assert_eq!(classified[1].1, Some("file_literal"));
    }

    // --- dynamic_output integration ---

    fn sort_like(_context: CommandContext) -> CrushResult<()> {
        Ok(())
    }

    /// Mirrors `stream:sort`'s own `dynamic_output` closely enough for a test: rejects a
    /// non-stream (here, specifically `Integer`) input outright, and otherwise passes the
    /// input type through unchanged.
    fn sort_like_dynamic_output(
        input: PipeInputType,
        _arguments: &[ValidationArgument],
    ) -> CrushResult<OutputType> {
        match input {
            PipeInputType::Known(ValueType::Integer) => {
                crate::lang::errors::command_error("Expected a stream as input.")
            }
            _ => Ok(OutputType::Passthrough),
        }
    }

    #[signature(sort_like, dynamic_output = sort_like_dynamic_output)]
    struct SortLikeSignature {
        #[unnamed()]
        field: Vec<String>,
    }

    fn takes_args(_context: CommandContext) -> CrushResult<()> {
        Ok(())
    }

    /// Mirrors `csv:from`'s own `dynamic_output` closely enough for a test: rejects a
    /// literal unnamed argument whose value is `"bad"`.
    fn takes_args_dynamic_output(
        _input: PipeInputType,
        arguments: &[ValidationArgument],
    ) -> CrushResult<OutputType> {
        for arg in arguments {
            if arg.argument_type.is_none() {
                if let ArgumentValue::Value(Value::String(s)) = &arg.value {
                    if s.as_ref() == "bad" {
                        return arg.error("bad argument value");
                    }
                }
            }
        }
        Ok(OutputType::Unknown)
    }

    #[signature(takes_args, dynamic_output = takes_args_dynamic_output)]
    struct TakesArgsSignature {
        #[unnamed()]
        field: Vec<String>,
    }

    fn scope_with_dynamic_output_commands() -> Scope {
        let root = Scope::create_root();
        root.declare("sort_like", Value::Command(SortLikeSignature::create_command()))
            .unwrap();
        root.declare("takes_args", Value::Command(TakesArgsSignature::create_command()))
            .unwrap();
        root
    }

    /// The motivating example: a bare literal integer piped into a command whose
    /// `dynamic_output` rejects a non-stream input should highlight that command as an
    /// error -- even though the literal itself was never a `Command` at all.
    #[test]
    fn literal_pipe_input_rejected_by_dynamic_output_is_an_error() {
        let scope = Some(scope_with_dynamic_output_commands());
        let classified = classify_tokens("3|sort_like", &scope).unwrap();
        let sort_like_idx = classified
            .iter()
            .position(|(tok, _)| matches!(tok, Token::String(s, _) if *s == "sort_like"))
            .unwrap();
        assert_eq!(classified[sort_like_idx].1, Some("error"));
    }

    /// The same command with no pipe input at all (nothing's been ruled out yet) must
    /// not be flagged -- only an actually-known, actually-bad input type is an error.
    #[test]
    fn dynamic_output_with_no_pipe_input_is_not_an_error() {
        let scope = Some(scope_with_dynamic_output_commands());
        let classified = classify_tokens("sort_like", &scope).unwrap();
        assert_eq!(classified[0].1, Some("command"));
    }

    /// An error from one stage's `dynamic_output` must not cascade into a false-positive
    /// error on the *next* stage -- the failed stage's output is treated as genuinely
    /// unknown, not propagated as if it had actually produced the rejected type.
    #[test]
    fn error_in_one_stage_does_not_cascade_to_the_next() {
        let scope = Some(scope_with_dynamic_output_commands());
        let classified = classify_tokens("3|sort_like|sort_like", &scope).unwrap();
        let sort_like_positions: Vec<usize> = classified
            .iter()
            .enumerate()
            .filter(|(_, (tok, _))| matches!(tok, Token::String(s, _) if *s == "sort_like"))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(sort_like_positions.len(), 2);
        assert_eq!(classified[sort_like_positions[0]].1, Some("error"));
        assert_ne!(classified[sort_like_positions[1]].1, Some("error"));
    }

    /// A `dynamic_output` function also sees the command's own literal arguments, not
    /// just the pipe's input type -- an argument value it rejects is an error too.
    #[test]
    fn literal_argument_rejected_by_dynamic_output_is_an_error() {
        let scope = Some(scope_with_dynamic_output_commands());
        let classified = classify_tokens("takes_args bad", &scope).unwrap();
        assert_eq!(classified[0].1, Some("error"));
    }

    /// The same command with an argument it accepts must highlight normally.
    #[test]
    fn literal_argument_accepted_by_dynamic_output_is_not_an_error() {
        let scope = Some(scope_with_dynamic_output_commands());
        let classified = classify_tokens("takes_args good", &scope).unwrap();
        assert_eq!(classified[0].1, Some("command"));
    }

    /// Regression test: a single-dash short flag (`sh -c ...`) used to panic
    /// (`name[2..]` assumed every `Token::Flag` had a two-character `--` prefix, but a
    /// short flag has only one dash) -- found via a pty test that types a command
    /// character by character, where highlighting runs on partial input constantly.
    #[test]
    fn single_dash_flag_does_not_panic() {
        let root = Scope::create_root();
        root.declare("my_cmd", Value::Command(MyCmdSignature::create_command()))
            .unwrap();
        let classified = classify_tokens("my_cmd -c", &Some(root)).unwrap();
        assert_eq!(classified.len(), 2);
    }

    /// Regression test, same bug: a lone, still-being-typed `-` (nothing after it yet)
    /// is itself a valid `Token::Flag` of length 1, which is exactly what triggered the
    /// out-of-bounds slice this guards against.
    #[test]
    fn lone_dash_flag_does_not_panic() {
        let root = Scope::create_root();
        root.declare("my_cmd", Value::Command(MyCmdSignature::create_command()))
            .unwrap();
        let classified = classify_tokens("my_cmd -", &Some(root)).unwrap();
        assert_eq!(classified.len(), 2);
    }
}
