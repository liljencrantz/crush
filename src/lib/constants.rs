use crate::lang::value::Value;
use crate::lang::scope::Scope;
use crate::lang::errors::CrushResult;

pub fn declare(root: &Scope) -> CrushResult<()> {
    let root_clone = root.clone();
    let e = root.create_lazy_namespace(
        "constants",
        Box::new(move |env: &Scope| {
            env.declare("true", Value::Bool(true))?;
            env.declare("false", Value::Bool(false))?;
            env.declare("global", Value::Scope(root_clone))?;
            env.readonly();
            Ok(())
        }))?;
    root.r#use(&e);
    Ok(())
}
