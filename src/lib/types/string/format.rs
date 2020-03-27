use crate::lang::{value::Value, argument::Argument};
use crate::lang::errors::{argument_error, CrushResult, mandate};
use crate::lang::command::{ExecutionContext, This};
use crate::lib::types::string::format::FormatState::*;

enum FormatState {
    Normal,
    OpenBrace,
    CloseBrace,
    Index(usize),
    Name(String),
}

fn format_argument(res: &mut String, arg: Option<&Argument>) -> CrushResult<()> {
    res.push_str(mandate(arg, "Missing argument")?.value.to_string().as_str());
    Ok(())
}

fn argument_by_name<'a>(name: &str, param: &'a Vec<Argument>) -> Option<&'a Argument> {
    for a in param {
        if let Some(arg_name) = a.argument_type.as_deref() {
            if name == arg_name {
                return Some(a);
            }
        }
    }
    None
}

fn do_format(format: &str, param: Vec<Argument>) -> CrushResult<String> {
    let mut implicit_idx = 0;
    let mut res = String::new();
    let mut state = Normal;
    for ch in format.chars() {
        state = match state {
            Normal =>
                match ch {
                    '{' => OpenBrace,
                    '}' => CloseBrace,
                    _ => {
                        res.push(ch);
                        Normal
                    }
                }

            CloseBrace => {
                match ch {
                    '}' => {
                        res.push('}');
                        Normal
                    }
                    _ => return argument_error("Unmatched closing brace"),
                }
            }

            OpenBrace =>
                match ch {
                    '{' => {
                        res.push('{');
                        Normal
                    }
                    '}' => {
                        format_argument(&mut res, param.get(implicit_idx))?;
                        implicit_idx += 1;
                        Normal
                    }
                    '0'..='9' => Index(ch.to_digit(10).unwrap() as usize),
                    'a'..='z' | 'A'..='Z' => Name(ch.to_string()),
                    _ => return argument_error("Invalid format string"),
                }

            Index(idx) =>
                match ch {
                    '}' => {
                        format_argument(&mut res, param.get(idx))?;
                        Normal
                    }
                    '0'..='9' => Index(idx * 10 + ch.to_digit(10).unwrap() as usize),
                    _ => return argument_error("Invalid format string"),
                }

            Name(name) =>
                match ch {
                    '}' => {
                        format_argument(&mut res, argument_by_name(name.as_str(), &param))?;
                        Normal
                    }
                    _ => Name(name + ch.to_string().as_str()),
                }
        }
    }
    Ok(res)
}

pub fn format(context: ExecutionContext) -> CrushResult<()> {
    let format = context.this.text()?;
    context.output.send(Value::String(
        do_format(
            &format,
            context.arguments)?
            .into_boxed_str()))
}
