use crate::lang::argument::{ArgumentDefinition, ArgumentValue, SwitchStyle, ValidationArgument};
use crate::lang::ast::expand_user;
use crate::lang::ast::lexer::{LanguageMode, Lexer, TokenizerMode};
use crate::lang::ast::source::{Source, SourceType};
use crate::lang::ast::token::Token;
use crate::lang::command::{Command, CrushCommand, OutputType, Parameter, PipeInputType};
use crate::lang::command_invocation::{is_runnable_as_command, resolve_external_command};
use crate::lang::errors::CrushResult;
use crate::lang::help::Help;
use crate::lang::serialization::SerializationState;
use crate::lang::serialization::model::Element;
use crate::lang::state::contexts::{CommandContext as CrushCommandContext, EvalContext};
use crate::lang::state::scope::Scope;
use crate::lang::value::{Value, ValueType};
use crate::util::escape::{unescape, unescape_file, unescape_file_raw};
use crate::util::html_escape;
use std::cmp::min;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
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

/// A minimal `Command` that exists only to carry a `{...}` closure literal's own
/// *predicted* output type (computed by recursively scanning its body as its own job --
/// see `classify_tokens`'s handling of `BlockEnd`) across into an existing
/// `dynamic_output` function that expects a real command value and calls
/// `.output_type()` on it, e.g. `select`'s own named-closure-argument handling. This
/// value only ever appears inside `ArgumentValue::Value(Value::Command(..))` for static
/// analysis, so every other `CrushCommand` method is unreachable -- nothing ever calls
/// `.eval()`/`.serialize()`/etc. on it.
struct PredictedOutputCommand {
    output: OutputType,
}

impl Display for PredictedOutputCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("{...}")
    }
}

impl Help for PredictedOutputCommand {
    fn signature(&self) -> String {
        "{...}".to_string()
    }

    fn short_help(&self) -> String {
        "".to_string()
    }

    fn long_help(&self) -> Option<String> {
        None
    }
}

impl CrushCommand for PredictedOutputCommand {
    fn eval(&self, _context: CrushCommandContext) -> CrushResult<()> {
        unreachable!("PredictedOutputCommand is only ever queried for its output type")
    }

    fn might_block(&self, _arguments: &[ArgumentDefinition], _context: &mut EvalContext) -> bool {
        false
    }

    fn name(&self) -> &str {
        ""
    }

    fn serialize(
        &self,
        _elements: &mut Vec<Element>,
        _state: &mut SerializationState,
    ) -> CrushResult<usize> {
        unreachable!("PredictedOutputCommand is never serialized")
    }

    fn bind_helper(&self, _wrapped: &Command, _this: Value) -> Command {
        unreachable!("PredictedOutputCommand is never bound")
    }

    fn output_type<'a>(&'a self, _input: &'a OutputType) -> Option<&'a ValueType> {
        match &self.output {
            OutputType::Known(t) => Some(t),
            _ => None,
        }
    }

    fn completion_data(&self) -> &[Parameter] {
        &[]
    }

    fn definition(&self) -> Option<String> {
        None
    }
}

/// Which bracket kind a `BracketFrame` was pushed for -- decides how its computed
/// result is used on pop, since a `{...}` closure literal and a `$(...)` substitution
/// behave differently even though both need the exact same save/restore machinery: a
/// substitution's own value *is* whatever its last job produces, but a closure literal's
/// own value is always just a `Command` -- what its body predicts is only relevant if
/// something later calls it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BracketKind {
    Sub,
    Block,
}

/// Saved outer state for one `{...}`/`$(...)` nesting level, pushed on `BlockStart`/
/// `SubStart` and popped on the matching `BlockEnd`/`SubEnd`. `classify_tokens` tracks
/// the command currently being scanned with one flat set of variables; entering a
/// bracket starts a genuinely fresh, independent job (its contents are scanned exactly
/// like a top-level script), so that flat state would otherwise be clobbered by whatever
/// the bracket's own contents resolve to. Saving it here and restoring it on the way out
/// is what lets type information flow across the boundary -- into `current_args` for a
/// bracket used as one argument's value, or into `literal_job_output` for a bracket used
/// as a job's own head -- without losing track of the *outer* job at all.
struct BracketFrame {
    kind: BracketKind,
    /// Whether this bracket is one argument of an already-identified outer command
    /// (so its result becomes a new entry in the restored `current_args`), as opposed to
    /// a job head in its own right (a bare statement, or a pipeline's first stage).
    is_argument: bool,
    /// The name this argument was given under, if any -- only meaningful when
    /// `is_argument` is true.
    push_name: Option<String>,
    saved_command_context: CommandContext,
    saved_current_command: Option<Command>,
    saved_current_reference_exists: bool,
    saved_current_command_result_idx: Option<usize>,
    saved_current_args: Vec<ValidationArgument>,
    saved_pipe_input: PipeInputType,
    saved_literal_job_output: Option<ValueType>,
    saved_latest_named_argument_info: Option<(usize, String)>,
}

/// Every token of `code`, in order, paired with the presentation category
/// (matching a key of the `$crush.highlight` color map, or a `tok-<name>`
/// CSS class for HTML output) that applies to it, if any -- plus whether the lexer
/// itself failed partway through (e.g. `ls <`, where `<` isn't a valid token start in
/// command mode). The lexer can't recover from an error and resume, so there's no way to
/// classify anything from that point onward; everything up to it is still classified
/// normally, and the caller is expected to render the remainder of `code` (from the end
/// of the last successful token, i.e. where rendering `classified` would otherwise leave
/// off) in the "error" color/class itself, rather than discarding all of `classified` and
/// falling back to no highlighting at all for the entire line. Shared by both output
/// modes below so the (fairly involved) command/argument-tracking state machine only
/// needs to live in one place.
fn classify_tokens<'a>(
    code: &'a str,
    scope: &Option<Scope>,
) -> (Vec<(Token<'a>, Option<&'static str>)>, bool) {
    let mut new_command = true;
    let mut prev = None;

    let mut tokens: Vec<Token<'a>> = Vec::new();
    let mut had_lexer_error = false;
    for item in Lexer::new(code, LanguageMode::Command, TokenizerMode::IncludeComments) {
        match item {
            Ok((_, tok, _)) => tokens.push(tok),
            Err(_) => {
                had_lexer_error = true;
                break;
            }
        }
    }

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
    // Reset at every job boundary (`finalize_command`'s call sites below) -- including
    // entering a `{...}`/`$(...)`, whose contents are scanned as their own fresh,
    // independent job; `bracket_stack` below is what makes that a real save/restore
    // instead of simply losing track of the outer job.
    let mut pipe_input = PipeInputType::None;
    let mut current_args: Vec<ValidationArgument> = Vec::new();
    let mut current_command_result_idx: Option<usize> = None;
    // A job doesn't have to start with a command invocation -- `3 | sort` pipes the
    // bare literal `3` into `sort` -- so this tracks a literal job head's own type,
    // separately from `current_command`, for `finalize_command` to fall back on.
    let mut literal_job_output: Option<ValueType> = None;
    // One frame per currently-open `{...}`/`$(...)`, saving the outer job's state so it
    // can be restored (with the bracket's own resolved type folded in, as a new argument
    // or as the outer job's own `literal_job_output`) once the matching close is seen --
    // see `BracketFrame`'s own doc comment.
    let mut bracket_stack: Vec<BracketFrame> = Vec::new();
    let argument_source = Source::new(SourceType::Input, Arc::from(code));

    let mut result: Vec<(Token<'a>, Option<&'static str>)> = Vec::with_capacity(tokens.len());

    for idx in 0..tokens.len() {
        let tok = tokens[idx];
        let ntok = tokens.get(idx + 1);

        new_command = match (new_command, prev, tok, ntok) {
            // A `;`/newline or a pipe always opens a fresh job. Also resets
            // `command_context` back to the root scope: a `:` earlier in the previous
            // command (e.g. `$host.procs`) narrows it into that namespace so a chained
            // path like `$io.base64.to` resolves each segment correctly, but a fresh
            // job's own command name must be looked up from the top level again, not as
            // a member of whatever namespace the last command happened to live in.
            // Without this, `$host.procs|count` colored `count` as an error -- it was
            // looked up as a (nonexistent) member of `host`, not as the ordinary,
            // top-level command it actually is.
            (_, _, Token::Separator(_, _) | Token::Pipe(_), _) => {
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
                // one; a fresh statement starts a job with nothing piped in at all.
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
            // `{...}` and `$(...)` always open a fresh job too -- the token right after
            // either is a command start, not a continuation of whatever came before the
            // bracket. Without this, something like `$(files --recurse /)` colored
            // `files` as a plain string, since the state machine only reset on
            // Separator/Pipe.
            //
            // Unlike Separator/Pipe above, a bracket's own *close* still has to come
            // (its contents are a real job, finalized and folded back in on
            // `BlockEnd`/`SubEnd` below) -- so this doesn't reset the outer job's state
            // in place; it saves it onto `bracket_stack` first, exactly so it can be
            // restored intact once the bracket closes.
            (_, _, Token::BlockStart(_) | Token::SubStart(_), _) => {
                let is_argument = current_command_result_idx.is_some();
                let push_name = if is_argument {
                    latest_named_argument_info
                        .as_ref()
                        .filter(|(name_idx, _)| idx == name_idx + 2)
                        .map(|(_, name)| name.clone())
                } else {
                    None
                };
                let fresh_context = match scope {
                    None => CommandContext::Unknown,
                    Some(s) => CommandContext::Known(Value::Scope(s.clone())),
                };
                bracket_stack.push(BracketFrame {
                    kind: match tok {
                        Token::SubStart(_) => BracketKind::Sub,
                        _ => BracketKind::Block,
                    },
                    is_argument,
                    push_name,
                    saved_command_context: std::mem::replace(&mut command_context, fresh_context),
                    saved_current_command: current_command.take(),
                    saved_current_reference_exists: current_reference_exists,
                    saved_current_command_result_idx: current_command_result_idx.take(),
                    saved_current_args: std::mem::take(&mut current_args),
                    saved_pipe_input: std::mem::replace(&mut pipe_input, PipeInputType::None),
                    saved_literal_job_output: literal_job_output.take(),
                    saved_latest_named_argument_info: latest_named_argument_info.take(),
                });
                current_reference_exists = false;
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

        // The matching close of a `{...}`/`$(...)` pushed by the arm above: finalize
        // whatever job its contents turned out to be, restore the outer job's own state
        // exactly as it was before the bracket opened, and fold the bracket's resolved
        // type into that restored state -- as a new argument if it was one argument's
        // value, or as `literal_job_output` if it was a job head in its own right.
        // `bracket_stack.pop()` returning `None` (a stray close with no matching open --
        // only possible on malformed input, e.g. mid-keystroke in an interactive
        // session) is simply ignored rather than treated as an error.
        if matches!(tok, Token::BlockEnd(_) | Token::SubEnd(_)) {
            if let Some(frame) = bracket_stack.pop() {
                let (next_input, error_category) = finalize_command(
                    &current_command,
                    &current_args,
                    &pipe_input,
                    &literal_job_output,
                );
                if let (Some(cat), Some(i)) = (error_category, current_command_result_idx) {
                    result[i].1 = Some(cat);
                }

                command_context = frame.saved_command_context;
                current_command = frame.saved_current_command;
                current_reference_exists = frame.saved_current_reference_exists;
                current_command_result_idx = frame.saved_current_command_result_idx;
                current_args = frame.saved_current_args;
                pipe_input = frame.saved_pipe_input;
                literal_job_output = frame.saved_literal_job_output;
                latest_named_argument_info = frame.saved_latest_named_argument_info;

                match (frame.kind, frame.is_argument) {
                    (BracketKind::Sub, true) => {
                        // A substitution's own value *is* whatever its last job
                        // produced.
                        let value = match next_input {
                            PipeInputType::Known(t) => ArgumentValue::Type(t),
                            PipeInputType::None | PipeInputType::Unknown => {
                                ArgumentValue::Unknown
                            }
                        };
                        current_args.push(ValidationArgument {
                            argument_type: frame.push_name,
                            switch_style: SwitchStyle::None,
                            value,
                            source: argument_source.clone(),
                        });
                    }
                    (BracketKind::Sub, false) => {
                        literal_job_output = match next_input {
                            PipeInputType::Known(t) => Some(t),
                            PipeInputType::None | PipeInputType::Unknown => None,
                        };
                    }
                    (BracketKind::Block, true) => {
                        // A closure literal's own value is always a `Command`; what its
                        // body predicts only matters to something that goes on to call
                        // it, like `select`'s own named-closure-argument handling --
                        // hence the `PredictedOutputCommand` wrapper, instead of
                        // `ArgumentValue::Type` (which would wrongly claim the
                        // argument's own value already *is* that type).
                        let predicted = Arc::new(PredictedOutputCommand {
                            output: match next_input {
                                PipeInputType::Known(t) => OutputType::Known(t),
                                PipeInputType::None | PipeInputType::Unknown => {
                                    OutputType::Unknown
                                }
                            },
                        });
                        current_args.push(ValidationArgument {
                            argument_type: frame.push_name,
                            switch_style: SwitchStyle::None,
                            value: ArgumentValue::Value(Value::Command(predicted)),
                            source: argument_source.clone(),
                        });
                    }
                    (BracketKind::Block, false) => {
                        literal_job_output = Some(ValueType::Command);
                    }
                }
            }
        }

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

        let inside_closure = bracket_stack.iter().any(|f| f.kind == BracketKind::Block);
        let category = get_category(
            tok,
            ntok,
            new_command,
            scope,
            &current_command,
            current_reference_exists,
            &expected_argument_type,
            inside_closure,
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

    (result, had_lexer_error)
}

pub fn syntax_highlight(
    code: &str,
    colors: &HashMap<String, String>,
    scope: &Option<Scope>,
) -> CrushResult<String> {
    let (classified, had_lexer_error) = classify_tokens(code, scope);
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

    // The lexer can't recover and resume after a failure, so there's nothing left to
    // classify token-by-token from here on -- but the failure itself is exactly the
    // thing a user most needs to see highlighted, instead of the rest of the line
    // silently reverting to no color at all.
    if had_lexer_error && pos < code.len() {
        match colors.get("error") {
            Some(color) if !color.is_empty() => {
                res.push_str(color);
                res.push_str(&code[pos..]);
                res.push_str("\x1b[0m");
            }
            _ => res.push_str(&code[pos..]),
        }
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
    let (classified, had_lexer_error) = classify_tokens(code, &None);
    let mut res = String::new();
    let mut pos = 0;
    let mut i = 0;

    while i < classified.len() {
        let (tok, category) = classified[i];
        res.push_str(&html_escape(
            &code[pos..min(tok.location().start, code.len())],
        ));

        if category == Some("command") {
            // A multi-segment command path ($dns.query_reverse, $io.base64.to)
            // lexes as separate String/MemberOperator/String/... tokens --
            // there's nothing at the lexer level distinguishing it from a
            // $value.method access. Stitch a whole chain of them back into
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

    if had_lexer_error && pos < code.len() {
        let escaped = html_escape(&code[pos..]);
        res.push_str(&format!("<span class=\"tok-error\">{escaped}</span>"));
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
    inside_closure: bool,
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
                    expand_user(name)
                        .ok()
                        .and_then(|s| unescape_file_raw(&s).ok())
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
        | TypeAnnotationColon(_)
        | ExprModeStart(_)
        | SubStart(_)
        | BlockEnd(_)
        | BlockStart(_) => Some("operator"),
        // In command position, a `$identifier` immediately followed by a
        // `MemberOperator` is the root of a namespaced-command chain (`$struct.of`,
        // `$fs.watch`) -- exactly the role a bareword root plays for `String` above,
        // just sigilled. Color it as part of that chain rather than as a plain
        // variable reference, regardless of whether the name happens to also resolve
        // as an ordinary value in scope.
        Identifier(_, _) if new_command && matches!(next_token_type, Some(MemberOperator(_))) => {
            Some("command")
        }
        Identifier(name, _) => match scope {
            None => Some("identifier"),
            Some(s) => match (s.get(&name[1..]).unwrap_or(None), next_token_type) {
                (Some(_), Some(Declare(_))) => Some("error"),
                (Some(_), _) => Some("identifier"),
                (None, Some(Declare(_))) => Some("identifier"),
                // A closure body can be called with arbitrary named arguments its
                // caller decides on -- a row's own columns, for `select`/`where`/`each`
                // (`$size`, `$file`, ...), an accumulator for `fold` (`$acc`), and so
                // on -- none of which are, or ever could be, visible in `scope` itself.
                // Flagging an unresolved identifier as a real error only makes sense
                // when we actually know the full set of names that could ever bind it;
                // inside a closure, we don't, so this stays neutral instead of guessing
                // wrong. Outside a closure, an unresolved name is still a real error --
                // this is the one case this highlighter can be confident about.
                (None, _) if inside_closure => Some("identifier"),
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
        Token::TypeAnnotationColon(_) => None,
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
    /// the real, builtin `fs` namespace (`$fs.files`, `$fs.mounts`, ...) closely enough to
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
        let classified = classify_tokens("fs", &scope).0;
        assert_eq!(classified.len(), 1);
        assert_ne!(
            classified[0].1,
            Some("error"),
            "a bare namespace name in command position should not be highlighted as an error"
        );
    }

    /// Same namespace, referenced the ordinary way (`$fs.my_cmd`) -- this already worked
    /// before the fix and must keep working after it.
    #[test]
    fn namespace_member_access_is_still_a_command() {
        let scope = Some(scope_with_fs_namespace());
        let classified = classify_tokens("$fs.my_cmd", &scope).0;
        assert_eq!(classified[0].1, Some("command"));
        assert_eq!(classified[2].1, Some("command"));
    }

    /// A plain, top-level command piped after a namespaced one (`$fs.my_cmd|count`, e.g.
    /// the reported `$host.procs|count`) used to be highlighted as an error. Resolving
    /// `my_cmd` as a member of `fs` narrows the lookup context used for the *next*
    /// command-position name into the `fs` namespace too (so a chain like
    /// `$io.base64.to` resolves each segment correctly) -- but nothing ever reset that
    /// context back to the root scope for the job that starts after the `|`, so `count`
    /// was looked up as a member of `fs` instead of the top-level scope, found nothing,
    /// and was colored as an error even though it's an ordinary, valid command.
    #[test]
    fn command_after_namespaced_command_and_pipe_is_not_an_error() {
        let root = scope_with_fs_namespace();
        root.declare("count", Value::Command(MyCmdSignature::create_command()))
            .unwrap();
        let scope = Some(root);

        let classified = classify_tokens("$fs.my_cmd|count", &scope).0;
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
        let classified = classify_tokens("not_a_real_command_or_namespace", &scope).0;
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
        let classified = classify_tokens("just_a_data_file", &scope).0;
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
        let classified = classify_tokens("a_real_program", &scope).0;
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
        let classified = classify_tokens("a_directory", &scope).0;
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
        let classified = classify_tokens(&code, &scope).0;
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
        let classified = classify_tokens(&code, &scope).0;
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
        let classified = classify_tokens(&code, &scope).0;
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

        let classified = classify_tokens("my_cmd ./this_file_does_not_exist_anywhere", &Some(root)).0;
        assert_eq!(classified[0].1, Some("command"));
        assert_eq!(classified[1].1, Some("file_literal"));
    }

    // --- dynamic_output integration ---

    fn sort_like(_context: CommandContext) -> CrushResult<()> {
        Ok(())
    }

    /// Mirrors `$stream.sort`'s own `dynamic_output` closely enough for a test: rejects a
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

    /// Mirrors `$csv.from`'s own `dynamic_output` closely enough for a test: rejects a
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

    // --- {}/$() boundary-crossing fixtures ---

    fn produces_integer(_context: CommandContext) -> CrushResult<()> {
        Ok(())
    }

    #[signature(produces_integer, output = OutputType::Known(ValueType::Integer))]
    struct ProducesIntegerSignature {}

    fn produces_string(_context: CommandContext) -> CrushResult<()> {
        Ok(())
    }

    #[signature(produces_string, output = OutputType::Known(ValueType::String))]
    struct ProducesStringSignature {}

    fn checks_arg_type(_context: CommandContext) -> CrushResult<()> {
        Ok(())
    }

    /// Rejects its `x` argument unless its (resolved, not necessarily literal) type is
    /// `Integer` -- used to prove that a `$(...)` substitution's own computed type
    /// reaches here as `ArgumentValue::Type`, not just `Unknown`.
    fn checks_arg_type_dynamic_output(
        _input: PipeInputType,
        arguments: &[ValidationArgument],
    ) -> CrushResult<OutputType> {
        for arg in arguments {
            if arg.argument_type.as_deref() == Some("x") {
                return match &arg.value {
                    ArgumentValue::Type(ValueType::Integer) => {
                        Ok(OutputType::Known(ValueType::Integer))
                    }
                    ArgumentValue::Type(_) => {
                        crate::lang::errors::command_error("expected an integer")
                    }
                    _ => Ok(OutputType::Unknown),
                };
            }
        }
        Ok(OutputType::Unknown)
    }

    #[signature(checks_arg_type, dynamic_output = checks_arg_type_dynamic_output)]
    struct ChecksArgTypeSignature {
        x: Option<Value>,
    }

    fn closure_arg_like(_context: CommandContext) -> CrushResult<()> {
        Ok(())
    }

    /// Mirrors `select`'s own named-closure-argument handling: calls `.output_type()`
    /// on its `x` argument's `Command` value, rejecting anything that doesn't predict
    /// `Integer` -- used to prove that a `{...}` closure literal's own predicted return
    /// type (computed recursively from its body) reaches here as a real `Command`, not
    /// just `Unknown`.
    fn closure_arg_like_dynamic_output(
        _input: PipeInputType,
        arguments: &[ValidationArgument],
    ) -> CrushResult<OutputType> {
        for arg in arguments {
            if arg.argument_type.as_deref() == Some("x") {
                if let ArgumentValue::Value(Value::Command(cmd)) = &arg.value {
                    return match cmd.output_type(&OutputType::Unknown) {
                        Some(ValueType::Integer) => Ok(OutputType::Known(ValueType::Integer)),
                        Some(_) => crate::lang::errors::command_error(
                            "expected a closure returning an integer",
                        ),
                        None => Ok(OutputType::Unknown),
                    };
                }
            }
        }
        Ok(OutputType::Unknown)
    }

    #[signature(closure_arg_like, dynamic_output = closure_arg_like_dynamic_output)]
    struct ClosureArgLikeSignature {
        x: Option<Command>,
    }

    fn two_args_like(_context: CommandContext) -> CrushResult<()> {
        Ok(())
    }

    /// Only accepts being called with *both* `x` and `y` given -- used to prove that an
    /// argument following a `{...}`/`$(...)` bracket is still collected into the same
    /// command's argument list, i.e. that entering/leaving the bracket didn't corrupt or
    /// lose track of the outer command at all.
    fn two_args_like_dynamic_output(
        _input: PipeInputType,
        arguments: &[ValidationArgument],
    ) -> CrushResult<OutputType> {
        let mut saw_x = false;
        let mut saw_y = false;
        for arg in arguments {
            match arg.argument_type.as_deref() {
                Some("x") => saw_x = true,
                Some("y") => saw_y = true,
                _ => {}
            }
        }
        if saw_x && saw_y {
            Ok(OutputType::Known(ValueType::Bool))
        } else {
            crate::lang::errors::command_error("expected both x and y")
        }
    }

    #[signature(two_args_like, dynamic_output = two_args_like_dynamic_output)]
    struct TwoArgsLikeSignature {
        x: Option<Value>,
        y: Option<Value>,
    }

    fn scope_with_dynamic_output_commands() -> Scope {
        let root = Scope::create_root();
        root.declare("sort_like", Value::Command(SortLikeSignature::create_command()))
            .unwrap();
        root.declare("takes_args", Value::Command(TakesArgsSignature::create_command()))
            .unwrap();
        root.declare(
            "produces_integer",
            Value::Command(ProducesIntegerSignature::create_command()),
        )
        .unwrap();
        root.declare(
            "produces_string",
            Value::Command(ProducesStringSignature::create_command()),
        )
        .unwrap();
        root.declare(
            "checks_arg_type",
            Value::Command(ChecksArgTypeSignature::create_command()),
        )
        .unwrap();
        root.declare(
            "closure_arg_like",
            Value::Command(ClosureArgLikeSignature::create_command()),
        )
        .unwrap();
        root.declare(
            "two_args_like",
            Value::Command(TwoArgsLikeSignature::create_command()),
        )
        .unwrap();
        root
    }

    /// The motivating example: a bare literal integer piped into a command whose
    /// `dynamic_output` rejects a non-stream input should highlight that command as an
    /// error -- even though the literal itself was never a `Command` at all.
    #[test]
    fn literal_pipe_input_rejected_by_dynamic_output_is_an_error() {
        let scope = Some(scope_with_dynamic_output_commands());
        let classified = classify_tokens("3|sort_like", &scope).0;
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
        let classified = classify_tokens("sort_like", &scope).0;
        assert_eq!(classified[0].1, Some("command"));
    }

    /// An error from one stage's `dynamic_output` must not cascade into a false-positive
    /// error on the *next* stage -- the failed stage's output is treated as genuinely
    /// unknown, not propagated as if it had actually produced the rejected type.
    #[test]
    fn error_in_one_stage_does_not_cascade_to_the_next() {
        let scope = Some(scope_with_dynamic_output_commands());
        let classified = classify_tokens("3|sort_like|sort_like", &scope).0;
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
        let classified = classify_tokens("takes_args bad", &scope).0;
        assert_eq!(classified[0].1, Some("error"));
    }

    /// The same command with an argument it accepts must highlight normally.
    #[test]
    fn literal_argument_accepted_by_dynamic_output_is_not_an_error() {
        let scope = Some(scope_with_dynamic_output_commands());
        let classified = classify_tokens("takes_args good", &scope).0;
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
        let classified = classify_tokens("my_cmd -c", &Some(root)).0;
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
        let classified = classify_tokens("my_cmd -", &Some(root)).0;
        assert_eq!(classified.len(), 2);
    }

    // --- {}/$() boundary-crossing ---

    fn find_command_token<'a>(
        classified: &'a [(Token<'a>, Option<&'static str>)],
        name: &str,
    ) -> (usize, Option<&'static str>) {
        classified
            .iter()
            .enumerate()
            .find(|(_, (tok, _))| matches!(tok, Token::String(s, _) if *s == name))
            .map(|(i, (_, cat))| (i, *cat))
            .unwrap_or_else(|| panic!("no `{}` token found in {:?}", name, classified))
    }

    /// A substitution used as a *named argument's* value should give the outer
    /// command's `dynamic_output` the substitution's own resolved type, not nothing.
    #[test]
    fn substitution_argument_type_crosses_the_boundary() {
        let scope = Some(scope_with_dynamic_output_commands());
        let classified = classify_tokens("checks_arg_type x=$(produces_integer)", &scope).0;
        let (_, cat) = find_command_token(&classified, "checks_arg_type");
        assert_ne!(cat, Some("error"), "an integer substitution should be accepted");
    }

    /// Same, but the substitution's type is the wrong one -- the outer command must
    /// actually see *that* type (not just "something"), and reject it.
    #[test]
    fn substitution_argument_of_the_wrong_type_is_rejected() {
        let scope = Some(scope_with_dynamic_output_commands());
        let classified = classify_tokens("checks_arg_type x=$(produces_string)", &scope).0;
        let (_, cat) = find_command_token(&classified, "checks_arg_type");
        assert_eq!(cat, Some("error"), "a string substitution should be rejected");
    }

    /// A substitution used as a whole pipeline stage's own job head must feed its
    /// resolved type into the *next* stage across the pipe, exactly like a bare literal
    /// already does -- the exact motivating `3|sort` case, just with `3` itself computed
    /// from a nested job instead of typed directly.
    #[test]
    fn substitution_as_job_head_feeds_the_next_pipe_stage() {
        let scope = Some(scope_with_dynamic_output_commands());
        let classified = classify_tokens("$(produces_integer)|sort_like", &scope).0;
        let (_, cat) = find_command_token(&classified, "sort_like");
        assert_eq!(
            cat,
            Some("error"),
            "sort_like should see Known(Integer) through the substitution and reject it"
        );
    }

    /// A `{...}` closure literal used as a named argument should give the outer
    /// command's `dynamic_output` a real `Command` whose `.output_type()` reports the
    /// closure body's own predicted type -- not just an opaque, uninspectable value.
    #[test]
    fn closure_argument_predicted_type_crosses_the_boundary() {
        let scope = Some(scope_with_dynamic_output_commands());
        let classified = classify_tokens("closure_arg_like x={produces_integer}", &scope).0;
        let (_, cat) = find_command_token(&classified, "closure_arg_like");
        assert_ne!(
            cat,
            Some("error"),
            "a closure predicted to return an integer should be accepted"
        );
    }

    /// Same, but the closure's own body predicts the wrong type.
    #[test]
    fn closure_argument_of_the_wrong_predicted_type_is_rejected() {
        let scope = Some(scope_with_dynamic_output_commands());
        let classified = classify_tokens("closure_arg_like x={produces_string}", &scope).0;
        let (_, cat) = find_command_token(&classified, "closure_arg_like");
        assert_eq!(
            cat,
            Some("error"),
            "a closure predicted to return a string should be rejected"
        );
    }

    /// An error inside a bracket's own contents must still recolor *that* inner
    /// command's token red, exactly as it would at the top level -- nesting inside an
    /// argument doesn't exempt it from the same check.
    #[test]
    fn error_inside_a_bracket_still_marks_the_inner_command() {
        let scope = Some(scope_with_dynamic_output_commands());
        let classified = classify_tokens("checks_arg_type x=$(3|sort_like)", &scope).0;
        let (_, cat) = find_command_token(&classified, "sort_like");
        assert_eq!(cat, Some("error"), "sort_like given Known(Integer) should still be flagged, even nested inside a substitution");
    }

    /// An argument that follows a bracket must still be collected into the *same*
    /// outer command's argument list -- proving the save/restore around the bracket
    /// didn't lose track of (or corrupt) the outer command at all.
    #[test]
    fn argument_after_a_bracket_is_still_collected() {
        let scope = Some(scope_with_dynamic_output_commands());
        let classified =
            classify_tokens("two_args_like x=$(produces_integer) y=5", &scope).0;
        let (_, cat) = find_command_token(&classified, "two_args_like");
        assert_ne!(cat, Some("error"), "both x and y were given, just with x's value nested in a substitution");
    }

    /// Sanity check for the test above: with only `x` given (no `y`), the same command
    /// really is rejected -- confirming the dynamic_output function does real work and
    /// the previous test wasn't vacuously passing.
    #[test]
    fn missing_argument_after_a_bracket_is_still_rejected() {
        let scope = Some(scope_with_dynamic_output_commands());
        let classified = classify_tokens("two_args_like x=$(produces_integer)", &scope).0;
        let (_, cat) = find_command_token(&classified, "two_args_like");
        assert_eq!(cat, Some("error"), "y was never given");
    }

    /// A bare `{...}` used as a job's own head (not an argument) is a closure value in
    /// its own right -- its type is `Command`, regardless of what its body predicts.
    #[test]
    fn bare_closure_as_job_head_is_command_typed() {
        let scope = Some(scope_with_dynamic_output_commands());
        // sort_like only rejects a *Known(Integer)* input; a bare closure statement's
        // own type (Command) should pass through untouched -- same as any other
        // non-Integer input.
        let classified = classify_tokens("{produces_integer}|sort_like", &scope).0;
        let (_, cat) = find_command_token(&classified, "sort_like");
        assert_ne!(
            cat,
            Some("error"),
            "a closure literal's own type is Command, not whatever it predicts"
        );
    }

    /// Regression guard: malformed/partial bracket input -- exactly what a live,
    /// character-by-character interactive session constantly produces mid-keystroke --
    /// must never panic, even though it's never valid syntax. Some of these are rejected
    /// by the lexer itself (an unclosed bracket, or a stray close with no matching open
    /// at all, are both genuine lexical errors -- `Err` is an expected, ordinary outcome
    /// for those); what this guards is the handful that *do* lex successfully but still
    /// exercise `bracket_stack` unusually (an extra stray close *after* a balanced
    /// pair, an empty bracket, ...), where only a panic would be a bug.
    #[test]
    fn malformed_bracket_input_does_not_panic() {
        let scope = Some(scope_with_dynamic_output_commands());
        for code in [
            "checks_arg_type x=$(",
            "checks_arg_type x=$(produces_integer",
            "closure_arg_like x={",
            "closure_arg_like x={produces_integer",
            ")",
            "}",
            "$(",
            "{",
            "checks_arg_type x=$(produces_integer))",
            "closure_arg_like x={produces_integer}}",
            "$()",
            "{}",
            "two_args_like x=$(produces_integer) y=$(",
        ] {
            // Only a panic is a failure here -- a lexical Err is a fine, expected
            // outcome for several of these.
            let _ = classify_tokens(code, &scope);
        }
    }

    /// Deep nesting (a substitution inside a closure inside a substitution, ...) must
    /// not panic or lose track -- `bracket_stack` has to actually behave like a stack
    /// (LIFO) rather than, say, a single saved frame that a second nesting level would
    /// silently clobber.
    #[test]
    fn deeply_nested_brackets_resolve_correctly() {
        let scope = Some(scope_with_dynamic_output_commands());
        let classified = classify_tokens(
            "checks_arg_type x=$(closure_arg_like x={$(produces_integer)})",
            &scope,
        )
        .0;
        let (_, cat) = find_command_token(&classified, "checks_arg_type");
        // closure_arg_like's own x is a closure {$(produces_integer)}, whose predicted
        // return type is Integer (from the innermost substitution) -- so closure_arg_like
        // itself resolves to Known(Integer), and checks_arg_type's own x (the whole
        // $(closure_arg_like ...) substitution) sees that same Integer type.
        assert_ne!(cat, Some("error"));
        let (_, inner_cat) = find_command_token(&classified, "closure_arg_like");
        assert_ne!(inner_cat, Some("error"));
    }

    fn find_identifier_token<'a>(
        classified: &'a [(Token<'a>, Option<&'static str>)],
        name: &str,
    ) -> Option<&'static str> {
        classified
            .iter()
            .find(|(tok, _)| matches!(tok, Token::Identifier(s, _) if *s == name))
            .map(|(_, cat)| *cat)
            .unwrap_or_else(|| panic!("no `{}` token found in {:?}", name, classified))
    }

    /// The bug report this guards: `select foo={$size}` (or any closure-argument
    /// command) must not flag `$size` as an unknown variable just because it isn't a
    /// real, lexically-scoped variable -- a closure passed as an argument can be called
    /// with arbitrary named arguments its caller decides on (a row's own columns, for
    /// `select`/`where`/`each`), which this highlighter has no way to know ahead of time.
    #[test]
    fn unresolved_identifier_inside_a_closure_argument_is_not_an_error() {
        let scope = Some(scope_with_dynamic_output_commands());
        let classified =
            classify_tokens("closure_arg_like x={$size}", &scope).0;
        let cat = find_identifier_token(&classified, "$size");
        assert_ne!(cat, Some("error"));
    }

    /// The same unresolved name at the top level, outside any closure, is still a real
    /// error -- the leniency above is specifically scoped to closure bodies, not a
    /// blanket "stop checking variables" regression.
    #[test]
    fn unresolved_identifier_at_top_level_is_still_an_error() {
        let scope = Some(scope_with_dynamic_output_commands());
        let classified = classify_tokens("$size", &scope).0;
        let cat = find_identifier_token(&classified, "$size");
        assert_eq!(cat, Some("error"));
    }

    /// A substitution is not a closure -- `$(...)` runs in the same lexical scope as the
    /// code around it, so an unresolved identifier inside one is still a real error.
    #[test]
    fn unresolved_identifier_inside_a_substitution_is_still_an_error() {
        let scope = Some(scope_with_dynamic_output_commands());
        let classified = classify_tokens("checks_arg_type x=$($size)", &scope).0;
        let cat = find_identifier_token(&classified, "$size");
        assert_eq!(cat, Some("error"));
    }

    // --- lexer failures ---

    /// classify_tokens itself must report that the lexer failed, alongside whatever it
    /// did manage to classify beforehand -- not discard that prefix's classification
    /// (`ls` is still a perfectly good, resolvable command) just because `<` can't be
    /// lexed in command mode.
    #[test]
    fn classify_tokens_reports_lexer_failure_without_discarding_the_prefix() {
        let scope = Some(scope_with_dynamic_output_commands());
        let (classified, had_lexer_error) = classify_tokens("sort_like <", &scope);
        assert!(had_lexer_error);
        let (_, cat) = find_command_token(&classified, "sort_like");
        assert_ne!(
            cat,
            Some("error"),
            "the successfully-lexed prefix should still be classified normally"
        );
    }

    /// A line that lexes cleanly end to end must report no lexer failure at all.
    #[test]
    fn classify_tokens_reports_no_lexer_failure_for_valid_input() {
        let scope = Some(scope_with_dynamic_output_commands());
        let (_, had_lexer_error) = classify_tokens("sort_like file", &scope);
        assert!(!had_lexer_error);
    }

    /// The motivating example: `ls <` must color the unlexable remainder (`<`) as an
    /// error, with the valid prefix (`ls`) left exactly as it would otherwise be
    /// colored -- not fall back to no highlighting at all for the entire line, which is
    /// what propagating classify_tokens's old Err all the way up to the rustyline
    /// Highlighter (see highlight_internal in rustyline_helper.rs) used to do.
    #[test]
    fn syntax_highlight_unlexable_remainder_is_colored_and_prefix_is_preserved() {
        let scope = Some(scope_with_dynamic_output_commands());
        let mut colors = HashMap::new();
        colors.insert("error".to_string(), "<<ERR>>".to_string());
        colors.insert("command".to_string(), "<<CMD>>".to_string());

        let rendered = syntax_highlight("sort_like <", &colors, &scope).unwrap();

        assert!(rendered.contains("<<CMD>>sort_like"), "got: {}", rendered);
        // The error region starts right after the valid prefix and still contains the
        // actual unlexable character -- whether or not the gap's own whitespace is
        // included in the colored span doesn't matter for how it looks rendered.
        let (_, after_prefix) = rendered.split_once("<<CMD>>sort_like").unwrap();
        assert!(after_prefix.contains("<<ERR>>"), "got: {}", rendered);
        let (_, after_error) = after_prefix.split_once("<<ERR>>").unwrap();
        assert!(after_error.contains('<'), "got: {}", rendered);
    }

    /// A line that's entirely unlexable from the very first character (nothing
    /// successfully classified at all) must still color the whole thing as an error,
    /// not panic or silently produce no output.
    #[test]
    fn syntax_highlight_wholly_unlexable_input_is_colored_as_an_error() {
        let scope = Some(scope_with_dynamic_output_commands());
        let mut colors = HashMap::new();
        colors.insert("error".to_string(), "<<ERR>>".to_string());

        let rendered = syntax_highlight("<", &colors, &scope).unwrap();
        assert_eq!(rendered, "<<ERR>><\x1b[0m");
    }

    /// HTML rendering gets the same treatment: the unlexable remainder is wrapped in
    /// tok-error, not silently dropped.
    #[test]
    fn syntax_highlight_html_colors_the_unlexable_remainder_as_an_error() {
        let rendered = syntax_highlight_html("ls <").unwrap();
        assert!(rendered.contains("<span class=\"tok-error\">"), "got: {}", rendered);
    }
}
