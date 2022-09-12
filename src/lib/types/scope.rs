use crate::lang::command::Command;
use crate::lang::command::OutputType::Unknown;
use crate::lang::command::TypeMap;
use crate::lang::errors::{mandate, CrushResult};
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::contexts::{ArgumentVector, This};
use lazy_static::lazy_static;
use ordered_map::OrderedMap;

fn full(name: &'static str) -> Vec<&'static str> {
    vec!["global", "types", "scope", name]
}

lazy_static! {
    pub static ref METHODS: OrderedMap<String, Command> = {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        res.declare(
            full("__getitem__"),
            getitem,
            false,
            "scope[name:string]",
            "Return the specified member",
            None,
            Unknown,
            [],
        );
        res
    };
}

fn getitem(mut context: CommandContext) -> CrushResult<()> {
    let val = context.this.scope()?;
    context.arguments.check_len(1)?;
    let name = context.arguments.string(0)?;
    context
        .output
        .send(mandate(val.get(name.as_ref())?, "Unknown member")?)
}
