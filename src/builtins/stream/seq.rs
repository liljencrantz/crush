use crate::lang::data::table::ColumnType;
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::state::contexts::CommandContext;
use crate::lang::{data::table::Row, value::Value, value::ValueType};
use signature::signature;

#[signature(
    stream.seq,
    can_block=true,
    short = "Return a stream of sequential numbers",
    long = "With no arguments, seq counts forever starting at 0. Given one to three",
    long = "unnamed numbers, they're interpreted the same way Python's `range` does:",
    long = "`seq to` counts from 0 up to (but not including) `to`; `seq from to` starts",
    long = "at `from` instead; `seq from to step` also sets the step size. The `from`,",
    long = "`to`, and `step` named arguments below are equivalent to the two- and",
    long = "three-number unnamed forms, spelled out -- but can't be mixed with the",
    long = "unnamed form in the same call, since e.g. `seq 3 to=10` would leave it",
    long = "ambiguous which one actually sets the end of the sequence.",
    example = "seq 3",
    example = "# Prepend an index column to the output of the files command",
    example = "zip $(seq) $(files)",
)]
#[derive(Debug)]
pub struct Seq {
    #[unnamed()]
    #[description(
        "1 to 3 numbers: `to`, `from to`, or `from to step` -- see the command's own long help. Can't be combined with the from/to/step named arguments."
    )]
    args: Vec<i128>,
    #[description("the first number in the sequence. Defaults to 0.")]
    from: Option<i128>,
    #[description(
        "the end of the sequence (exclusive). If not specified, the sequence will continue forever."
    )]
    to: Option<i128>,
    #[description("the step size. Defaults to 1.")]
    step: Option<i128>,
}

pub fn seq(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Seq::parse(context.remove_arguments(), &context.global_state.printer())?;

    let (from, to, step) = if cfg.args.is_empty() {
        (cfg.from.unwrap_or(0), cfg.to, cfg.step.unwrap_or(1))
    } else {
        if cfg.from.is_some() || cfg.to.is_some() || cfg.step.is_some() {
            return command_error(
                "seq: can't combine unnamed arguments with the from/to/step named arguments.",
            );
        }
        match cfg.args.as_slice() {
            [to] => (0, Some(*to), 1),
            [from, to] => (*from, Some(*to), 1),
            [from, to, step] => (*from, Some(*to), *step),
            _ => return command_error("seq takes at most 3 unnamed arguments."),
        }
    };

    let output = context.initialize_output(&[ColumnType::new("value", ValueType::Integer)])?;

    let mut idx = from;
    loop {
        if let Some(to) = to {
            if step > 0 {
                if idx >= to {
                    break;
                }
            } else if idx <= to {
                break;
            }
        }
        output.send(Row::new(vec![Value::Integer(idx)]))?;
        idx += step;
    }
    Ok(())
}
