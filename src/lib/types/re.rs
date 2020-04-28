use crate::lang::errors::{CrushResult, argument_error};
use crate::lang::{value::Value, execution_context::ExecutionContext};
use regex::Regex;
use crate::lang::command::CrushCommand;
use crate::lang::execution_context::{ArgumentVector, This};
use std::collections::HashMap;
use lazy_static::lazy_static;
use crate::lang::command::TypeMap;
use signature::signature;
use crate::lang::argument::ArgumentHandler;

fn full(name: &'static str) -> Vec<&'static str> {
    vec!["global", "types", "re", name]
}

lazy_static! {
    pub static ref METHODS: HashMap<String, Box<dyn CrushCommand +  Sync + Send>> = {
        let mut res: HashMap<String, Box<dyn CrushCommand +  Send + Sync>> = HashMap::new();
        res.declare(full("match"), r#match, false,
            "re =~ input:string", "True if the input matches the pattern", None);
        res.declare(full("not_match"), not_match, false,
            "re !~ input:string", "True if the input does not match the pattern", None);
        res.declare(full("replace"),
            replace, false,
            "re ~ text replacement", "Replace the first match of the regex in text with the replacement", None);
        res.declare(full("replace_all"),
            replace_all, false,
            "re ~ text replacement", "Replace all matches of the regex in text with the replacement", None);
        res.declare(full("new"),
            new, false,
            "re:new pattern:string", "Create a new regular expression instance", None);
        res
    };
}

fn new(mut context: ExecutionContext) -> CrushResult<()> {
    let def = context.arguments.string(0)?;
    let res = match Regex::new(def.as_ref()) {
        Ok(r) => Value::Regex(def, r),
        Err(e) => return argument_error(e.to_string().as_str()),
    };
    context.output.send(res)
}

fn r#match(mut context: ExecutionContext) -> CrushResult<()> {
    let re = context.this.re()?.1;
    let needle = context.arguments.string(0)?;
     context.output.send(Value::Bool(re.is_match(&needle)))
}

fn not_match(mut context: ExecutionContext) -> CrushResult<()> {
    let re = context.this.re()?.1;
    let needle = context.arguments.string(0)?;
    context.output.send(Value::Bool(!re.is_match(&needle)))
}

#[signature]
struct ReplaceSignature {
    text: String,
    replacement: String,
}

fn replace(context: ExecutionContext) -> CrushResult<()> {
    let re = context.this.re()?.1;
    let args: ReplaceSignature = ReplaceSignature::parse(context.arguments, &context.printer)?;
    context.output.send(Value::string(re.replace(&args.text, args.replacement.as_str()).as_ref()))
}

fn replace_all(context: ExecutionContext) -> CrushResult<()> {
    let re = context.this.re()?.1;
    let args: ReplaceSignature = ReplaceSignature::parse(context.arguments, &context.printer)?;
    context.output.send(Value::string(re.replace_all(&args.text, args.replacement.as_str()).as_ref()))
}
