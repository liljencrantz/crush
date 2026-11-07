use crate::lang::ast::expand_user;
use crate::lang::ast::node::TextLiteralStyle;
use crate::lang::ast::node::TextLiteralStyle::Unquoted;
use crate::lang::command::Parameter;
use crate::lang::command_invocation::is_runnable_as_command;
use crate::lang::completion::parse::{
    CompletionCommand, LastArgument, ParseResult, PartialCommandResult, parse,
};
use crate::lang::errors::CrushResult;
use crate::lang::parser::Parser;
/**
  Main entry point for tab completion code
*/
use crate::lang::state::scope::Scope;
use crate::lang::value::{Value, ValueType};
use crate::util::directory_lister::DirectoryLister;
use crate::util::escape::escape_without_quotes;
use crate::util::file::cwd;
use crate::util::glob::Glob;
use nix::NixPath;
use std::collections::HashSet;
use std::ops::Deref;
use std::path::PathBuf;

pub mod parse;

pub struct Completion {
    completion: String,
    display: String,
    position: usize,
}

impl Completion {
    pub fn new(
        completion: impl Into<String>,
        display: impl Into<String>,
        position: usize,
    ) -> Completion {
        Completion {
            completion: completion.into(),
            display: display.into(),
            position,
        }
    }

    pub fn complete(&self, line: &str) -> String {
        let mut res = line.to_string();
        res.insert_str(self.position, &self.completion);
        res
    }

    pub fn display(&self) -> &str {
        &self.display
    }

    pub fn replacement(&self) -> &str {
        &self.completion
    }
}

fn is_or_has_type(value: &Value, pattern: &ValueType, max_depth: i8) -> bool {
    if pattern.is(value) {
        return true;
    }
    has_type(value, pattern, max_depth)
}

fn has_type(value: &Value, pattern: &ValueType, max_depth: i8) -> bool {
    if max_depth <= 0 {
        return false;
    }
    for name in &value.fields() {
        if value
            .field(name)
            .map(|opt| {
                opt.map(|val| is_or_has_type(&val, pattern, max_depth - 1))
                    .unwrap_or(false)
            })
            .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

fn completion_suffix(maybe_scope: CrushResult<Option<Value>>, t: &ValueType) -> &str {
    match t {
        // A completed name that itself resolves to a namespace (`Value::Scope`) used
        // to suggest appending `:` to invite completing straight into a member of
        // it. `.` can't play the same role unconditionally: a bare (no `$`) namespace
        // name can't be dot-chained into a command the way it could be `:`-chained
        // (see the dot_member_operator.crush test for why -- it'd be ambiguous with a
        // real external command like `backup.sh`). A bare namespace name is still a
        // complete, valid reference on its own (running it just lists its members),
        // so a plain space is the suggestion that's never wrong -- at the cost of no
        // longer inviting the chain for the `$namespace.` case where it would work.
        ValueType::Any => match maybe_scope {
            Ok(Some(Value::Scope(_))) => " ",
            Ok(Some(Value::Empty))
            | Ok(Some(Value::Bool(_)))
            | Ok(Some(Value::String(_)))
            | Ok(Some(Value::Command(_))) => " ",
            _ => "",
        },

        target_type => match maybe_scope {
            Ok(Some(completion_target)) => {
                if completion_target
                    .value_type()
                    .is_compatible_with(target_type)
                {
                    if has_type(&completion_target, target_type, 8) {
                        ""
                    } else {
                        " "
                    }
                } else {
                    // Same reasoning as the `Value::Scope` case above: this name
                    // doesn't directly have the target type, but a bare (no `$`)
                    // reference to it can't be dot-chained into a member that might,
                    // so a plain space -- a complete, valid reference either way --
                    // is the suggestion that's never wrong.
                    " "
                }
            }
            _ => "",
        },
    }
}

fn prefix_match(prefix: &str, whole: &str) -> bool {
    if prefix.is_empty() && whole.starts_with('_') {
        return false;
    }
    whole.starts_with(prefix)
}

fn complete_label(
    value: Value,
    prefix: &str,
    t: &ValueType,
    cursor: usize,
    out: &mut Vec<Completion>,
) -> CrushResult<()> {
    out.append(
        &mut value
            .fields()
            .iter()
            .filter(|k| prefix_match(prefix, k))
            .filter(|k| {
                value
                    .field(*k)
                    .map(|opt| opt.map(|val| is_or_has_type(&val, t, 4)).unwrap_or(false))
                    .unwrap_or(false)
            })
            .map(|k| Completion {
                completion: format!(
                    "{}{}",
                    &k[prefix.len()..],
                    completion_suffix(value.field(k), t)
                ),
                display: k.clone(),
                position: cursor,
            })
            .collect(),
    );
    Ok(())
}

/// Every runnable `$PATH` entry (see `is_runnable_as_command`) whose name starts with
/// `prefix`, from every directory listed in `path_env` (a `:`-separated `$PATH`-shaped
/// string, e.g. `"/usr/bin:/usr/local/bin"`). Listed through `lister` -- the same
/// injectable `DirectoryLister` regular file completion already uses -- rather than
/// reading the real filesystem directly, so a test using `FakeDirectoryLister` (which
/// errors on any path it wasn't explicitly told about) never sees real, machine-specific
/// `$PATH` entries leak into its expected completion list. `path_env` is likewise a
/// plain argument rather than a real `$PATH` read, for the same reason --
/// `complete_external_commands` below is what reads the real one.
///
/// A name found in more than one directory (a common case: PATH generally has several
/// `bin` directories, and the same tool can exist in more than one) is only offered
/// once, matching how a shell would only ever run the first match.
fn complete_path_entries(
    lister: &impl DirectoryLister,
    path_env: &str,
    prefix: &str,
    cursor: usize,
    out: &mut Vec<Completion>,
) {
    let mut seen = HashSet::new();
    for dir in path_env.split(':') {
        if dir.is_empty() {
            continue;
        }
        let Ok(entries) = lister.list(dir) else {
            continue;
        };
        for entry in entries {
            let Some(name) = entry.name.to_str() else {
                continue;
            };
            if !prefix_match(prefix, name) || !seen.insert(name.to_string()) {
                continue;
            }
            if !is_runnable_as_command(&entry.full_path) {
                continue;
            }
            out.push(Completion::new(
                format!("{} ", &name[prefix.len()..]),
                name,
                cursor,
            ));
        }
    }
}

/// Command-position completion's own external-command source: every runnable entry on
/// the real `$PATH`, matching `prefix`. Complements `complete_label`, which only knows
/// about names declared in the crush scope tree (builtins, session variables, ...) --
/// without this, a script or tool that's only ever invoked as a bare external command
/// (never wrapped in a crush builtin) never completed at all.
fn complete_external_commands(
    lister: &impl DirectoryLister,
    prefix: &str,
    cursor: usize,
    out: &mut Vec<Completion>,
) -> CrushResult<()> {
    let path_env = crate::util::env::get("PATH").unwrap_or_default();
    complete_path_entries(lister, &path_env, prefix, cursor, out);
    Ok(())
}

fn complete_file(
    lister: &impl DirectoryLister,
    prefix: impl Into<PathBuf>,
    quoted: TextLiteralStyle,
    value_type: &ValueType,
    dirs_only: bool,
    cursor: usize,
    out: &mut Vec<Completion>,
) -> CrushResult<()> {
    if !value_type.is_compatible_with(&ValueType::File) {
        return Ok(());
    }
    let prefix = prefix.into();
    // Expand a leading `~`/`~user` into the real home directory for the purposes of
    // finding which directory to actually list -- `lister.list("~")` would otherwise
    // just look for a literal directory named `~`, which doesn't exist, so e.g. `~/D`
    // silently completed nothing at all. Only affects which directory gets read:
    // `prefix_str` below (the part matched against each entry's name, and the part of
    // the completion text it replaces) is still taken from the original, unexpanded
    // prefix, so a completion for `~/D` still inserts `~/Documents/`, not the expanded
    // absolute path.
    let expanded_prefix = match prefix.to_str() {
        Some(s) if s.starts_with('~') => {
            expand_user(s).map(PathBuf::from).unwrap_or_else(|_| prefix.clone())
        }
        _ => prefix.clone(),
    };
    let (prefix_str, parent) = if prefix.is_empty() {
        ("", PathBuf::from("."))
    } else if prefix.to_str().unwrap_or("").ends_with('/') {
        ("", expanded_prefix)
    } else {
        (
            prefix
                .components()
                .last()
                .and_then(|p| p.as_os_str().to_str())
                .unwrap_or(""),
            expanded_prefix
                .parent()
                .map(|p| p.to_path_buf())
                .map(|p| if p.is_empty() { PathBuf::from(".") } else { p })
                .unwrap_or(PathBuf::from("/")),
        )
    };
    if let Ok(dirs) = lister.list(parent) {
        out.append(
            &mut dirs
                .filter(|k| k.name.to_str().unwrap().starts_with(prefix_str))
                .filter(|k| !dirs_only || k.is_directory)
                .map(|k| Completion {
                    completion: format!(
                        "{}{}",
                        &k.name.to_str().unwrap()[prefix_str.len()..],
                        match (quoted, k.is_directory) {
                            (_, true) => "/",
                            (TextLiteralStyle::Quoted, false) => "' ",
                            (TextLiteralStyle::Unquoted, false) => " ",
                        },
                    ),
                    display: k.name.to_str().unwrap().to_string(),
                    position: cursor,
                })
                .collect(),
        );
    }
    Ok(())
}

fn complete_argument_name(
    arguments: &[Parameter],
    prefix: &str,
    cursor: usize,
    out: &mut Vec<Completion>,
    is_switch: bool,
) -> CrushResult<()> {
    out.append(
        &mut arguments
            .iter()
            .filter(|a| prefix_match(prefix, a.name.as_str()))
            .map(|a| Completion {
                completion: format!(
                    "{}{}",
                    &a.name[prefix.len()..],
                    if is_switch { " " } else { "=" }
                ),
                display: a.name.clone(),
                position: cursor,
            })
            .collect(),
    );
    Ok(())
}

fn complete_argument_values(
    allowed: &Vec<Value>,
    parse_result: &PartialCommandResult,
    cursor: usize,
    res: &mut Vec<Completion>,
) -> CrushResult<()> {
    for val in allowed {
        match (val, &parse_result.last_argument) {
            (Value::String(full), LastArgument::QuotedString(prefix)) => {
                if prefix_match(prefix, full) {
                    res.push(Completion::new(
                        format!("{}\" ", escape_without_quotes(&full[prefix.len()..])),
                        full.deref(),
                        cursor,
                    ));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn complete_argument_description(
    argument_description: &Parameter,
    parse_result: &PartialCommandResult,
    cursor: usize,
    scope: &Scope,
    res: &mut Vec<Completion>,
) -> CrushResult<()> {
    if let Some(allowed) = &argument_description.allowed {
        complete_argument_values(allowed, parse_result, cursor, res)?;
    }
    if let Some(cmd) = &argument_description.complete {
        cmd(&parse_result, cursor, scope, res)?;
    }
    Ok(())
}

fn complete_partial_argument(
    parse_result: PartialCommandResult,
    cursor: usize,
    scope: &Scope,
    lister: &impl DirectoryLister,
    res: &mut Vec<Completion>,
) -> CrushResult<()> {
    if let Some(desc) = parse_result.last_argument_description() {
        complete_argument_description(desc, &parse_result, cursor, scope, res)?;
    }

    let argument_type = parse_result.last_argument_type();
    let dirs_only = parse_result.last_argument_dirs_only();
    match parse_result.last_argument {
        LastArgument::Switch(name) => {
            if let CompletionCommand::Known(cmd) = parse_result.command {
                complete_argument_name(cmd.completion_data(), &name, cursor, res, true)?;
            }
        }

        LastArgument::Unknown => {
            complete_label(Value::Scope(scope.clone()), "", &argument_type, cursor, res)?;
            complete_file(lister, "", Unquoted, &argument_type, dirs_only, cursor, res)?;
            if parse_result.last_argument_name.is_none() {
                if let CompletionCommand::Known(cmd) = parse_result.command {
                    complete_argument_name(cmd.completion_data(), "", cursor, res, false)?;
                }
            }
        }

        LastArgument::Label(label) => {
            complete_label(
                Value::Scope(scope.clone()),
                &label,
                &argument_type,
                cursor,
                res,
            )?;
        }

        LastArgument::Field(label) => {
            if parse_result.last_argument_name.is_none() {
                complete_file(lister, &label, Unquoted, &argument_type, dirs_only, cursor, res)?;
                if let CompletionCommand::Known(cmd) = parse_result.command {
                    complete_argument_name(cmd.completion_data(), &label, cursor, res, false)?;
                }
            }
        }

        LastArgument::Member(parent, field) => {
            complete_label(parent, &field, &argument_type, cursor, res)?;
        }

        LastArgument::File(l, quoted) => {
            complete_file(lister, &l, quoted, &argument_type, dirs_only, cursor, res)?;
        }

        LastArgument::QuotedString(_) => {}

        LastArgument::Glob(glob) => {
            let mut glob_res = Vec::new();
            Glob::new(&glob).complete(&cwd()?, &mut glob_res)?;
            for comp in glob_res {
                res.push(Completion::new(comp.clone(), comp, 0))
            }
        }
    }
    Ok(())
}

pub fn complete(
    line: &str,
    cursor: usize,
    scope: &Scope,
    parser: &Parser,
    lister: &impl DirectoryLister,
) -> CrushResult<Vec<Completion>> {
    let parse_result = parse(line, cursor, scope, parser)?;
    let mut res = Vec::new();
    match parse_result {
        ParseResult::Nothing => {
            complete_label(
                Value::Scope(scope.clone()),
                "",
                &ValueType::Any,
                cursor,
                &mut res,
            )?;
            complete_external_commands(lister, "", cursor, &mut res)?;
            complete_file(lister, "", Unquoted, &ValueType::Any, false, cursor, &mut res)?;
        }

        ParseResult::PartialLabel(label) => {
            complete_label(
                Value::Scope(scope.clone()),
                &label,
                &ValueType::Any,
                cursor,
                &mut res,
            )?;
        }

        ParseResult::PartialField(label) => {
            complete_label(
                Value::Scope(scope.clone()),
                &label,
                &ValueType::Any,
                cursor,
                &mut res,
            )?;
            complete_external_commands(lister, &label, cursor, &mut res)?;
            // A bare, unprefixed word in command position (e.g. `bur`, as opposed to
            // `./bur` or `'bur`, which the parser already routes to `PartialFile`
            // instead) can also be run as a relative-path external command -- the same
            // reason `ParseResult::Nothing` (nothing typed yet) already offers file
            // completion alongside scope labels and $PATH entries. Without this, typing
            // even one character of a local script/executable's name lost file
            // completion entirely, while a completely empty command position, or the
            // same name prefixed with `./`, both still worked.
            complete_file(lister, &label, Unquoted, &ValueType::Any, false, cursor, &mut res)?;
        }

        ParseResult::PartialMember(parent, label) => {
            complete_label(parent, &label, &ValueType::Any, cursor, &mut res)?;
        }

        ParseResult::PartialFile(cmd, quoted) => {
            complete_file(lister, &cmd, quoted, &ValueType::Any, false, cursor, &mut res)?
        }

        ParseResult::PartialArgument(parse_result) => {
            complete_partial_argument(parse_result, cursor, scope, lister, &mut res)?
        }

        ParseResult::PartialQuotedString(_) => {}
    }

    // Sorted alphabetically by what the user actually sees, with `completion`/
    // `position` only as tie-breakers for two entries that happen to display the same.
    // This also makes deduplication below trivial: different completion sources (a
    // label, an external command, a file, ...) can independently suggest the exact
    // same completion -- e.g. a directory entry named `ls` and the external command
    // `ls` on $PATH both completing to the same text at the same position -- and once
    // sorted, every such duplicate is adjacent.
    res.sort_by(|a, b| {
        (&a.display, &a.completion, a.position).cmp(&(&b.display, &b.completion, b.position))
    });
    res.dedup_by(|a, b| {
        a.display == b.display && a.completion == b.completion && a.position == b.position
    });

    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::state::contexts::CommandContext;
    use crate::lang::value::Value;
    use crate::util::directory_lister::directory_lister;
    use crate::util::directory_lister::tests::FakeDirectoryLister;
    use signature::signature;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Mutex;

    fn parser() -> Parser {
        Parser::new()
    }

    // Serializes the one test below that overrides the real process-wide `$PATH` --
    // `cargo test`'s default multi-threaded runner would otherwise let it race against
    // any other test that reads `$PATH`.
    static PATH_TEST_LOCK: Mutex<()> = Mutex::new(());

    /// Points `$PATH` at `dir` for as long as the guard is alive, restoring whatever
    /// `$PATH` held before on drop (even if the test body panics).
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

    /// A fresh scratch directory under the OS temp dir, unique per test (by name), so
    /// parallel tests never collide on the same path.
    fn scratch_dir(test_name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "crush_completion_test_{}_{}",
            test_name,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_executable(path: &std::path::Path, content: &[u8]) {
        std::fs::write(path, content).unwrap();
        let mut perms = std::fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms).unwrap();
    }

    fn write_plain_file(path: &std::path::Path, content: &[u8]) {
        std::fs::write(path, content).unwrap();
        let mut perms = std::fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(path, perms).unwrap();
    }

    fn lister() -> FakeDirectoryLister {
        let mut res = FakeDirectoryLister::new("/home/rabbit");
        res.add("burrow", &vec!["carrot", "lettuce"])
            .add("burrow/table", &vec!["water"]);
        res
    }

    fn empty_lister() -> FakeDirectoryLister {
        let mut res = FakeDirectoryLister::new("/home/rabbit");
        res.add("/home/rabbit", &vec![]);
        res
    }

    fn my_cmd(_context: CommandContext) -> CrushResult<()> {
        Ok(())
    }

    fn allowed_cmd(_context: CommandContext) -> CrushResult<()> {
        Ok(())
    }

    fn multi_argument_cmd(_context: CommandContext) -> CrushResult<()> {
        Ok(())
    }

    fn dirs_only_cmd(_context: CommandContext) -> CrushResult<()> {
        Ok(())
    }

    #[signature(my_cmd)]
    struct MyCmdSignature {
        super_fancy_argument: ValueType,
    }

    #[signature(allowed_cmd)]
    struct AllowedCmdSignature {
        #[values("foo", "bar", "baz")]
        argument: String,
    }

    #[signature(multi_argument_cmd)]
    struct MultiArgumentCmdSignature {
        #[values("foo")]
        argument1: String,
        #[values("bar")]
        argument2: String,
        #[values("baz")]
        argument3: String,
    }

    #[signature(dirs_only_cmd)]
    struct DirsOnlyCmdSignature {
        #[directories_only]
        destination: PathBuf,
    }

    fn scope_with_function() -> Scope {
        let root = Scope::create_root();
        let chld = root
            .create_namespace(
                "namespace",
                "bla",
                None,
                Box::new(|env| {
                    MyCmdSignature::declare(env)?;
                    Ok(())
                }),
            )
            .unwrap();
        let _chld2 = root
            .create_namespace(
                "other_namespace",
                "bla",
                None,
                Box::new(|env| {
                    AllowedCmdSignature::declare(env)?;
                    MultiArgumentCmdSignature::declare(env)?;
                    DirsOnlyCmdSignature::declare(env)?;
                    Ok(())
                }),
            )
            .unwrap();
        root.r#use(&chld);
        root
    }

    #[test]
    fn check_empty() {
        let line = "";
        let cursor = 0;

        let s = Scope::create_root();
        s.declare("abcd", Value::Empty).unwrap();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "abcd ");
    }

    #[test]
    fn check_empty_with_file() {
        let line = "";
        let cursor = 0;

        let s = Scope::create_root();
        let completions = complete(line, cursor, &s, &parser(), &lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "burrow/");
    }

    #[test]
    fn check_argument_completion() {
        let line = "my_cmd super_";
        let cursor = line.len();

        let s = scope_with_function();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(
            &completions[0].complete(line),
            "my_cmd super_fancy_argument="
        );
    }

    #[test]
    fn check_namespaced_argument_completion() {
        let line = "$namespace.my_cmd super_";
        let cursor = line.len();

        let s = scope_with_function();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(
            &completions[0].complete(line),
            "$namespace.my_cmd super_fancy_argument="
        );
    }

    #[test]
    fn check_argument_completion_when_cursor_isnt_on_anything() {
        let line = "my_cmd ";
        let cursor = line.len();

        let s = scope_with_function();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
    }

    #[test]
    fn check_switch_completion() {
        let line = "my_cmd --super_";
        let cursor = line.len();

        let s = scope_with_function();
        s.declare("super_confusing_variable", Value::Empty).unwrap();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(
            &completions[0].complete(line),
            "my_cmd --super_fancy_argument "
        );
    }

    #[test]
    fn test_that_attribute_component_can_be_completed() {
        let line = "$namespace.my";
        let cursor = line.len();

        let s = scope_with_function();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "$namespace.my_cmd ");
    }

    #[test]
    fn test_that_empty_attribute_component_can_be_completed() {
        let line = "$namespace.";
        let cursor = line.len();

        let s = scope_with_function();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "$namespace.my_cmd ");
    }

    /// A bare (no `$`) namespace completion can't suggest chaining straight into a
    /// member the way `$namespace.` can (a bare `namespace.command` never resolves,
    /// only `$namespace.command` does -- see dot_member_operator.crush), so it's
    /// offered as a complete, standalone reference instead, exactly like completing
    /// an ordinary command name.
    #[test]
    fn test_that_bare_scope_completion_appends_a_space() {
        let line = "namespac";
        let cursor = line.len();

        let s = scope_with_function();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "namespace ");
    }

    #[test]
    fn test_that_commands_can_be_completed_in_a_pipeline() {
        let line = "a | ";
        let cursor = line.len();

        let s = Scope::create_root();
        s.declare("abcd", Value::Empty).unwrap();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "a | abcd ");
    }

    #[test]
    fn check_empty_switch_completion() {
        let line = "my_cmd --";
        let cursor = line.len();

        let s = scope_with_function();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(
            &completions[0].complete(line),
            "my_cmd --super_fancy_argument "
        );
    }

    #[test]
    fn check_empty_token() {
        let line = "a ";
        let cursor = line.len();

        let s = Scope::create_root();
        s.declare("abcd", Value::Empty).unwrap();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "a abcd ");
    }

    #[test]
    fn check_completion_of_subcommand_works() {
        let line = "x $(a";
        let cursor = line.len();

        let s = Scope::create_root();
        s.declare("abcd", Value::Empty).unwrap();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "x $(abcd ");
    }

    #[test]
    fn check_completion_of_expression_subcommand_works() {
        let line = "x (a";
        let cursor = line.len();

        let s = Scope::create_root();
        s.declare("abcd", Value::Empty).unwrap();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "x (abcd ");
    }

    #[test]
    fn complete_simple_command() {
        let line = "ab";
        let cursor = line.len();

        let s = Scope::create_root();
        s.declare("abcd", Value::Empty).unwrap();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "abcd ");
    }

    #[test]
    fn complete_simple_file() {
        let line = "'bur";
        let cursor = line.len();

        let s = Scope::create_root();
        let completions = complete(line, cursor, &s, &parser(), &lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "'burrow/");
    }

    #[test]
    fn complete_simple_file_with_dot() {
        let line = "./bur";
        let cursor = line.len();

        let s = Scope::create_root();
        let completions = complete(line, cursor, &s, &parser(), &lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "./burrow/");
    }

    /// `~/Doc` (e.g. the second argument to `mv ~/Doc`) must expand `~` to the real
    /// home directory before listing it -- `lister.list("~")` would otherwise look for
    /// a literal directory named `~`, which doesn't exist, so typing `~/<anything>`
    /// silently completed nothing at all. Rooted at the *real* `expand_user("~")` (the
    /// same function the fix itself calls, going through the real OS user database --
    /// not mockable, so this is the only way to test it deterministically) rather than
    /// the fake lister's usual "/home/rabbit", so the fix is exercised end to end.
    #[test]
    fn complete_file_expands_tilde_to_home_directory() {
        let home = PathBuf::from(expand_user("~").unwrap());
        let mut home_lister = FakeDirectoryLister::new(home.clone());
        home_lister.add(home.join("Documents"), &["notes.txt"]);

        let line = "mv ~/Doc";
        let cursor = line.len();

        let s = Scope::create_root();
        let completions = complete(line, cursor, &s, &parser(), &home_lister).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "mv ~/Documents/");
    }

    #[test]
    fn complete_file_with_cursor_after_slash() {
        let line = "./burrow/";
        let cursor = line.len();

        let s = Scope::create_root();
        let completions = complete(line, cursor, &s, &parser(), &lister()).unwrap();
        assert_eq!(completions.len(), 3);
        assert_eq!(&completions[0].complete(line), "./burrow/carrot ");
    }

    #[test]
    fn complete_long_path() {
        let line = "./burrow/car";
        let cursor = line.len();

        let s = Scope::create_root();
        let completions = complete(line, cursor, &s, &parser(), &lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "./burrow/carrot ");
    }

    #[test]
    fn complete_long_quoted_path() {
        let line = "'burrow/car";
        let cursor = line.len();

        let s = Scope::create_root();
        let completions = complete(line, cursor, &s, &parser(), &lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "'burrow/carrot' ");
    }

    /// A bare, unprefixed word in command position (`bur`, with no `./` or quote) is
    /// how you'd start typing a relative-path external command/script -- it should get
    /// file completion exactly like an empty command position already does (see
    /// `ParseResult::Nothing`'s own handling in `complete`), not just once it's already
    /// unambiguous via `./bur` or `'bur` (both parse to `PartialFile` instead, a
    /// separate case that already worked). Before the fix, `ParseResult::PartialField`
    /// only offered scope labels and `$PATH` entries, silently dropping file completion
    /// the moment a single character was typed.
    #[test]
    fn complete_bare_command_position_offers_file_completion() {
        let line = "bur";
        let cursor = line.len();

        let s = Scope::create_root();
        let completions = complete(line, cursor, &s, &parser(), &lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "burrow/");
    }

    #[test]
    fn complete_directories_only_excludes_plain_files() {
        // Same "./burrow/" listing complete_file_with_cursor_after_slash above uses
        // (carrot, lettuce, and subdirectory table) -- that test shows all 3 get
        // offered normally; #[directories_only] on this fixture command's own
        // parameter should narrow that down to just table.
        let line = "$other_namespace.dirs_only_cmd destination=./burrow/";
        let cursor = line.len();

        let s = scope_with_function();
        let completions = complete(line, cursor, &s, &parser(), &lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(
            &completions[0].complete(line),
            "$other_namespace.dirs_only_cmd destination=./burrow/table/"
        );
    }

    #[test]
    fn complete_namespaced_command() {
        let line = "$abcd.bc";
        let cursor = line.len();

        let s = Scope::create_root();
        s.create_namespace(
            "abcd",
            "bla",
            None,
            Box::new(|env| {
                env.declare("bcde", Value::Empty).unwrap();
                Ok(())
            }),
        )
        .unwrap();

        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "$abcd.bcde ");
    }

    #[test]
    fn complete_namespaced_argument() {
        let line = "xxx $abcd.bc";
        let cursor = line.len();

        let s = Scope::create_root();
        s.create_namespace(
            "abcd",
            "bla",
            None,
            Box::new(|env| {
                env.declare("bcde", Value::Empty).unwrap();
                Ok(())
            }),
        )
        .unwrap();

        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "xxx $abcd.bcde ");
    }

    #[test]
    fn complete_simple_argument() {
        let line = "abcd $ab";
        let cursor = line.len();

        let s = Scope::create_root();
        s.declare("abcd", Value::Empty).unwrap();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "abcd $abcd ");
    }

    #[test]
    fn check_cursor_in_middle_of_token() {
        let line = "ab";
        let cursor = 1;

        let s = Scope::create_root();
        s.declare("abcd", Value::Empty).unwrap();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "abcd b");
    }

    #[test]
    fn check_multiple_token() {
        let line = "ab $cd ef";
        let cursor = 6;

        let s = Scope::create_root();
        s.declare("cdef", Value::Empty).unwrap();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "ab $cdef  ef");
    }

    #[test]
    fn check_named_argument() {
        let line = "ab foo=$cd";
        let cursor = 10;

        let s = Scope::create_root();
        s.declare("cdef", Value::Empty).unwrap();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "ab foo=$cdef ");
    }

    #[test]
    fn check_fake_appended_labels_are_ignored() {
        let line = "a b=";
        let cursor = line.len();

        let s = Scope::create_root();
        s.declare("xxxx", Value::Empty).unwrap();
        s.declare("aaaa", Value::Empty).unwrap();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 2);
    }

    #[test]
    fn check_completion_type_filtering() {
        let line = "my_cmd super_fancy_argument=$t";
        let cursor = line.len();

        let s = scope_with_function();
        s.declare("tumbleweed", Value::Empty).unwrap();
        s.declare("type", Value::Type(ValueType::Empty)).unwrap();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(
            &completions[0].complete(line),
            "my_cmd super_fancy_argument=$type "
        );
    }

    #[test]
    fn check_allowed_value_completion() {
        let line = "$other_namespace.allowed_cmd argument=\"f";
        let cursor = line.len();

        let s = scope_with_function();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(
            &completions[0].complete(line),
            "$other_namespace.allowed_cmd argument=\"foo\" "
        );
    }

    #[test]
    fn check_simple_argument_description_tracking() {
        let line = "$other_namespace.allowed_cmd \"f";
        let cursor = line.len();

        let s = scope_with_function();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(
            &completions[0].complete(line),
            "$other_namespace.allowed_cmd \"foo\" "
        );
    }

    #[test]
    fn check_unnamed_argument_description_tracking() {
        let line = "$other_namespace.multi_argument_cmd \"foo\" \"";
        let cursor = line.len();

        let s = scope_with_function();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(
            &completions[0].complete(line),
            "$other_namespace.multi_argument_cmd \"foo\" \"bar\" "
        );
    }

    #[test]
    fn check_many_unnamed_argument_description_tracking() {
        let line = "$other_namespace.multi_argument_cmd \"foo\" \"bar\" \"";
        let cursor = line.len();

        let s = scope_with_function();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(
            &completions[0].complete(line),
            "$other_namespace.multi_argument_cmd \"foo\" \"bar\" \"baz\" "
        );
    }

    #[test]
    fn check_named_argument_description_tracking() {
        let line = "$other_namespace.multi_argument_cmd \"foo\" argument3=\"baz\" \"";
        let cursor = line.len();

        let s = scope_with_function();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(
            &completions[0].complete(line),
            "$other_namespace.multi_argument_cmd \"foo\" argument3=\"baz\" \"bar\" "
        );
    }

    #[test]
    fn check_named_and_unnamed_argument_description_tracking() {
        let line = "$other_namespace.multi_argument_cmd argument2=\"bar\" \"foo\" \"";
        let cursor = line.len();

        let s = scope_with_function();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(
            &completions[0].complete(line),
            "$other_namespace.multi_argument_cmd argument2=\"bar\" \"foo\" \"baz\" "
        );
    }

    #[test]
    fn check_method_completion() {
        let line = "\"\".f";
        let cursor = line.len();

        let s = Scope::create_root();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "\"\".format ");
    }

    // The interactive REPL never completes directly against the scope commands are
    // declared into -- `main.rs` declares every builtin into a root scope, then hands
    // completion a *child* of that scope (`root_scope.create_child(&root_scope,
    // Namespace)`), the same way the top-level session scope always works, so that
    // session-local variables/closures don't pollute the read-only builtins registry.
    // Every test above completes directly against the declaring scope, which is why
    // none of them caught this: name *resolution* (`Scope::get`, used to actually run
    // a typed command) already walks up through `parent_scope`, but name *enumeration*
    // (`Value::fields()`, used for completion) used `Scope::dump_local()`, which
    // doesn't -- so completing from a child scope silently found nothing declared in
    // any ancestor, i.e. no completions at all for a fresh interactive session.
    fn scope_with_command_in_parent() -> Scope {
        let root = Scope::create_root();
        root.declare("my_cmd", Value::Command(MyCmdSignature::create_command()))
            .unwrap();
        root.declare("my_var", Value::Empty).unwrap();
        root.create_child(&root, crate::lang::state::scope::ScopeType::Namespace)
    }

    #[test]
    fn check_command_completion_from_child_scope() {
        let line = "my_c";
        let cursor = line.len();

        let s = scope_with_command_in_parent();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "my_cmd ");
    }

    #[test]
    fn check_variable_completion_from_child_scope() {
        let line = "$my_v";
        let cursor = line.len();

        let s = scope_with_command_in_parent();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "$my_var ");
    }

    /// End-to-end regression test against the real builtins tree, set up exactly the way
    /// `main.rs` does for an interactive session (`declare` into a root scope, then hand
    /// completion a *child* of it) -- so this fails the same way the reported bug did if
    /// the root-scope-vs-child-scope distinction above is ever reintroduced.
    #[test]
    fn check_builtin_command_completion_from_real_interactive_session_scope() {
        let root = Scope::create_root();
        crate::builtins::declare(&root).unwrap();
        let session_scope = root.create_child(&root, crate::lang::state::scope::ScopeType::Namespace);

        let line = "wh";
        let cursor = line.len();
        let completions = complete(line, cursor, &session_scope, &parser(), &empty_lister()).unwrap();
        assert!(
            !completions.is_empty(),
            "expected at least one builtin command completing `wh` (e.g. `which`)"
        );
        assert!(completions.iter().any(|c| c.display() == "which"));
    }

    /// `complete_path_entries` only offers `$PATH` entries `is_runnable_as_command`
    /// considers valid: a real executable and a real directory, but not a plain,
    /// non-executable file -- exactly the same distinction the earlier syntax-
    /// highlighting fix draws for a bare word used as a command.
    #[test]
    fn complete_path_entries_offers_only_runnable_entries() {
        let dir = scratch_dir("offers_only_runnable");
        write_executable(&dir.join("wh_tool"), b"#!/bin/sh\necho hi\n");
        write_plain_file(&dir.join("wh_data"), b"not a program");
        std::fs::create_dir_all(dir.join("wh_dir")).unwrap();

        let mut res = Vec::new();
        complete_path_entries(&directory_lister(), &dir.display().to_string(), "wh_", 3, &mut res);

        let mut names: Vec<&str> = res.iter().map(|c| c.display()).collect();
        names.sort();
        assert_eq!(names, vec!["wh_dir", "wh_tool"]);
    }

    /// The same tool present on more than one `$PATH` directory (a common real-world
    /// case -- PATH generally has several `bin` directories) is only offered once.
    #[test]
    fn complete_path_entries_deduplicates_across_directories() {
        let dir1 = scratch_dir("dedup_1");
        let dir2 = scratch_dir("dedup_2");
        write_executable(&dir1.join("dup_tool"), b"#!/bin/sh\n");
        write_executable(&dir2.join("dup_tool"), b"#!/bin/sh\n");
        let path_env = format!("{}:{}", dir1.display(), dir2.display());

        let mut res = Vec::new();
        complete_path_entries(&directory_lister(), &path_env, "dup_", 4, &mut res);
        assert_eq!(res.len(), 1);
    }

    /// A `FakeDirectoryLister`-backed test (every other test in this module) never sees
    /// real `$PATH` entries leak in, since the fake lister errors on any path it wasn't
    /// explicitly told about -- confirmed here directly rather than just inferred from
    /// the other tests still passing.
    #[test]
    fn complete_path_entries_ignores_real_path_when_using_fake_lister() {
        let mut res = Vec::new();
        complete_path_entries(&empty_lister(), "/usr/bin:/bin", "", 0, &mut res);
        assert!(res.is_empty());
    }

    /// End-to-end: `complete()` itself, with the real `$PATH` pointed at a scratch
    /// directory holding one real executable -- the exact scenario reported ("`wh`
    /// should complete with external commands too").
    #[test]
    fn check_external_command_completion_end_to_end() {
        let _lock = PATH_TEST_LOCK.lock().unwrap();
        let dir = scratch_dir("end_to_end");
        write_executable(&dir.join("which_gizmo"), b"#!/bin/sh\n");
        let _guard = PathGuard::set(&dir);

        let line = "which_g";
        let cursor = line.len();
        let s = Scope::create_root();
        let completions = complete(line, cursor, &s, &parser(), &directory_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "which_gizmo ");
    }

    /// The exact scenario this guards: a name that's *both* a scope function and a
    /// runnable `$PATH` entry (easily happens -- a builtin and a real tool sharing a
    /// name, or just two different completion sources genuinely agreeing) must still
    /// only be offered once, not once per source that happened to suggest it.
    #[test]
    fn check_completion_deduplicates_across_sources() {
        let _lock = PATH_TEST_LOCK.lock().unwrap();
        let dir = scratch_dir("dedup_across_sources");
        write_executable(&dir.join("which_gizmo"), b"#!/bin/sh\n");
        let _guard = PathGuard::set(&dir);

        let line = "which_g";
        let cursor = line.len();
        let s = Scope::create_root();
        s.declare("which_gizmo", Value::Command(MyCmdSignature::create_command()))
            .unwrap();
        let completions = complete(line, cursor, &s, &parser(), &directory_lister()).unwrap();
        assert_eq!(
            completions.len(),
            1,
            "expected one deduplicated completion, got {:?}",
            completions.iter().map(|c| c.display()).collect::<Vec<_>>()
        );
        assert_eq!(&completions[0].complete(line), "which_gizmo ");
    }

    /// Completions from unrelated sources (here, two scope variables) come back sorted
    /// alphabetically by what's actually displayed, regardless of declaration order.
    #[test]
    fn check_completions_are_sorted_alphabetically() {
        let line = "";
        let cursor = 0;

        let s = Scope::create_root();
        s.declare("zebra", Value::Empty).unwrap();
        s.declare("apple", Value::Empty).unwrap();
        s.declare("mango", Value::Empty).unwrap();
        let completions = complete(line, cursor, &s, &parser(), &empty_lister()).unwrap();
        let displays: Vec<&str> = completions.iter().map(|c| c.display()).collect();
        let mut sorted = displays.clone();
        sorted.sort();
        assert_eq!(displays, sorted);
        assert_eq!(displays, vec!["apple", "mango", "zebra"]);
    }
}
