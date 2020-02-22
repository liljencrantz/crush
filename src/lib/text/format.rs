use crate::lang::{Value, Argument};
use crate::errors::{argument_error, CrushResult, mandate};
use crate::lang::ExecutionContext;
use crate::lib::text::format::FormatState::{NORMAL, BRACKET, IDX, NAME};

enum FormatState {
    NORMAL,
    BRACKET,
    IDX(usize),
    NAME(String),
}

fn format_argument(res: &mut String, arg: Option<&Argument>) -> CrushResult<()> {
    res.push_str(mandate(arg, "Missing argument")?.value.to_string().as_str());
    Ok(())
}

fn argument_by_name<'a>(name: &str, param: & 'a Vec<Argument>) -> Option<& 'a Argument> {
    for a in param {
        if let Some(arg_name) = a.name.as_deref() {
            if name == arg_name {
                return Some(a)
            }
        }
    }
    None
}

fn do_format(format: &str, param: Vec<Argument>) -> CrushResult<String> {
    let mut implicit_idx = 0;
    let mut res = String::new();
    let mut state = NORMAL;
    for ch in format.chars() {
        state = match state {
            NORMAL =>
                match ch {
                    '{' => BRACKET,
                    _ => {
                        res.push(ch);
                        NORMAL
                    }
                }

            BRACKET =>
                match ch {
                    '}' => {
                        format_argument(&mut res, param.get(implicit_idx))?;
                        implicit_idx += 1;
                        NORMAL
                    }
                    '0'..='9' => IDX(ch.to_digit(10).unwrap() as usize),
                    'a'..='z' | 'A'..='Z' => NAME(ch.to_string()),
                    _ => return argument_error("Invalid format string"),
                }

            IDX(idx) =>
                match ch {
                    '}' => {
                        format_argument(&mut res, param.get(idx))?;
                        NORMAL
                    }
                    '0'..='9' => IDX(idx*10 + ch.to_digit(10).unwrap() as usize),
                    _ => return argument_error("Invalid format string"),

                }

            NAME(name) =>
                match ch {
                    '}' => {
                        format_argument(&mut res, argument_by_name(name.as_str(), &param))?;
                        NORMAL
                    }
                    _ => NAME(name + ch.to_string().as_str()),
                }
        }
    }
    Ok(res)
}

pub fn format(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() < 1 {
        return argument_error("Expected at least one argument");
    }
    let format_arg = context.arguments.remove(0);

    match (format_arg.name.as_deref(), format_arg.value) {
        (None, Value::Text(format)) =>
            context.output.send(Value::Text(
                do_format(
                    format.as_ref(),
                    context.arguments)?
                    .into_boxed_str())),
        _ => argument_error("Expected format string as first, unnamed argument"),
    }
}
