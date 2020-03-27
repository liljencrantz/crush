use crate::lang::value::Value;
use crate::lang::scope::Scope;
use crate::lang::errors::CrushResult;
use crate::util::file::home;

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("constants")?;
    root.r#use(&env);
    env.declare("true", Value::Bool(true))?;
    env.declare("false", Value::Bool(false))?;
    env.declare("global", Value::Scope(root.clone()))?;
    env.declare("home", Value::File(home()?))?;
    env.readonly();
    Ok(())
}
