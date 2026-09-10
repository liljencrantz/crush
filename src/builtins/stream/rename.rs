use crate::lang::command::OutputType::Passthrough;
use crate::lang::data::table::ColumnType;
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::ordered_string_map::OrderedStringMap;
use crate::lang::state::contexts::CommandContext;
use signature::signature;

#[signature(
    stream.rename,
    can_block = true,
    output = Passthrough,
    short = "Rename one or more columns of a stream.",
    long = "Every argument name is the current name of a column, and its value is the",
    long = "new name to give it. Columns not mentioned pass through unchanged.",
    example = "files | rename file=path",
)]
pub struct Rename {
    #[description("mapping from current column name to new column name.")]
    #[named()]
    renames: OrderedStringMap<String>,
}

pub fn rename(mut context: CommandContext) -> CrushResult<()> {
    let mut cfg: Rename =
        Rename::parse(context.remove_arguments(), &context.global_state.printer())?;
    let mut input = context.input_stream()?;

    let mut output_type: Vec<ColumnType> = Vec::with_capacity(input.types().len());
    for ct in input.types() {
        match cfg.renames.remove(ct.name()) {
            Some(new_name) => output_type.push(ColumnType::new_with_format_from_string(
                new_name,
                ct.format,
                ct.cell_type.clone(),
            )),
            None => output_type.push(ct.clone()),
        }
    }

    if !cfg.renames.is_empty() {
        let unknown: Vec<&String> = cfg.renames.keys().collect();
        return command_error(format!(
            "No column named `{}` to rename.",
            unknown
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join("`, `")
        ));
    }

    let output = context.initialize_output(&output_type)?;
    while let Some(row) = input.next_row()? {
        output.send(row)?;
    }
    Ok(())
}
