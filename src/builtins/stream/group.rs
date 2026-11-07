use crate::lang::argument::{ArgumentValue, ValidationArgument};
use crate::lang::command::Command;
use crate::lang::command::{OutputType, PipeInputType};
use crate::lang::data::table::ColumnType;
use crate::lang::data::table::ColumnVec;
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::job_control::{StreamControlMessage, spawn_control_fanout};
use crate::lang::ordered_string_map::OrderedStringMap;
use crate::lang::pipe::{TableInputStream, pipe};
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::global_state::GlobalState;
use crate::lang::state::handles::JobType::Background;
use crate::lang::state::scope::Scope;
use crate::{
    lang::pipe::{TableOutputStream, unlimited_streams},
    lang::{data::table::Row, value::Value, value::ValueType},
};
use crossbeam::channel::{Receiver, Sender, unbounded};
use signature::signature;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Computes `group`'s real output type, mirroring `group()`'s own column-building
/// exactly: the group-by columns keep their real input types, and every aggregation
/// column is `Any` -- the real command never attempts to resolve what its own
/// aggregation closures actually return (an aggregation runs once per *group*, on a
/// table_input_stream of that group's rows, so there's nothing meaningful to call
/// `dynamic_output`/`output_type` against ahead of time anyway). Only the (plain,
/// literal) group-by column names need to resolve for this to work.
fn group_dynamic_output(
    input: PipeInputType,
    arguments: &[ValidationArgument],
) -> CrushResult<OutputType> {
    let input_columns: &[ColumnType] = match &input {
        PipeInputType::Known(
            ValueType::TableInputStream(c) | ValueType::TableOutputStream(c) | ValueType::Table(c),
        ) => c,
        _ => return Ok(OutputType::Unknown),
    };

    let mut group_by: Vec<(&ValidationArgument, String)> = Vec::new();
    let mut aggregate_names: Vec<String> = Vec::new();
    for arg in arguments {
        match &arg.argument_type {
            Some(name) => aggregate_names.push(name.clone()),
            None => match &arg.value {
                ArgumentValue::Value(Value::String(s)) => group_by.push((arg, s.to_string())),
                ArgumentValue::Unknown | ArgumentValue::Type(_) => return Ok(OutputType::Unknown),
                _ => return arg.error("Invalid argument"),
            },
        }
    }

    if group_by.is_empty() {
        return command_error("No group-by column specified");
    }

    let mut output_columns = Vec::new();
    for (arg, name) in &group_by {
        match input_columns.find(name) {
            Ok(idx) => output_columns.push(input_columns[idx].clone()),
            Err(_) => return arg.error(format!("Unknown column `{}`", name)),
        }
    }
    for name in &aggregate_names {
        output_columns.push(ColumnType::new_from_string(
            name.clone(),
            ValueType::Any,
            "the result of aggregating this group's rows.".to_string(),
        ));
    }

    let output_columns = output_columns.as_slice().deduplicate_names();
    Ok(OutputType::Known(ValueType::TableInputStream(output_columns)))
}

#[signature(
    stream.group,
    can_block = true,
    short = "Group stream by the specified column(s)",
    example = "# Group files in current tree by the number of hardlinks pointing to them, show",
    example = "# the number of files and the sum total file size for each link count. Sort results",
    example = "# by size.",
    example = "files --recurse | group links file_count=$count size={sum size} | sort size",
    dynamic_output = group_dynamic_output,
)]
pub struct Group {
    #[unnamed()]
    #[description("the column(s) to group by and copy into the output stream.")]
    group_by: Vec<String>,
    #[named()]
    #[description(
        "create additional columns by aggregating the grouped rows using the supplied aggregation command. The supplied command will be called once for each group, with a table_input_stream containing all rows within that group. Whatever the command outputs will be the value for the specified column for that group."
    )]
    command: OrderedStringMap<Command>,
}

fn aggregate(
    commands: Vec<Command>,
    context: &CommandContext,
    global_state: GlobalState,
    scope: Scope,
    destination: TableOutputStream,
    task_input: Receiver<(Vec<Value>, TableInputStream)>,
) -> CrushResult<()> {
    while let Ok((key, rows)) = task_input.recv() {
        match commands.len() {
            0 => {
                destination.send(Row::new(key))?;
            }
            1 => {
                let (input_sender, input_receiver) = pipe();
                let (output_sender, output_receiver) = pipe();
                input_sender.send(Value::TableInputStream(rows))?;
                drop(input_sender);
                commands[0].eval(
                    CommandContext::new(
                        &scope,
                        &global_state,
                        &context.source,
                        context.next_command_handle(),
                        Background,
                    )
                    .with_input(input_receiver)
                    .with_output(output_sender),
                )?;
                let mut result = key;
                result.push(output_receiver.recv()?);
                destination.send(Row::new(result))?;
            }
            _ => {
                let mut receivers = Vec::with_capacity(commands.len());
                let mut streams = Vec::with_capacity(commands.len());
                for command in &commands {
                    let (input_sender, input_receiver) = pipe();
                    let (output_sender, output_receiver) = pipe();
                    streams.push(input_sender.initialize(rows.types())?);

                    let local_command = command.clone();
                    let local_scope = scope.clone();
                    let local_state = global_state.clone();
                    let local_source = context.source.clone();
                    let warning_state = local_state.clone();
                    let next_id = context.next_command_handle();
                    let new_context = CommandContext::new(
                        &local_scope,
                        &local_state,
                        &local_source,
                        next_id,
                        Background,
                    )
                    .with_input(input_receiver)
                    .with_output(output_sender);

                    context.global_state.threads().spawn(
                        "group:aggr",
                        &context.next_command_handle(),
                        move || {
                            if let Err(e) = local_command.eval(new_context) {
                                warning_state.warn(&e);
                            }
                            Ok(())
                        },
                    )?;
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
    scope: &Scope,
    destination: &TableOutputStream,
    task_input: &Receiver<(Vec<Value>, TableInputStream)>,
    context: &CommandContext,
    global_state: &GlobalState,
    remaining_workers: Arc<AtomicUsize>,
    fanout_shutdown: Option<Sender<()>>,
) -> CrushResult<()> {
    let my_commands: Vec<Command> = cfg
        .command
        .iter()
        .map(|(_name, cmd)| cmd.clone())
        .collect::<Vec<_>>();
    let my_scope = scope.clone();
    let my_input = task_input.clone();
    let my_destination = destination.clone();
    let my_context = context.clone();
    let my_state = global_state.clone();
    context.global_state.threads().spawn(
        "group:collect",
        &context.next_command_handle(),
        move || {
            let warning_state = my_state.clone();
            if let Err(e) = aggregate(
                my_commands,
                &my_context,
                my_state,
                my_scope,
                my_destination,
                my_input,
            ) {
                warning_state.warn(&e);
            }
            // The last worker to finish tells the fan-out thread it can stop -- see
            // broadcast_control's own doc comment for why it can't just wait for
            // `control` (the job's shared one) to disconnect on its own.
            if remaining_workers.fetch_sub(1, Ordering::SeqCst) == 1 {
                if let Some(shutdown) = &fanout_shutdown {
                    let _ = shutdown.send(());
                }
            }
            Ok(())
        },
    )?;
    Ok(())
}

pub fn group(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Group::parse(context.remove_arguments(), &context.global_state.printer())?;
    let mut input = context.input_stream()?;
    let input_type = input.types().to_vec();
    let indices: Vec<usize> = cfg
        .group_by
        .iter()
        .map(|f| input_type.as_slice().find(f))
        .collect::<CrushResult<Vec<_>>>()?;

    if indices.is_empty() {
        return command_error("No group-by column specified");
    }

    let mut output_type = indices
        .iter()
        .map(|input_idx| input_type[*input_idx].clone())
        .collect::<Vec<_>>();

    for name in cfg.command.keys() {
        output_type.push(ColumnType::new_from_string(
            name.clone(),
            ValueType::Any,
            "the result of aggregating this group's rows.".to_string(),
        ));
    }

    let output_type = output_type.as_slice().deduplicate_names();
    const WORKER_COUNT: usize = 16;

    let output = context.initialize_output(&output_type)?;
    let mut groups: HashMap<Vec<Value>, TableOutputStream> = HashMap::new();

    let (task_output, task_input) = unbounded::<(Vec<Value>, TableInputStream)>();

    // Every worker needs its own dedicated copy of the job's control channel, not a
    // plain clone of `output`'s -- see spawn_control_fanout's and
    // TableOutputStream::control's doc comments for why a shared clone would only ever
    // let one of the sixteen workers actually respond to a pause/terminate.
    let (worker_controls, fanout_shutdown): (Vec<Option<Receiver<StreamControlMessage>>>, Option<Sender<()>>) =
        match output.control() {
            Some(control) => {
                let (receivers, shutdown) =
                    spawn_control_fanout(&context, "group:control-fanout", control, WORKER_COUNT)?;
                (receivers.into_iter().map(Some).collect(), Some(shutdown))
            }
            None => ((0..WORKER_COUNT).map(|_| None).collect(), None),
        };
    let remaining_workers = Arc::new(AtomicUsize::new(WORKER_COUNT));

    for worker_control in worker_controls {
        let worker_output = match worker_control {
            Some(control) => output.clone().with_control(control),
            None => output.clone(),
        };
        create_worker_thread(
            &cfg,
            &context.scope,
            &worker_output,
            &task_input,
            &context,
            &context.global_state,
            remaining_workers.clone(),
            fanout_shutdown.clone(),
        )?;
    }

    drop(task_input);

    while let Some(row) = input.next_row()? {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::argument::SwitchStyle;
    use crate::lang::ast::source::{Source, SourceType};
    use std::sync::Arc;

    fn columns() -> Vec<ColumnType> {
        vec![
            ColumnType::new("links", ValueType::Integer, ""),
            ColumnType::new("file", ValueType::String, ""),
        ]
    }

    fn unnamed_arg(value: ArgumentValue) -> ValidationArgument {
        ValidationArgument {
            argument_type: None,
            switch_style: SwitchStyle::None,
            value,
            source: Source::new(SourceType::Input, Arc::from("")),
        }
    }

    fn named_arg(name: &str, value: ArgumentValue) -> ValidationArgument {
        ValidationArgument {
            argument_type: Some(name.to_string()),
            switch_style: SwitchStyle::None,
            value,
            source: Source::new(SourceType::Input, Arc::from("")),
        }
    }

    fn known() -> PipeInputType {
        PipeInputType::Known(ValueType::TableInputStream(columns()))
    }

    #[test]
    fn group_by_column_keeps_its_type() {
        let args = [unnamed_arg(ArgumentValue::Value(Value::from("links")))];
        match group_dynamic_output(known(), &args).unwrap() {
            OutputType::Known(ValueType::TableInputStream(c)) => {
                assert_eq!(c.len(), 1);
                assert_eq!(c[0].name(), "links");
                assert_eq!(c[0].cell_type, ValueType::Integer);
            }
            other => panic!("expected Known(TableInputStream), got {:?}", other),
        }
    }

    #[test]
    fn aggregation_columns_are_any() {
        let args = [
            unnamed_arg(ArgumentValue::Value(Value::from("links"))),
            named_arg("count", ArgumentValue::Unknown),
        ];
        match group_dynamic_output(known(), &args).unwrap() {
            OutputType::Known(ValueType::TableInputStream(c)) => {
                assert_eq!(c.len(), 2);
                assert_eq!(c[1].name(), "count");
                assert_eq!(c[1].cell_type, ValueType::Any);
            }
            other => panic!("expected Known(TableInputStream), got {:?}", other),
        }
    }

    #[test]
    fn no_group_by_column_is_an_error() {
        let args = [named_arg("count", ArgumentValue::Unknown)];
        assert!(group_dynamic_output(known(), &args).is_err());
    }

    #[test]
    fn unknown_group_by_column_is_an_error() {
        let args = [unnamed_arg(ArgumentValue::Value(Value::from("nonexistent")))];
        assert!(group_dynamic_output(known(), &args).is_err());
    }

    #[test]
    fn unknown_input_type_is_unknown() {
        let args = [unnamed_arg(ArgumentValue::Value(Value::from("links")))];
        match group_dynamic_output(PipeInputType::Unknown, &args).unwrap() {
            OutputType::Unknown => {}
            other => panic!("expected Unknown, got {:?}", other),
        }
    }
}
