use crate::lang::command::OutputType::Known;
use crate::lang::data::table::{ColumnType, Row};
use crate::lang::errors::{CrushResult, command_error, terminate};
use crate::lang::job_control::{ChannelBasedController, StreamControlMessage};
use crate::lang::signature::files::Files;
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use chrono::Local;
use crossbeam::channel::{bounded, select, unbounded};
use notify::{RecursiveMode, Watcher};
use signature::signature;
use std::path::PathBuf;

static WATCH_OUTPUT_TYPE: [ColumnType; 3] = [
    ColumnType::new("path", ValueType::File),
    ColumnType::new("kind", ValueType::String),
    ColumnType::new("timestamp", ValueType::Time),
];

#[signature(
    fs.watch,
    can_block = true,
    output = Known(ValueType::table_input_stream(&WATCH_OUTPUT_TYPE)),
    short = "Watch a path for filesystem changes and stream them as rows.",
    long = "Uses the operating system's native filesystem notification API (inotify on",
    long = "Linux, FSEvents on macOS, ReadDirectoryChangesW on Windows), so what counts as a",
    long = "change and how quickly it is reported is a best-effort property of the host OS,",
    long = "not a cross-platform guarantee -- rapid changes may be coalesced, and exact",
    long = "granularity differs by platform.",
    example = "fs:watch . --recurse | where {kind == \"Create(File)\"}",
)]
pub struct Watch {
    #[description("the path to watch.")]
    path: Files,
    #[description("also watch subdirectories.")]
    #[default(false)]
    recurse: bool,
}

pub fn watch(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Watch = Watch::parse(context.remove_arguments(), &context.global_state.printer())?;
    let paths: Vec<PathBuf> = cfg.path.try_into()?;
    if paths.len() != 1 {
        return command_error(format!(
            "Expected exactly one path to watch, got {}.",
            paths.len()
        ));
    }

    let (event_sender, event_receiver) = unbounded();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let _ = event_sender.send(res);
    })?;
    watcher.watch(
        &paths[0],
        if cfg.recurse {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        },
    )?;

    let (control_sender, control_receiver) = bounded(1);
    let control = Box::from(ChannelBasedController::new(control_sender));
    context.command_handle().register(control);

    let output = context.initialize_output(&WATCH_OUTPUT_TYPE)?;

    loop {
        select! {
            recv(event_receiver) -> msg => match msg {
                Ok(Ok(event)) => {
                    let kind = format!("{:?}", event.kind);
                    let timestamp = Local::now();
                    for path in event.paths {
                        output.send(Row::new(vec![
                            Value::from(path),
                            Value::from(kind.clone()),
                            Value::Time(timestamp),
                        ]))?;
                    }
                }
                Ok(Err(_)) => {}
                Err(_) => return terminate(),
            },
            recv(control_receiver) -> msg => match msg {
                Ok(StreamControlMessage::Terminate) => return terminate(),
                Ok(StreamControlMessage::Pause) => loop {
                    match control_receiver.recv() {
                        Ok(StreamControlMessage::Resume) => break,
                        Ok(StreamControlMessage::Terminate) => return terminate(),
                        Ok(StreamControlMessage::Pause) => {}
                        Err(_) => return terminate(),
                    }
                },
                Ok(StreamControlMessage::Resume) => {}
                Err(_) => return terminate(),
            },
        }
    }
}
