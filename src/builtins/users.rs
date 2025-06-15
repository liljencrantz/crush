use crate::argument_error_legacy;
use crate::lang::any_str::AnyStr;
use crate::lang::command::OutputType::Known;
use crate::lang::command::OutputType::Unknown;
use crate::lang::command::{Command, CrushCommand};
use crate::lang::data::r#struct::Struct;
use crate::lang::errors::{CrushResult, error};
use crate::lang::serialization::{deserialize, serialize};
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::Scope;
use crate::lang::state::this::This;
use crate::lang::value::{Value, ValueType};
use crate::lang::{data::table::ColumnType, data::table::Row};
use crate::util::logins;
use crate::util::user_map::{get_all_users, get_current_username, get_user};
use signature::signature;
use std::io::{Read, Write};
use std::process;
use std::process::Stdio;
use std::sync::OnceLock;

#[signature(
    users.me,
    can_block = false,
    short = "current user",
    example = "# Show my own uid",
    example = "users:me:uid",
)]
struct Me {}

fn me(context: CommandContext) -> CrushResult<()> {
    context
        .output
        .send(get_user_value(&get_current_username()?)?)
}

static CURRENT_OUTPUT_TYPE: [ColumnType; 5] = [
    ColumnType::new("name", ValueType::String),
    ColumnType::new("tty", ValueType::String),
    ColumnType::new("host", ValueType::String),
    ColumnType::new("time", ValueType::Time),
    ColumnType::new("pid", ValueType::Integer),
];

#[signature(
    users.current,
    can_block = true,
    output = Known(ValueType::TableInputStream(CURRENT_OUTPUT_TYPE.to_vec())),
    short = "List all currently logged in users with their tty, login time, etc.",
    example = "# Find the username of the latest user to login in to this system",
    example = "$(users:current | sort time --reverse)[0] | member name",
)]
struct Current {}

fn current(context: CommandContext) -> CrushResult<()> {
    let output = context.output.initialize(&CURRENT_OUTPUT_TYPE)?;

    for l in logins::list()? {
        output.send(Row::new(vec![
            Value::from(l.user),
            Value::from(l.tty),
            Value::from(l.host.unwrap_or_else(|| "".to_string())),
            Value::Time(l.time),
            Value::from(l.pid),
        ]))?;
    }
    Ok(())
}

static LIST_OUTPUT_TYPE: [ColumnType; 6] = [
    ColumnType::new("name", ValueType::String),
    ColumnType::new("home", ValueType::File),
    ColumnType::new("shell", ValueType::File),
    ColumnType::new("information", ValueType::String),
    ColumnType::new("uid", ValueType::Integer),
    ColumnType::new("gid", ValueType::Integer),
];

pub fn user_struct() -> &'static Struct {
    static CELL: OnceLock<Struct> = OnceLock::new();
    CELL.get_or_init(|| {
        let do_cmd = <dyn CrushCommand>::command(
            r#do,
            true,
            &["global", "user"],
            "do command",
            "Run specified closure or command as another user",
            None::<AnyStr>,
            Unknown,
            [],
        );
        Struct::new(vec![("do", Value::Command(do_cmd))], None)
    })
}

#[signature(
    users.r#do,
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
fn r#do(mut context: CommandContext) -> CrushResult<()> {
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

        let mut child = cmd.spawn()?;
        let mut stdin = child.stdin.take().ok_or("Expected stdin stream")?;
        let mut serialized = Vec::new();
        serialize(&Value::Command(cfg.command), &mut serialized)?;

        context.spawn("sudo:stdin", move || {
            stdin.write(&serialized)?;
            Ok(())
        })?;

        let mut stdout = child.stdout.take().ok_or("Expected output stream")?;
        let env = context.scope.clone();
        let my_context = context.clone();
        context.spawn("sudo:stdout", move || {
            let mut buff = Vec::new();
            stdout.read_to_end(&mut buff)?;
            if buff.len() == 0 {
                error("No value returned")
            } else {
                my_context.output.send(deserialize(&buff, &env)?)
            }
        })?;

        let mut stderr = child.stderr.take().ok_or("Expected error stream")?;
        context.spawn("sudo:stderr", move || {
            let mut buff = Vec::new();
            stderr.read_to_end(&mut buff)?;
            let errors = String::from_utf8(buff)?;
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
    } else {
        argument_error_legacy("Invalid user")
    }
}

#[signature(
    users.list,
    can_block = true,
    output = Known(ValueType::table_input_stream(&LIST_OUTPUT_TYPE)),
    short = "List all users on the system",
)]
struct List {}

fn list(context: CommandContext) -> CrushResult<()> {
    let output = context.output.initialize(&LIST_OUTPUT_TYPE)?;
    for u in get_all_users()? {
        output.send(Row::new(vec![
            Value::from(u.name),
            Value::from(u.home),
            Value::from(u.shell),
            Value::from(u.information),
            Value::from(u.uid),
            Value::from(u.gid),
        ]))?;
    }
    Ok(())
}

#[signature(
    users.__getitem__,
    can_block = false,
    short = "find a user by name",
    example = "# Remove the file foo.txt as root",
    example = "users[root]:do {rm foo.txt}",
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
        Ok(user) => Ok(Value::Struct(Struct::new(
            vec![
                ("username", Value::from(input_name)),
                ("name", Value::from(user.name)),
                ("home", Value::from(user.home)),
                ("shell", Value::from(user.shell)),
                ("information", Value::from(user.information)),
                ("uid", Value::from(user.uid)),
                ("gid", Value::from(user.gid)),
            ],
            Some(user_struct().clone()),
        ))),
        Err(e) => Err(e),
    }
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "users",
        "User commands",
        Box::new(move |users| {
            Me::declare(users)?;
            Current::declare(users)?;
            List::declare(users)?;
            GetItem::declare(users)?;
            Ok(())
        }),
    )?;
    Ok(())
}
