use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::Scope;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use crate::lang::command::OutputType::Known;
use signature::signature;
use crate::data::table::{ColumnType, Row};
use crate::lang::signature::number::Number;

#[signature(
    random.float,
    can_block = false,
    short = "generate a random floating point number between 0 (inclusive) and 1 (exclusive)",
    output = Known(ValueType::Float),
)]
struct Float {
    #[default(Number::Float(1.0))]
    #[description("upper bound (exclusive).")]
    to: Number,
}

fn float(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Float = Float::parse(context.remove_arguments(), &context.global_state.printer())?;
    context
        .output
        .send(Value::Float(rand::random::<f64>() * cfg.to.as_float()))?;
    Ok(())
}

static FLOAT_STREAM_OUTPUT_TYPE: [ColumnType; 1] = [ColumnType::new("value", ValueType::Float)];

static INTEGER_STREAM_OUTPUT_TYPE: [ColumnType; 1] = [ColumnType::new("value", ValueType::Integer)];

#[signature(
    random.float_stream,
    can_block = true,
    short = "generate a stream of random floating point numbers between 0 (inclusive) and 1 (exclusive)",
    output = Known(ValueType::table_input_stream(&FLOAT_STREAM_OUTPUT_TYPE)),
    example = "# Generate 20 floating point numbers between 0 and 100",
    example = "random.float_stream 100 | head 20",
)]
struct FloatStream {
    #[default(Number::Float(1.0))]
    #[description("upper bound (exclusive).")]
    to: Number,
}

fn float_stream(mut context: CommandContext) -> CrushResult<()> {
    let cfg = FloatStream::parse(context.remove_arguments(), &context.global_state.printer())?;
    let to = cfg.to.as_float();
    let output = context.output.initialize(&FLOAT_STREAM_OUTPUT_TYPE)?;
    loop {
        output
            .send(Row::new(vec![Value::Float(rand::random::<f64>() * to)]))?;
    }
}

#[signature(
    random.integer,
    can_block = false,
    short = "generate a random integer between 0 and 1 (or some other specified number)",
    output = Known(ValueType::Integer),
)]
struct Integer {
    #[default(2)]
    #[description("upper bound (exclusive).")]
    to: i128,
}

fn integer(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Integer::parse(context.remove_arguments(), &context.global_state.printer())?;
    let n = rand::random::<f64>() * (cfg.to as f64);
    context.output.send(Value::Integer(n as i128))?;
    Ok(())
}

#[signature(
    random.integer_stream,
    can_block = true,
    short = "generate a stream of random integer numbers between 0 (inclusive) and 2 (exclusive)",
    output = Known(ValueType::table_input_stream(&INTEGER_STREAM_OUTPUT_TYPE)),
    example = "# Generate 20 integers between 0 and 100",
    example = "random.integer_stream 100 | head 20",
)]
struct IntegerStream {
    #[default(2)]
    #[description("upper bound (exclusive).")]
    to: i128,
}

fn integer_stream(mut context: CommandContext) -> CrushResult<()> {
    let cfg = IntegerStream::parse(context.remove_arguments(), &context.global_state.printer())?;
    let output = context.output.initialize(&INTEGER_STREAM_OUTPUT_TYPE)?;
    loop {
        output
            .send(Row::new(vec![Value::Integer((rand::random::<f64>() * (cfg.to as f64)) as i128)]))?;
    }
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "random",
        "Random number generation",
        Box::new(move |env| {
            Float::declare(env)?;
            FloatStream::declare(env)?;
            Integer::declare(env)?;
            IntegerStream::declare(env)?;
            Ok(())
        }),
    )?;
    Ok(())
}

