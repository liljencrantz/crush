use crate::lang::command::OutputType::Unknown;
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::Value;
use signature::signature;

#[signature(
    stream.union,
    can_block = true,
    output = Unknown,
    short = "Concatenate two or more streams with identical column types into one.",
    long = "Every row of the first stream is output, in order, followed by every row of",
    long = "the second stream, and so on. All streams must have identical column types.",
    example = "union $(seq 0 3) $(seq 10 13)",
)]
pub struct Union {
    #[description("the streams to concatenate.")]
    #[unnamed()]
    streams: Vec<Value>,
}

pub fn union(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Union = Union::parse(context.remove_arguments(), &context.global_state.printer())?;
    if cfg.streams.len() < 2 {
        return command_error("Expected at least two streams to union.");
    }

    let mut streams = Vec::new();
    for s in cfg.streams {
        streams.push(s.stream(context.command_handle())?);
    }

    let output_type = streams[0].types().to_vec();
    for (idx, s) in streams.iter().enumerate().skip(1) {
        if s.types() != output_type.as_slice() {
            return command_error(format!(
                "Stream {} has column types that differ from the first stream's.",
                idx + 1
            ));
        }
    }

    let output = context.initialize_output(&output_type)?;
    for mut s in streams {
        while let Some(row) = s.next_row()? {
            output.send(row)?;
        }
    }
    Ok(())
}
