use crate::lib::types::OrderedStringMap;
use signature::signature;
use crate::lang::errors::{argument_error_legacy, CrushResult, mandate};
use crate::lang::state::contexts::CommandContext;
use crate::lang::{argument::Argument, value::Value};
use crate::lang::command::OutputType::Known;
use crate::lang::state::this::This;
use crate::lang::value::ValueType;
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

fn argument_by_name<'a>(name: &str, param: &'a [Argument]) -> Option<&'a Argument> {
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
            Normal => match ch {
                '{' => OpenBrace,
                '}' => CloseBrace,
                _ => {
                    res.push(ch);
                    Normal
                }
            },

            CloseBrace => match ch {
                '}' => {
                    res.push('}');
                    Normal
                }
                _ => return argument_error_legacy("Unmatched closing brace"),
            },

            OpenBrace => match ch {
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
                _ => return argument_error_legacy("Invalid format string"),
            },

            Index(idx) => match ch {
                '}' => {
                    format_argument(&mut res, param.get(idx))?;
                    Normal
                }
                '0'..='9' => Index(idx * 10 + ch.to_digit(10).unwrap() as usize),
                _ => return argument_error_legacy("Invalid format string"),
            },

            Name(name) => match ch {
                '}' => {
                    format_argument(&mut res, argument_by_name(name.as_str(), &param))?;
                    Normal
                }
                _ => Name(name + ch.to_string().as_str()),
            },
        }
    }
    Ok(res)
}

#[signature(
    types.string.format, can_block = false, output = Known(ValueType::String),
    short = "Format arguments into a string",
    example = "\"Hello {name}\":format name=$name")]
pub struct Format {
    #[description("The named parameters to format into the pattern string")]
    #[named()]
    named: OrderedStringMap<Value>,
    #[description("The unnamed parameters to format into the pattern string")]
    #[unnamed()]
    unnamed: Vec<Value>,
}

pub fn format(mut context: CommandContext) -> CrushResult<()> {
    let format = context.this.string()?;
    context
        .output
        .send(Value::from(do_format(&format, context.arguments)?))
}
