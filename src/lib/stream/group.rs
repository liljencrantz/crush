use crate::lang::argument::ArgumentHandler;
use crate::lang::command::Command;
use crate::lang::errors::{mandate, CrushResult};
use crate::lang::execution_context::CommandContext;
use crate::lang::job::JobJoinHandle;
use crate::lang::ordered_string_map::OrderedStringMap;
use crate::lang::printer::Printer;
use crate::lang::scope::Scope;
use crate::lang::stream::{channels, InputStream};
use crate::lang::table::ColumnType;
use crate::lang::table::ColumnVec;
use crate::lang::value::Field;
use crate::util::thread::{build, handle};
use crate::{
    lang::errors::argument_error,
    lang::stream::{unlimited_streams, OutputStream},
    lang::{table::Row, value::Value, value::ValueType},
};
use crossbeam::{unbounded, Receiver};
use signature::signature;
use std::collections::HashMap;

#[signature(
    group,
    can_block = true,
    short = "Group stream by the specified column(s)",
    example = "find . | group ^user ^type file_count={count} size={sum ^size}"
)]
pub struct Group {
    #[unnamed()]
    #[description("the column(s) to group by and copy into the output stream.")]
    group_by: Vec<Field>,
    #[named()]
    #[description("create these additional columns by aggregating the grouped rows using the supplied aggregation command.")]
    command: OrderedStringMap<Command>,
}

fn aggregate(
    commands: Vec<Command>,
    printer: Printer,
    scope: Scope,
    destination: OutputStream,
    task_input: Receiver<(Vec<Value>, InputStream)>,
) -> CrushResult<()> {
    while let Ok((key, rows)) = task_input.recv() {
        match commands.len() {
            0 => {
                destination.send(Row::new(key))?;
            }
            1 => {
                let (input_sender, input_receiver) = channels();
                let (output_sender, output_receiver) = channels();
                input_sender.send(Value::TableStream(rows))?;
                drop(input_sender);
                commands[0].invoke(CommandContext {
                    input: input_receiver,
                    output: output_sender,
                    arguments: vec![],
                    scope: scope.clone(),
                    this: None,
                    printer: printer.clone(),
                })?;
                let mut result = key;
                result.push(output_receiver.recv()?);
                destination.send(Row::new(result))?;
            }
            _ => {
                let mut receivers = Vec::with_capacity(commands.len());
                let mut streams = Vec::with_capacity(commands.len());
                for command in &commands {
                    let (input_sender, input_receiver) = channels();
                    let (output_sender, output_receiver) = channels();
                    streams.push(input_sender.initialize(rows.types().to_vec())?);
                    printer.handle_error(command.invoke(CommandContext {
                        input: input_receiver,
                        output: output_sender,
                        arguments: vec![],
                        scope: scope.clone(),
                        this: None,
                        printer: printer.clone(),
                    }));
                    receivers.push(output_receiver);
                }

                while let Ok(row) = rows.recv() {
                    for stream in streams.iter() {
                        let _ = stream.send(row.clone());
                    }
                }
                drop(streams);

                let mut result = key;
                for receiver in receivers {
                    result.push(receiver.recv()?);
                }
                destination.send(Row::new(result))?;
            }
        }
    }
    Ok(())
}

fn create_worker_thread(
    cfg: &Group,
    printer: &Printer,
    scope: &Scope,
    destination: &OutputStream,
    task_input: &Receiver<(Vec<Value>, InputStream)>,
) -> JobJoinHandle {
    let my_commands: Vec<Command> = cfg
        .command
        .iter()
        .map(|(_name, cmd)| cmd.copy())
        .collect::<Vec<_>>();
    let my_printer = printer.clone();
    let my_scope = scope.clone();
    let my_input = task_input.clone();
    let my_destination = destination.clone();
    handle(build("group-worker").spawn(move || {
        let local_printer = my_printer.clone();
        local_printer.handle_error(aggregate(
            my_commands,
            my_printer,
            my_scope,
            my_destination,
            my_input,
        ));
    }))
}

pub fn group(context: CommandContext) -> CrushResult<()> {
    let cfg: Group = Group::parse(context.arguments, &context.printer)?;
    let mut input = mandate(
        context.input.recv()?.stream(),
        "Expected input to be a stream",
    )?;
    let input_type = input.types().to_vec();
    let indices: Vec<usize> = cfg
        .group_by
        .iter()
        .map(|f| input_type.as_slice().find(f))
        .collect::<CrushResult<Vec<_>>>()?;

    if indices.is_empty() {
        return argument_error("No group-by column specified");
    }

    let mut output_type = indices
        .iter()
        .map(|input_idx| input_type[*input_idx].clone())
        .collect::<Vec<_>>();

    for name in cfg.command.keys() {
        output_type.push(ColumnType::new(name, ValueType::Any));
    }

    let output = context.output.initialize(output_type)?;
    let mut groups: HashMap<Vec<Value>, OutputStream> = HashMap::new();

    let (task_output, task_input) = unbounded::<(Vec<Value>, InputStream)>();

    for _ in 0..16 {
        create_worker_thread(&cfg, &context.printer, &context.scope, &output, &task_input);
    }

    drop(task_input);

    while let Ok(row) = input.read() {
        let key = indices
            .iter()
            .map(|idx| row.cells()[*idx].clone())
            .collect::<Vec<_>>();
        let val = groups.get(&key);
        match val {
            None => {
                let (output_stream, input_stream) = unlimited_streams(input_type.to_vec());
                let _ = task_output.send((key.clone(), input_stream));
                let _ = output_stream.send(row);
                groups.insert(key, output_stream);
            }
            Some(output_stream) => {
                let _ = output_stream.send(row);
            }
        }
    }
    Ok(())
}
