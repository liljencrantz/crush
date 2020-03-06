use crate::lang::value::Value;
use crate::lang::scope::Scope;
use crate::lang::errors::CrushResult;

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("constants")?;
    root.r#use(&env);
    env.declare_str("true", Value::Bool(true))?;
    env.declare_str("false", Value::Bool(false))?;
    env.declare_str("global", Value::Scope(root.clone()))?;
    env.readonly();
    Ok(())
}
