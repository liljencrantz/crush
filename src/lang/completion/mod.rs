use crate::lang::data::scope::Scope;
use crate::lang::errors::{CrushResult, mandate};
use crate::lang::value::{ValueType, Value};
use crate::util::directory_lister::DirectoryLister;
use std::path::PathBuf;
use crate::lang::completion::parse::{ParseResult, CompletionCommand, LastArgument, parse, PartialCommandResult};
use nix::NixPath;
use crate::lang::command::ArgumentDescription;

pub mod parse;

pub struct Completion {
    completion: String,
    display: String,
    position: usize,
}

impl Completion {
    pub fn complete(
        &self,
        line: &str,
    ) -> String {
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
    if max_depth <= 0 {
        return false;
    }
    if pattern.is(value) {
        return true;
    }
    for name in &value.fields() {
        if value.field( name)
            .map(|opt| opt.map(|val|
                is_or_has_type(&val, pattern, max_depth - 1))
                .unwrap_or(false))
            .unwrap_or(false) {
            return true;
        }
    }
    false
}

fn complete_value(
    value: Value,
    prefix: &[String],
    t: &ValueType,
    cursor: usize,
    out: &mut Vec<Completion>,
) -> CrushResult<()> {
    if prefix.len() == 1 {
        out.append(&mut value.fields()
            .iter()
            .filter(|k| k.starts_with(&prefix[0]))
            .filter(|k| value.field( *k)
                .map(|opt| opt.map(
                    |val| is_or_has_type(&val, t, 4))
                    .unwrap_or(false))
                .unwrap_or(false))
            .map(|k| Completion {
                completion: k[prefix[0].len()..].to_string(),
                display: k.clone(),
                position: cursor,
            })
            .collect());
        Ok(())
    } else {
        let child = mandate(value.field(&prefix[0])?, "Unknown member")?;
        complete_value(child, &prefix[1..], t, cursor, out)
    }
}

fn complete_file(
    lister: &impl DirectoryLister,
    prefix: impl Into<PathBuf>,
    value_type: &ValueType,
    cursor: usize,
    out: &mut Vec<Completion>,
) -> CrushResult<()> {
    if !value_type.is_compatible_with(&ValueType::File) {
        return Ok(());
    }
    let prefix = prefix.into();
    let (prefix_str, parent) = if prefix.is_empty() {
        (
            "",
            PathBuf::from(".")
        )
    } else {
        (
            prefix.components().last().and_then(|p| p.as_os_str().to_str()).unwrap_or(""),
            prefix.parent()
                .map(|p| p.to_path_buf())
                .map(|p| if p.is_empty() { PathBuf::from(".") } else { p })
                .unwrap_or(PathBuf::from("/")),
        )
    };
    if let Ok(dirs) = lister.list(parent) {
        out.append(&mut dirs
            .filter(|k| k.name.to_str().unwrap().starts_with(prefix_str))
            .map(|k| Completion {
                completion: k.name.to_str().unwrap()[prefix_str.len()..].to_string(),
                display: k.name.to_str().unwrap().to_string(),
                position: cursor,
            })
            .collect());
    }
    Ok(())
}

fn complete_argument_name(
    arguments: &Vec<ArgumentDescription>,
    prefix: &str,
    cursor: usize,
    out: &mut Vec<Completion>,
    is_switch: bool,
) -> CrushResult<()> {
    out.append(&mut arguments
        .iter()
        .filter(|a| a.name.starts_with(prefix))
        .map(|a| Completion {
            completion: format!(
                "{}{}",
                &a.name[prefix.len()..],
                if is_switch { "" } else { "=" }),
            display: a.name.clone(),
            position: cursor,
        })
        .collect());
    Ok(())
}

pub fn complete_partial_argument(
    parse_result: PartialCommandResult,
    cursor: usize,
    scope: &Scope,
    lister: &impl DirectoryLister,
    res: &mut Vec<Completion>,
) -> CrushResult<()> {
    let argument_type = parse_result.last_argument_type();
    match parse_result.last_argument {

        LastArgument::Switch(name) => {
            if let CompletionCommand::Known(cmd) = parse_result.command {
                complete_argument_name(cmd.arguments(), &name, cursor, res, true)?;
            }
        }

        LastArgument::Unknown => {
            complete_value(Value::Scope(scope.clone()), &vec!["".to_string()], &argument_type, cursor, res)?;
            complete_file(lister, "", &argument_type, cursor, res)?;
            if parse_result.last_argument_name.is_none() {
                if let CompletionCommand::Known(cmd) = parse_result.command {
                    complete_argument_name(cmd.arguments(), "", cursor, res, false)?;
                }
            }
        }

        LastArgument::Field(l) => {
            complete_value(Value::Scope(scope.clone()), &l, &argument_type, cursor, res)?;
            if l.len() == 1 {
                complete_file(lister, &l[0], &argument_type, cursor, res)?;
                if parse_result.last_argument_name.is_none() {
                    if let CompletionCommand::Known(cmd) = parse_result.command {
                        complete_argument_name(cmd.arguments(), &l[0], cursor, res, false)?;
                    }
                }
            }
        }

        LastArgument::Path(l) => {
            complete_file(lister, &l, &argument_type, cursor, res)?;
        }

        LastArgument::QuotedString(_) => {}

    }
    Ok(())
}

pub fn complete(
    line: &str,
    cursor: usize,
    scope: &Scope,
    lister: &impl DirectoryLister,
) -> CrushResult<Vec<Completion>> {
    let parse_result = parse(line, cursor, scope)?;
    let mut res = Vec::new();
    match parse_result {

        ParseResult::Nothing => {
            complete_value(Value::Scope(scope.clone()), &vec!["".to_string()], &ValueType::Any, cursor, &mut res)?;
            complete_file(lister, "", &ValueType::Any, cursor, &mut res)?;
        }

        ParseResult::PartialCommand(cmd) => {
            complete_value(Value::Scope(scope.clone()), &cmd, &ValueType::Any, cursor, &mut res)?;
            if cmd.len() == 1 {
                complete_file(lister, &cmd[0], &ValueType::Any, cursor, &mut res)?;
            }
        }

        ParseResult::PartialPath(cmd) =>
            complete_file(lister, &cmd, &ValueType::Any, cursor, &mut res)?,

        ParseResult::PartialArgument(parse_result) =>
            complete_partial_argument(parse_result, cursor, scope, lister, &mut res)?,
    }

    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::value::Value;
    use crate::util::directory_lister::FakeDirectoryLister;
    use signature::signature;
    use crate::lang::execution_context::CommandContext;

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

    #[signature(my_cmd)]
    struct MyCmdSignature {
        super_fancy_argument: ValueType,
    }

    fn scope_with_function() -> Scope {
        let root = Scope::create_root();
        let chld = root.create_namespace("namespace", Box::new(|env| {
            MyCmdSignature::declare(env)?;
            Ok(())
        })).unwrap();
        root.r#use(&chld);
        root
    }

    #[test]
    fn check_empty() {
        let line = "";
        let cursor = 0;

        let s = Scope::create_root();
        s.declare("abcd", Value::Empty()).unwrap();
        let completions = complete(line, cursor, &s, &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "abcd");
    }

    #[test]
    fn check_empty_with_file() {
        let line = "";
        let cursor = 0;

        let s = Scope::create_root();
        let completions = complete(line, cursor, &s, &lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "burrow");
    }

    #[test]
    fn check_argument_completion() {
        let line = "my_cmd super_";
        let cursor = 13;

        let s = scope_with_function();
        let completions = complete(line, cursor, &s, &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "my_cmd super_fancy_argument=");
    }

    #[test]
    fn check_argument_completion_when_cursor_isnt_on_anything() {
        let line = "my_cmd ";
        let cursor = 7;

        let s = scope_with_function();
        let completions = complete(line, cursor, &s, &empty_lister()).unwrap();
        assert_eq!(completions.len(), 3);
    }

    #[test]
    fn check_switch_completion() {
        let line = "my_cmd --super_";
        let cursor = 15;

        let s = scope_with_function();
        s.declare("super_confusing_variable", Value::Empty()).unwrap();
        let completions = complete(line, cursor, &s, &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "my_cmd --super_fancy_argument");
    }

    #[test]
    fn test_that_attribute_component_can_be_completed() {
        let line = "namespace:my";
        let cursor = 12;

        let s = scope_with_function();
        let completions = complete(line, cursor, &s, &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "namespace:my_cmd");
    }

    #[test]
    fn test_that_empty_attribute_component_can_be_completed() {
        let line = "namespace:";
        let cursor = 10;

        let s = scope_with_function();
        let completions = complete(line, cursor, &s, &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "namespace:my_cmd");
    }

    #[test]
    fn test_that_commands_can_be_completed_in_a_pipeline() {
        let line = "a | ";
        let cursor = 4;

        let s = Scope::create_root();
        s.declare("abcd", Value::Empty()).unwrap();
        let completions = complete(line, cursor, &s, &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "a | abcd");
    }


    #[test]
    fn check_empty_switch_completion() {
        let line = "my_cmd --";
        let cursor = 9;

        let s = scope_with_function();
        let completions = complete(line, cursor, &s, &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "my_cmd --super_fancy_argument");
    }

    #[test]
    fn check_empty_token() {
        let line = "a ";
        let cursor = 2;

        let s = Scope::create_root();
        s.declare("abcd", Value::Empty()).unwrap();
        let completions = complete(line, cursor, &s, &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "a abcd");
    }

    #[test]
    fn check_subcommand() {
        let line = "x (a";
        let cursor = 4;

        let s = Scope::create_root();
        s.declare("abcd", Value::Empty()).unwrap();
        let completions = complete(line, cursor, &s, &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "x (abcd");
    }


    #[test]
    fn complete_simple_command() {
        let line = "ab";
        let cursor = 2;

        let s = Scope::create_root();
        s.declare("abcd", Value::Empty()).unwrap();
        let completions = complete(line, cursor, &s, &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "abcd");
    }

    #[test]
    fn complete_simple_file() {
        let line = "bur";
        let cursor = 3;

        let s = Scope::create_root();
        let completions = complete(line, cursor, &s, &lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "burrow");
    }

    #[test]
    fn complete_simple_file_with_dot() {
        let line = "./bur";
        let cursor = 5;

        let s = Scope::create_root();
        let completions = complete(line, cursor, &s, &lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "./burrow");
    }

    #[test]
    fn complete_long_path() {
        let line = "burrow/car";
        let cursor = 10;

        let s = Scope::create_root();
        let completions = complete(line, cursor, &s, &lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "burrow/carrot");
    }

    #[test]
    fn complete_namespaced_command() {
        let line = "abcd:bc";
        let cursor = 7;

        let s = Scope::create_root();
        s.create_namespace("abcd", Box::new(|env| {
            env.declare("bcde", Value::Empty()).unwrap();
            Ok(())
        })).unwrap();

        let completions = complete(line, cursor, &s, &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "abcd:bcde");
    }

    #[test]
    fn complete_namespaced_argument() {
        let line = "xxx abcd:bc";
        let cursor = 11;

        let s = Scope::create_root();
        s.create_namespace("abcd", Box::new(|env| {
            env.declare("bcde", Value::Empty()).unwrap();
            Ok(())
        })).unwrap();

        let completions = complete(line, cursor, &s, &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "xxx abcd:bcde");
    }

    #[test]
    fn complete_simple_argument() {
        let line = "abcd ab";
        let cursor = 7;

        let s = Scope::create_root();
        s.declare("abcd", Value::Empty()).unwrap();
        let completions = complete(line, cursor, &s, &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "abcd abcd");
    }

    #[test]
    fn check_cursor_in_middle_of_token() {
        let line = "ab";
        let cursor = 1;

        let s = Scope::create_root();
        s.declare("abcd", Value::Empty()).unwrap();
        let completions = complete(line, cursor, &s, &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "abcdb");
    }

    #[test]
    fn check_multiple_token() {
        let line = "ab cd ef";
        let cursor = 5;

        let s = Scope::create_root();
        s.declare("cdef", Value::Empty()).unwrap();
        let completions = complete(line, cursor, &s, &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "ab cdef ef");
    }

    #[test]
    fn check_named_argument() {
        let line = "ab foo=cd";
        let cursor = 9;

        let s = Scope::create_root();
        s.declare("cdef", Value::Empty()).unwrap();
        let completions = complete(line, cursor, &s, &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "ab foo=cdef");
    }

    #[test]
    fn check_fake_appended_labels_are_ignored() {
        let line = "a b=";
        let cursor = 4;

        let s = Scope::create_root();
        s.declare("xxxx", Value::Empty()).unwrap();
        s.declare("aaaa", Value::Empty()).unwrap();
        let completions = complete(line, cursor, &s, &empty_lister()).unwrap();
        assert_eq!(completions.len(), 2);
    }

    #[test]
    fn check_completion_type_filtering() {
        let line = "my_cmd super_fancy_argument=t";
        let cursor = 29;

        let s = scope_with_function();
        s.declare("tumbleweed", Value::Empty()).unwrap();
        s.declare("type", Value::Type(ValueType::Empty)).unwrap();
        let completions = complete(line, cursor, &s, &empty_lister()).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "my_cmd super_fancy_argument=type");
    }

}
