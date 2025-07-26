macro_rules! binary_op {
    ($name:ident, $this_type:ident, $($input_type:ident, $output_type:ident, $operation:expr), *) => {
fn $name(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let this = context.this.$this_type()?;
    match (context.arguments.value(0)?) {
        $( Value::$input_type(v) => context.output.send(Value::$output_type($operation(this, v))), )*
        other => return crate::lang::errors::command_error(format!(
            "Incompatible argument type for arithmetic operation: {}",
            other.value_type().to_string(),
            )),
    }
}
    }
}
