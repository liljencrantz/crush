use crate::lang::data::scope::Scope;
use crate::lang::errors::{CrushResult, error, mandate};
use crate::lang::parser::tokenize;
use std::collections::HashMap;
use ordered_map::OrderedMap;
use crate::lang::ast::{TokenNode, TokenType, JobListNode, CommandNode, Node};
use crate::lang::argument::ArgumentDefinition;
use crate::lang::value::{ValueDefinition, Field, ValueType, Value};
use crate::lang::command::Command;

pub struct Completion {
    completion: String,
    position: usize,
}

impl Completion {
    pub fn complete(&self,
                    line: &str,
    ) -> String {
        let mut res = line.to_string();
        res.insert_str(self.position, &self.completion);
        res
    }
}

enum CompletionCommand {
    Unknown,
    Known(Command),
}

impl Clone for CompletionCommand {
    fn clone(&self) -> Self {
        match self {
            CompletionCommand::Unknown => CompletionCommand::Unknown,
            CompletionCommand::Known(c) => CompletionCommand::Known(c.copy()),
        }
    }
}

#[derive(Clone)]
enum LastArgument {
    Unknown,
    Label(String),
    QuotedString(String),
}

#[derive(Clone)]
struct PartialCommandResult {
    command: CompletionCommand,
    previousArguments: Vec<(Option<String>, ValueType)>,
    last_argument: LastArgument,
}

#[derive(Clone)]
enum ParseResult {
    Nothing,
    PartialCommand(Field),
    PartialArgument(PartialCommandResult),
}

struct ParseState {
    vec: Vec<TokenNode>,
    idx: usize,
}

impl ParseState {
    pub fn new(vec: Vec<TokenNode>) -> ParseState {
        ParseState {
            vec,
            idx: 0,
        }
    }

    pub fn next(&mut self) -> Option<&str> {
        self.idx += 1;
        self.vec.get(self.idx).map(|t| t.data.as_str())
    }

    pub fn peek(&self) -> Option<&str> {
        self.vec.get(self.idx + 1).map(|t| t.data.as_str())
    }

    pub fn location(&self) -> Option<(usize, usize)> {
        self.vec.get(self.idx).map(|t| (t.start, t.end))
    }
}

fn complete_cmd(cmd: Option<String>, args: Vec<ArgumentDefinition>, arg: TokenNode, scope: Scope) -> CrushResult<Vec<Completion>> {
    let mut map = scope.dump()?;

    let mut res = Vec::new();

    for name in map.keys() {
        if name.starts_with(&arg.data) {
            res.push(Completion {
                completion: name.strip_prefix(&arg.data).unwrap().to_string(),
                position: arg.end,
            })
        }
    }

    Ok(res)
}


fn find_command_in_job_list(mut ast: JobListNode, cursor: usize) -> CrushResult<CommandNode> {
    for job in &ast.jobs {
        if job.location.contains(cursor) {
            for cmd in &job.commands {
                if cmd.location.contains(cursor) {
                    return Ok(cmd.clone())
                }
            }
        }
    }
    mandate(ast.jobs.last().and_then(|j| j.commands.last().map(|c| c.clone())), "Nothing to complete")
}

fn simple_path(node: &Node) -> CrushResult<Field> {
    match node {
        Node::Label(label) => Ok(vec![label.string.clone()]),
        Node::GetAttr(p, a) => {
            let mut res = simple_path(p.as_ref())?;
            res.push(a.string.clone());
            Ok(res)
        }
        _ => {
            error("Invalid path")
        }
    }
}

fn complete_parse(line: &str, cursor: usize, scope: &Scope) -> CrushResult<ParseResult> {
    let ast = crate::lang::parser::ast(&line[0..cursor])?;

    if ast.jobs.len() == 0 {
        return Ok(ParseResult::Nothing);
    }

    let cmd = find_command_in_job_list(ast, cursor)?;

    if cmd.expressions.len() == 0 {
        return Ok(ParseResult::Nothing);
    }
    else if cmd.expressions.len() == 1 {
        let cmd = &cmd.expressions[0];
        if cmd.location().contains(cursor) {
            match cmd {
                Node::Label(_) |
                Node::GetAttr(_, _) => {
                    return Ok(ParseResult::PartialCommand(simple_path(cmd)?));
                },
                Node::Path(parent, child) => {panic!("AAA");},
                Node::File(path, _) => {panic!("AAA");},
                Node::String(string) => {panic!("AAA");},
                Node::GetItem(parent, item) => {panic!("AAA");},

                _ => {return error("Can't extract command to complete")}
            }
        }
        else {
            return Ok(ParseResult::PartialArgument(
                PartialCommandResult {
                    command: CompletionCommand::Unknown,
                    previousArguments: vec![],
                    last_argument: LastArgument::Unknown
                }));
        }
    } else {
        match cmd.expressions.last().unwrap() {
            Node::Label(l) => {
                return Ok(ParseResult::PartialArgument(
                    PartialCommandResult {
                        command: CompletionCommand::Unknown,
                        previousArguments: vec![],
                        last_argument: LastArgument::Label(l.string.clone())
                    }));
            },
            Node::String(_) => {error("String completions not yet impemented")},
            _ => {
                error("Can't extract argument to complete")
            }
        }
    }
}

fn complete_value(value: Value, prefix: &[String], t: ValueType, cursor: usize) -> CrushResult<Vec<Completion>> {
    if prefix.len() == 1 {
        Ok(value.fields()
            .iter()
            .filter(|k| k.starts_with(&prefix[0]))
            .map(|k| Completion { completion: k[prefix[0].len()..].to_string(), position: cursor })
            .collect())
    } else {
        let child = mandate(value.field(&prefix[0])?, "Unknown member")?;
        complete_value(child, &prefix[1..], t, cursor)
    }
}

pub fn complete(line: &str, cursor: usize, scope: &Scope) -> CrushResult<Vec<Completion>> {
    let cmd = complete_parse(line, cursor, scope)?;

    match cmd {
        ParseResult::Nothing => {
            return complete_value(Value::Scope(scope.clone()), &vec!["".to_string()], ValueType::Any, cursor);
        },
        ParseResult::PartialCommand(cmd) => {
                return complete_value(Value::Scope(scope.clone()), &cmd, ValueType::Any, cursor)
        },
        ParseResult::PartialArgument(p) => {
            match p.command {
                CompletionCommand::Unknown => {
                    match p.last_argument {
                        LastArgument::Unknown => {
                            return complete_value(Value::Scope(scope.clone()), &vec!["".to_string()], ValueType::Any, cursor)
                        },
                        LastArgument::Label(l) => {
                            return complete_value(Value::Scope(scope.clone()), &vec![l], ValueType::Any, cursor)
                        },
                        LastArgument::QuotedString(_) => {},
                    }
                },
                CompletionCommand::Known(_) => {},
            }
        },
    }
    error("unimplemented completion")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::value::Value;
    use crate::lang::data::scope::ScopeLoader;

    #[test]
    fn check_empty() {
        let line = "";
        let cursor = 0;

        let s = Scope::create_root();
        s.declare("abcd", Value::Empty()).unwrap();
        let completions = complete(line, cursor, &s).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "abcd");
    }

    #[test]
    fn check_empty_token() {
        let line = "a ";
        let cursor = 2;

        let s = Scope::create_root();
        s.declare("abcd", Value::Empty()).unwrap();
        let completions = complete(line, cursor, &s).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "a abcd");
    }

    #[test]
    fn complete_simple_command() {
        let line = "ab";
        let cursor = 2;

        let s = Scope::create_root();
        s.declare("abcd", Value::Empty()).unwrap();
        let completions = complete(line, cursor, &s).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "abcd");
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

        let completions = complete(line, cursor, &s).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "abcd:bcde");
    }


    #[test]
    fn complete_simple_argument() {
        let line = "abcd ab";
        let cursor = 7;

        let s = Scope::create_root();
        s.declare("abcd", Value::Empty()).unwrap();
        let completions = complete(line, cursor, &s).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "abcd abcd");
    }

    #[test]
    fn check_cursor_in_middle_of_token() {
        let line = "ab";
        let cursor = 1;

        let s = Scope::create_root();
        s.declare("abcd", Value::Empty()).unwrap();
        let completions = complete(line, cursor, &s).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "abcdb");
    }

    #[test]
    fn check_multiple_token() {
        let line = "ab cd ef";
        let cursor = 5;

        let s = Scope::create_root();
        s.declare("cdef", Value::Empty()).unwrap();
        let completions = complete(line, cursor, &s).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "ab cdef ef");
    }
}
