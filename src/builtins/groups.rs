use crate::argument_error_legacy;
use crate::lang::command::OutputType::Known;
use crate::lang::data::r#struct::Struct;
use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::Scope;
use crate::lang::value::{Value, ValueType};
use crate::lang::{data::table::ColumnType, data::table::Row};
use signature::signature;
use std::ops::Add;

static LIST_OUTPUT_TYPE: [ColumnType; 2] = [
    ColumnType::new("name", ValueType::String),
    ColumnType::new("gid", ValueType::Integer),
];

#[signature(
    groups.list,
    can_block = true,
    output = Known(ValueType::table_input_stream(&LIST_OUTPUT_TYPE)),
    short = "List all groups on the system",
)]
struct List {}

fn list(context: CommandContext) -> CrushResult<()> {
    let output = context.output.initialize(&LIST_OUTPUT_TYPE)?;
    let groups = sysinfo::Groups::new_with_refreshed_list();

    for g in groups.list() {
        output.send(Row::new(vec![
            Value::from(g.name()),
            Value::from(g.id().add(0)),
        ]))?;
    }
    Ok(())
}

#[signature(
    groups.__getitem__,
    can_block = false,
    short = "find a user by name",
    example = "# Find the group staff",
    example = "groups[staff]",
)]
struct GetItem {
    #[description("the name of the group to find.")]
    name: String,
}

fn __getitem__(mut context: CommandContext) -> CrushResult<()> {
    let cfg: GetItem = GetItem::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.output.send(get_group_value(&cfg.name)?)
}

fn get_group_value(input_name: &str) -> CrushResult<Value> {
    let groups = sysinfo::Groups::new_with_refreshed_list();
    for g in groups.list() {
        if g.name() == input_name {
            return Ok(Value::Struct(Struct::new(
                vec![
                    ("name", Value::from(g.name())),
                    ("gid", Value::from(g.id().add(0))),
                ],
                None,
            )));
        }
    }
    argument_error_legacy(format!("unknown group {}", input_name))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "groups",
        "User group commands",
        Box::new(move |groups| {
            List::declare(groups)?;
            GetItem::declare(groups)?;
            Ok(())
        }),
    )?;
    Ok(())
}
