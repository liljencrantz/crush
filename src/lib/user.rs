use crate::lang::command::OutputType::Known;
use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::Scope;
use crate::lang::data::r#struct::Struct;
use crate::lang::value::{Value, ValueType};
use signature::signature;
use crate::lang::{data::table::ColumnType, data::table::Row};
use lazy_static::lazy_static;
use crate::util::user_map::{get_all_users, get_current_username, get_user};

#[signature(
me,
can_block = false,
short = "current user",
)]
struct Me {}

fn me(context: CommandContext) -> CrushResult<()> {
    context.output.send(get_user_value(&get_current_username()?)?)
}

lazy_static! {
    static ref LIST_OUTPUT_TYPE: Vec<ColumnType> = vec![
        ColumnType::new("name", ValueType::String),
        ColumnType::new("home", ValueType::File),
        ColumnType::new("shell", ValueType::File),
        ColumnType::new("information", ValueType::String),
        ColumnType::new("uid", ValueType::Integer),
        ColumnType::new("gid", ValueType::Integer),
    ];
}

#[signature(
list,
can_block = true,
output = Known(ValueType::TableInputStream(LIST_OUTPUT_TYPE.clone())),
short = "List all users on the system",
)]
struct List {}

fn list(context: CommandContext) -> CrushResult<()> {
    let output = context.output.initialize(LIST_OUTPUT_TYPE.clone())?;
    for u in get_all_users()? {
        output.send(Row::new(
            vec![
                Value::String(u.name),
                Value::File(u.home),
                Value::File(u.shell),
                Value::String(u.information),
                Value::Integer(u.uid as i128),
                Value::Integer(u.gid as i128),
            ]))?;
    }
    Ok(())
}

#[signature(
find,
can_block = false,
short = "find a user by name",
)]
struct Find {
    #[description("the of the user to find.")]
    name: String,
}

fn find(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Find = Find::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.output.send(get_user_value(&cfg.name)?)
}

fn get_user_value(input_name: &str) -> CrushResult<Value> {
    match get_user(input_name) {
        Ok(user) =>
            Ok(Value::Struct(Struct::new(
                vec![
                    ("name", Value::String(user.name)),
                    ("home", Value::File(user.home)),
                    ("shell", Value::File(user.shell)),
                    ("information", Value::String(user.information)),
                    ("uid", Value::Integer(user.uid as i128)),
                    ("gid", Value::Integer(user.gid as i128)),
                ],
                None,
            ))),
        Err(e) => Err(e),
    }
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "user",
        "User commands",
        Box::new(move |user| {
            Me::declare(user)?;
            Find::declare(user)?;
            List::declare(user)?;
            Ok(())
        }),
    )?;
    Ok(())
}
