use crate::lang::scope::Scope;
use crate::lang::errors::CrushResult;
use crate::lang::{value::Value, execution_context::ExecutionContext, execution_context::ArgumentVector, binary::BinaryReader};
use crate::lang::pretty_printer::print_value;
use crate::lang::command::CrushCommand;
use crate::lang::list::List;
use crate::lang::value::ValueType;

mod lines;
mod csv;
mod json;
mod http;

pub fn val(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    context.output.send(context.arguments.value(0)?)
}

pub fn dir(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    context.output.send(
        Value::List(List::new(ValueType::String,
                              context.arguments.value(0)?.fields()
                                  .drain(..)
                                  .map(|n| Value::String(n))
                                  .collect()))
    )
}

    fn echo(mut context: ExecutionContext) -> CrushResult<()> {
    for arg in context.arguments.drain(..) {
        print_value(arg.value);
    }
    Ok(())
}

fn cat(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::BinaryStream(BinaryReader::paths(context.arguments.files()?)?))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("io")?;
    root.r#use(&env);
    env.declare("cat", Value::Command(CrushCommand::command(
        cat, true,
        "cat @files:(file|glob)", "Read specified files as binary stream", None)))?;
    env.declare("http", Value::Command(CrushCommand::command(
        http::perform, true,
    "http url:string [form=formdata:string] [method=method:string] [header=header:string]...",
    "Make a http request",
    Some(r#"    Headers should be on the form "key:value".

        Examples:

    http "https://example.com/" header=("Authorization: Bearer {}":format token)"#))))?;
    env.declare("lines", Value::Command(CrushCommand::command(
        lines::perform, true,
        "lines @files:(file|glob)", "Read specified files as a table with one line of text per row", None)))?;
    env.declare("csv", Value::Command(CrushCommand::command(
        csv::perform, true,
        "csv <column_name>=type:type... [head=skip:integer] [separator=separator:string] [trim=trim:string] @files:(file|glob)",
        "Parse specified files as CSV files", Some(r#"    Examples:

    csv separator="," head=1 name=string age=integer nick=string"#))))?;
    env.declare("json", Value::Command(CrushCommand::command(
        json::perform, true,
        "json [file:file]", "Parse json", Some(
            r#"    Input can either be a binary stream or a file.

    Examples:

    json some_file.json

    (http "https://jsonplaceholder.typicode.com/todos/3"):body | json"#))))?;
    env.declare("echo", Value::Command(CrushCommand::command(
        echo, false,
        "echo @value:any", "Prints all arguments directly to the screen", None)))?;
    env.declare("val", Value::Command(CrushCommand::command(
        val, false,
        "val value:any",
        "Return value",
    Some(r#"    This command is useful if you want to e.g. pass a command as input in
    a pipeline instead of executing it. It is different from the echo command
    in that val returns the value, and echo prints it to screen."#))))?;
    env.declare("dir", Value::Command(CrushCommand::command(
        dir, false,
        "dir value:any", "List members of value", None)))?;
    env.readonly();

    Ok(())
}
