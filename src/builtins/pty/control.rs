use crate::lang::data::r#struct::Struct;
use crate::lang::value::Value;
use std::sync::OnceLock;

/// Named, one-character constants for control characters, so a script can write
/// `$t:write $t:control:z` (or `pty:control:z` at the module level) for Ctrl-Z instead of
/// a bare, hard-to-read `"\x1a"` literal. There is no separate "send a key" mechanism --
/// `write` takes these as perfectly ordinary text.
pub fn control_value() -> &'static Value {
    static CELL: OnceLock<Value> = OnceLock::new();
    CELL.get_or_init(|| {
        Value::Struct(Struct::new(
            vec![
                ("c", Value::from("\x03")), // Ctrl-C / SIGINT
                ("d", Value::from("\x04")), // Ctrl-D / EOF
                ("z", Value::from("\x1a")), // Ctrl-Z / SIGTSTP
            ],
            None,
        ))
    })
}
