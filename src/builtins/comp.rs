use crate::lang::command::OutputType::Known;
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::Scope;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use signature::signature;
use std::cmp::Ordering;

macro_rules! cmp {
    ($struct_name:ident, $name:ident, $op:expr) => {
        pub fn $name(mut context: CommandContext) -> CrushResult<()> {
            let cfg =
                $struct_name::parse(context.remove_arguments(), &context.global_state.printer())?;
            match cfg.left.partial_cmp(&cfg.right) {
                Some(ordering) => context.output.send(Value::Bool($op(ordering))),
                None => {
                    return command_error(format!(
                        "The two provided values of types {} and {} could not be compared",
                        cfg.left.value_type().to_string(),
                        cfg.right.value_type().to_string(),
                    ))
                }
            }
        }
    };
}

#[signature(
    comp.gt,
    can_block = false,
    short = "True if left side is greater than right side",
    long = "In expression mode, this method can be used via the the `>` operator.",
    example = "gt 10 5",
    example = "(10 > 5)",
    output = Known(ValueType::Bool),
)]
struct Gt {
    #[description("the left side of the comparison.")]
    left: Value,
    #[description("the right side of the comparison.")]
    right: Value,
}

#[signature(
    comp.lt,
    can_block = false,
    short = "True if left side is less than right side",
    long = "In expression mode, this method can be used via the the `<` operator.",
    example = "lt 10 5",
    example = "(10 < 5)",
    output = Known(ValueType::Bool),
)]
struct Lt {
    #[description("the left side of the comparison.")]
    left: Value,
    #[description("the right side of the comparison.")]
    right: Value,
}

#[signature(
    comp.gte,
    can_block = false,
    short = "True if left side is greater than or equal right side",
    long = "In expression mode, this method can be used via the the `>=` operator.",
    example = "gte 10 5",
    example = "(10 >+ 5)",
    output = Known(ValueType::Bool),
)]
struct Gte {
    #[description("the left side of the comparison.")]
    left: Value,
    #[description("the right side of the comparison.")]
    right: Value,
}

#[signature(
    comp.lte,
    can_block = false,
    short = "True if left side is less than or equal than right side",
    long = "In expression mode, this method can be used via the the `<=` operator.",
    example = "lte 10 5",
    example = "(10 <= 5)",
    output = Known(ValueType::Bool),
)]
struct Lte {
    #[description("the left side of the comparison.")]
    left: Value,
    #[description("the right side of the comparison.")]
    right: Value,
}

cmp!(Gt, gt, |o| o == Ordering::Greater);
cmp!(Lt, lt, |o| o == Ordering::Less);
cmp!(Gte, gte, |o| o != Ordering::Less);
cmp!(Lte, lte, |o| o != Ordering::Greater);

#[signature(
    comp.eq,
    can_block = false,
    short = "True if left side is equal to right side",
    long = "In expression mode, this method can be used via the the `==` operator.",
    example = "eq 10 5",
    example = "(10 == 5)",
    output = Known(ValueType::Bool),
)]
struct Eq {
    #[description("the left side of the comparison.")]
    left: Value,
    #[description("the right side of the comparison.")]
    right: Value,
}

pub fn eq(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Eq::parse(context.remove_arguments(), context.global_state.printer())?;
    context.output.send(Value::Bool(cfg.left.eq(&cfg.right)))
}

#[signature(
    comp.neq,
    can_block = false,
    short = "True if left side is not equal to right side",
    long = "In expression mode, this method can be used via the the `!=` operator.",
    example = "ne 10 5",
    example = "(10 != 5)",
    output = Known(ValueType::Bool),
)]
struct Neq {
    #[description("the left side of the comparison.")]
    left: Value,
    #[description("the right side of the comparison.")]
    right: Value,
}

pub fn neq(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Neq::parse(context.remove_arguments(), context.global_state.printer())?;
    context.output.send(Value::Bool(!cfg.left.eq(&cfg.right)))
}

#[signature(
    comp.not,
    can_block = false,
    short = "Negates the argument",
    long = "In expression mode, this method can be used via the the `!` operator.",
    example = "not $true",
    example = "(!$true)",
    output = Known(ValueType::Bool),
)]
struct Not {
    #[description("the value to negate.")]
    argument: bool,
}

pub fn not(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Not::parse(context.remove_arguments(), context.global_state.printer())?;
    context.output.send(Value::Bool(!cfg.argument))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let e = root.create_namespace(
        "comp",
        "Comparison operators",
        Box::new(|env| {
            env.declare("gt", Value::Command(Gt::create_command()))?;
            env.declare("gte", Value::Command(Gte::create_command()))?;
            env.declare("lt", Value::Command(Lt::create_command()))?;
            env.declare("lte", Value::Command(Lte::create_command()))?;
            env.declare("eq", Value::Command(Eq::create_command()))?;
            env.declare("neq", Value::Command(Neq::create_command()))?;
            env.declare("not", Value::Command(Not::create_command()))?;
            Ok(())
        }),
    )?;
    root.r#use(&e);
    Ok(())
}
