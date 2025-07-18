use crate::lang::argument::column_names;
use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::data::r#struct::Struct;
use crate::lang::errors::CrushResult;
use crate::lang::ordered_string_map::OrderedStringMap;
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::{Value, ValueType};
use ordered_map::OrderedMap;
use signature::signature;
use std::sync::OnceLock;

pub fn methods() -> &'static OrderedMap<String, Command> {
    static CELL: OnceLock<OrderedMap<String, Command>> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();

        Of::declare_method(&mut res);
        res
    })
}

#[signature(
    types.struct.of,
    can_block = false,
    output = Known(ValueType::Struct),
    short = "Construct a struct with the specified members",
    long = "Unnamed arguments will be given the names _1, _2, _3, and so on.",
    long = "",
    long = "Unlike a struct created via the `class` command, a struct created via `struct:of` does not have a parent or a `__setattr__` method. The lack of a `__setattr__` method means that a \"data struct\" is immutable, though its members may potentially be modified, depending on their type.",
    example = "struct:of foo=5 bar=\"baz\" false",
)]
#[allow(unused)]
struct Of {
    #[description("unnamed values.")]
    #[unnamed]
    unnamed: Vec<Value>,
    #[description("named values.")]
    #[named]
    named: OrderedStringMap<Value>,
}

fn of(context: CommandContext) -> CrushResult<()> {
    let mut names = column_names(&context.arguments);
    let arr = names
        .drain(..)
        .zip(context.arguments)
        .map(|(name, arg)| (name, arg.value))
        .collect::<Vec<_>>();
    context.output.send(Value::Struct(Struct::new(arr, None)))
}
