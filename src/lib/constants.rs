use crate::data::Value;
use crate::namespace::Namespace;
use crate::errors::CrushResult;

pub fn declare(root: &Namespace) -> CrushResult<()> {
    let env = root.create_namespace("constants")?;
    root.uses(&env);
    root.declare_str("true", Value::Bool(true))?;
    root.declare_str("false", Value::Bool(false))?;
    root.declare_str("global", Value::Env(root.clone()))?;
    Ok(())
}
