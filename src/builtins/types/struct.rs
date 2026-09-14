use crate::lang::argument::column_names;
use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::data::r#struct::Struct;
use crate::lang::data::table::{ColumnType, ColumnVec};
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
        Join::declare_method(&mut res);
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

#[signature(
    types.struct.join,
    can_block = false,
    output = Known(ValueType::Struct),
    short = "Combine any number of structs into one containing all of their members",
    long = "On a name collision between two of the given structs, the later one's member is",
    long = "renamed by appending `_2`, `_3`, and so on (repeating until the generated name is",
    long = "unique) -- the same renaming `join`, `zip` and `group` already use when combining",
    long = "columns from more than one source, applied here to struct members instead.",
    long = "",
    long = "Only each struct's own local members are used, not any inherited from a parent",
    long = "(e.g. via `class`) -- the same \"data struct\" semantics `struct:of` itself uses.",
    example = "struct:join (struct:of a=1 b=2) (struct:of b=3 c=4)",
)]
#[allow(unused)]
struct Join {
    #[description("the structs to combine.")]
    #[unnamed]
    structs: Vec<Struct>,
}

fn join(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Join = Join::parse(context.remove_arguments(), &context.global_state.printer())?;
    let elements: Vec<(String, Value)> = cfg
        .structs
        .iter()
        .flat_map(|s| s.local_elements())
        .collect();
    let columns: Vec<ColumnType> = elements
        .iter()
        .map(|(name, value)| ColumnType::new_from_string(name.clone(), value.value_type()))
        .collect();
    let deduped = columns
        .as_slice()
        .deduplicate_names()
        .into_iter()
        .zip(elements)
        .map(|(column, (_, value))| (column.name().to_string(), value))
        .collect::<Vec<_>>();
    context
        .output
        .send(Value::Struct(Struct::new(deduped, None)))
}
