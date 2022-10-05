use std::io::{Read, Write};
use std::process;
use std::process::Stdio;
use crate::lang::command::OutputType::Known;
use crate::lang::command::OutputType::Unknown;
use crate::lang::errors::{CrushResult, error, mandate};
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::Scope;
use crate::lang::data::r#struct::Struct;
use crate::lang::value::{Value, ValueType};
use signature::signature;
use crate::lang::{data::table::ColumnType, data::table::Row};
use lazy_static::lazy_static;
use crate::util::user_map::{get_all_users, get_current_username, get_user};
use crate::lang::command::{Command, CrushCommand};
use crate::lang::serialization::{deserialize, serialize};
use crate::lang::state::this::This;
use crate::{argument_error_legacy, to_crush_error};
use crate::util::logins;

#[signature(
me,
can_block = false,
short = "current user",
)]
struct Me {}

fn me(context: CommandContext) -> CrushResult<()> {
    context.output.send(get_user_value(&get_current_username()?)?)
}

lazy_static! {
    static ref CURRENT_OUTPUT_TYPE: Vec<ColumnType> = vec![
        ColumnType::new("name", ValueType::String),
        ColumnType::new("tty", ValueType::String),
        ColumnType::new("host", ValueType::String),
        ColumnType::new("time", ValueType::Time),
        ColumnType::new("pid", ValueType::Integer),
    ];
}

#[signature(
current,
can_block = true,
output = Known(ValueType::TableInputStream(CURRENT_OUTPUT_TYPE.clone())),
short = "Currently logged in users",
)]
struct Current {}

fn current(context: CommandContext) -> CrushResult<()> {
    let output = context.output.initialize(&CURRENT_OUTPUT_TYPE)?;

    for l in logins::list()? {
        output.send(Row::new(vec![
            Value::from(l.user),
            Value::from(l.tty),
            Value::Time(l.time),
            Value::from(l.pid),
            Value::from(l.host.unwrap_or_else(||{"".to_string()})),
        ]))?;
    }
    Ok(())
}

lazy_static! {
    static ref LIST_OUTPUT_TYPE: Vec<ColumnType> = vec![
        ColumnType::new("name", ValueType::String),
        ColumnType::new("home", ValueType::File),
        ColumnType::new("shell", ValueType::File),
        ColumnType::new("information", ValueType::String),
        ColumnType::new("uid", ValueType::Integer),
        ColumnType::new("gid", ValueType::Integer),
    ];
}

lazy_static! {
    pub static ref USER: Struct = {
        let do_cmd = <dyn CrushCommand>::command(
                sudo,
                true,
                &["global", "user"],
                "do command",
                "Run specified closure or command as another user",
                None,
                Unknown,
                [],
            );
            Struct::new(
                vec![
                    ("do", Value::Command(do_cmd)),
                ],
                None,
            )
    };
}

#[signature(
sudo,
can_block = true,
short = "Execute a lambda as another user.",
)]
pub struct Do {
    #[description("the command to run as another user.")]
    command: Command,
}

/**
Current implementation is crude and grossly inefficient.

Firstly, it just shells out to the sudo command - which leads to potential visual problems with
the terminal.

Secondly, it creates 3 separate subthreads just to deal with stdin, stdout and stderr without
blocking while the main thread waits for the command to exit. It is easy to do this much more
efficiently, but this was the most straight forward implementation and the sudo command should
never be run in a loop regardless.
 */
fn sudo(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Do = Do::parse(context.remove_arguments(), &context.global_state.printer())?;
    let this = context.this.r#struct()?;
    if let Some(Value::String(username)) = this.get("username") {
        let mut cmd = process::Command::new("sudo");
        let printer = context.global_state.printer().clone();

        cmd.arg("--user").arg(&username.to_string());
        cmd.arg("--").arg("crush").arg("--pup");
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = to_crush_error(cmd.spawn())?;
        let mut stdin = mandate(child.stdin.take(), "Expected stdin stream")?;
        let mut serialized = Vec::new();
        serialize(&Value::Command(cfg.command), &mut serialized)?;

        context.spawn("sudo:stdin", move || {
            stdin.write(&serialized)?;
            Ok(())
        })?;

        let mut stdout = mandate(child.stdout.take(), "Expected output stream")?;
        let env = context.scope.clone();
        let my_context = context.clone();
        context.spawn("sudo:stdout", move || {
            let mut buff = Vec::new();
            to_crush_error(stdout.read_to_end(&mut buff))?;
            if buff.len() == 0 {
                error("No value returned")
            } else {
                my_context
                    .output
                    .send(deserialize(&buff, &env)?)
            }
        })?;

        let mut stderr = mandate(child.stderr.take(), "Expected error stream")?;
        context.spawn("sudo:stderr", move || {
            let mut buff = Vec::new();
            to_crush_error(stderr.read_to_end(&mut buff))?;
            let errors = to_crush_error(String::from_utf8(buff))?;
            for e in errors.split('\n') {
                let err = e.trim();
                if !err.is_empty() {
                    printer.error(err);
                }
            }
            Ok(())
        })?;

        child.wait()?;
        Ok(())
    }   else {
        argument_error_legacy("Invalid user")
    }
}




#[signature(
list,
can_block = true,
output = Known(ValueType::TableInputStream(LIST_OUTPUT_TYPE.clone())),
short = "List all users on the system",
)]
struct List {}

fn list(context: CommandContext) -> CrushResult<()> {
    let output = context.output.initialize(&LIST_OUTPUT_TYPE)?;
    for u in get_all_users()? {
        output.send(Row::new(
            vec![
                Value::from(u.name),
                Value::from(u.home),
                Value::from(u.shell),
                Value::from(u.information),
                Value::Integer(u.uid as i128),
                Value::Integer(u.gid as i128),
            ]))?;
    }
    Ok(())
}

#[signature(
__getitem__,
can_block = false,
short = "find a user by name",
)]
struct GetItem {
    #[description("the name of the user to find.")]
    name: String,
}

fn __getitem__(mut context: CommandContext) -> CrushResult<()> {
    let cfg: GetItem = GetItem::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.output.send(get_user_value(&cfg.name)?)
}

fn get_user_value(input_name: &str) -> CrushResult<Value> {
    match get_user(input_name) {
        Ok(user) =>
            Ok(Value::Struct(Struct::new(
                vec![
                    ("username", Value::from(input_name)),
                    ("name", Value::from(user.name)),
                    ("home", Value::from(user.home)),
                    ("shell", Value::from(user.shell)),
                    ("information", Value::from(user.information)),
                    ("uid", Value::Integer(user.uid as i128)),
                    ("gid", Value::Integer(user.gid as i128)),
                ],
                Some(USER.clone()),
            ))),
        Err(e) => Err(e),
    }
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "user",
        "User commands",
        Box::new(move |user| {
            Me::declare(user)?;
            Current::declare(user)?;
            List::declare(user)?;
            GetItem::declare(user)?;
            Ok(())
        }),
    )?;
    Ok(())
}
