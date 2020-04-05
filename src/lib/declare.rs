


macro_rules! declare {
    ($name:ident, $this_type:ident, $($input_type:ident, $output_type:ident, $operation:expr), *) => {
fn $name(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let this = context.this.$this_type()?;
    match (context.arguments.value(0)?) {
        $( Value::$input_type(v) => context.output.send(Value::$output_type($operation(this, v))), )*
        _ => return argument_error("Expected only arguments of the same type"),
    }
}
    }
}
