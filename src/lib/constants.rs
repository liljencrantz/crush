use crate::lang::value::Value;
use crate::lang::scope::Scope;
use crate::lang::errors::CrushResult;
use std::path::Path;
use crate::util::file::home;

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("constants")?;
    root.r#use(&env);
    env.declare_str("true", Value::Bool(true))?;
    env.declare_str("false", Value::Bool(false))?;
    env.declare_str("global", Value::Scope(root.clone()))?;
    env.declare_str("root", Value::File(Box::from(Path::new("/"))))?;
    env.declare_str("home", Value::File(home()?))?;
    env.readonly();
    Ok(())
}
