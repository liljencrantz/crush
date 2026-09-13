use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::this::This;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use ordered_map::OrderedMap;
use signature::signature;
use std::sync::OnceLock;

pub fn methods() -> &'static OrderedMap<String, Command> {
    static CELL: OnceLock<OrderedMap<String, Command>> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();

        Is::declare_method(&mut res);
        IsNot::declare_method(&mut res);

        res
    })
}

#[signature(
    types.r#type.__is__,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "True if the needle's type is this type. Not meant to be called directly -- use the `like` command, the `=~` operator, or the `is` arm of a `match` block.",
)]
struct Is {
    #[description("the value whose type to check.")]
    needle: Value,
}

fn __is__(mut context: CommandContext) -> CrushResult<()> {
    let t = context.this.r#type()?;
    let cfg: Is = Is::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.output.send(Value::Bool(t.is(&cfg.needle)))
}

#[signature(
    types.r#type.__is_not__,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "False if the needle's type is this type. Not meant to be called directly -- use the `like` command or the `!~` operator.",
)]
struct IsNot {
    #[description("the value whose type to check.")]
    needle: Value,
}

fn __is_not__(mut context: CommandContext) -> CrushResult<()> {
    let t = context.this.r#type()?;
    let cfg: IsNot = IsNot::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.output.send(Value::Bool(!t.is(&cfg.needle)))
}
