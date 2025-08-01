use crate::lang::command::OutputType::Known;
use crate::lang::data::table::ColumnFormat;
use crate::lang::data::table::ColumnType;
use crate::lang::data::table::Row;
use crate::lang::errors::CrushResult;
use crate::lang::pipe::TableOutputStream;
use crate::lang::signature::files;
use crate::lang::signature::files::Files;
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use crate::util::directory_lister::{DirectoryLister, directory_lister};
use signature::signature;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

static OUTPUT_TYPE: [ColumnType; 3] = [
    ColumnType::new_with_format("size", ColumnFormat::ByteUnit, ValueType::Integer),
    ColumnType::new("blocks", ValueType::Integer),
    ColumnType::new("file", ValueType::File),
];

#[signature(
    fs.usage,
    can_block = true,
    output = Known(ValueType::table_input_stream(& OUTPUT_TYPE)),
    short = "Calculate the recursive directory space usage.",
)]
pub struct Usage {
    #[unnamed()]
    #[description("the files to calculate the recursive size of.")]
    directory: Vec<Files>,

    #[description("do not show directory sizes for subdirectories.")]
    #[default(false)]
    silent: bool,

    #[description("write sizes for all files, not just directories.")]
    #[default(false)]
    all: bool,
}

fn size(
    path: &Path,
    silent: bool,
    all: bool,
    is_directory: bool,
    output: &TableOutputStream,
    lister: &impl DirectoryLister,
) -> CrushResult<(u64, u64)> {
    let mut sz = path.metadata().map(|m| m.size()).unwrap_or(0);
    let mut bl = path.metadata().map(|m| m.blocks()).unwrap_or(0);
    Ok(if is_directory {
        for child in lister.list(path)? {
            let (child_sz, child_bl) = size(
                &child.full_path,
                silent,
                all,
                child.is_directory,
                output,
                lister,
            )?;
            if (!silent && child.is_directory) || all {
                output.send(Row::new(vec![
                    Value::from(child_sz),
                    Value::from(child_bl),
                    Value::from(child.full_path),
                ]))?;
            }
            sz += child_sz;
            bl += child_bl;
        }
        (sz, bl)
    } else {
        (sz, bl)
    })
}

fn usage(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Usage = Usage::parse(context.remove_arguments(), &context.global_state.printer())?;
    let output = context.output.initialize(&OUTPUT_TYPE)?;
    let dirs = if !cfg.directory.is_empty() {
        files::into_paths(cfg.directory)?
    } else {
        vec![PathBuf::from(".")]
    };
    for file in dirs {
        let (sz, bl) = size(
            &file,
            cfg.silent,
            cfg.all,
            file.is_dir(),
            &output,
            &directory_lister(),
        )?;

        output.send(Row::new(vec![
            Value::Integer(sz as i128),
            Value::Integer(bl as i128),
            Value::from(file),
        ]))?
    }

    Ok(())
}
