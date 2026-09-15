/*!
D-Bus support.

The system and session busses are exposed as lazily loaded namespaces, `dbus:system` and
`dbus:session`. Each bus mirrors the names of the services on it, split on `.`, so the service
`org.freedesktop.login1` is `dbus:system:org:freedesktop:login1`. A service namespace in turn
contains the service's object tree, split on `/`, so the object `/org/freedesktop/login1` of that
service is `dbus:system:org:freedesktop:login1:org:freedesktop:login1`. An object contains one
callable member per method and per property.

Everything is loaded one level at a time, the first time it is used, which is what makes tab
completion work without walking the whole bus up front: listing the services on a bus takes two
calls, `ListNames` and `ListActivatableNames`, and each object level is a single `Introspect` call.
 */
use crate::lang::command::OutputType::{Known, Unknown};
use crate::lang::command::{CrushCommand, Parameter};
use crate::lang::data::binary::FileReader;
use crate::lang::data::dict::Dict;
use crate::lang::data::list::List;
use crate::lang::data::r#struct::Struct;
use crate::lang::data::table::{ColumnType, Row};
use crate::lang::errors::{CrushResult, command_error, data_error, eof_error, error};
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::{Scope, ScopeLoader};
use crate::lang::state::this::This;
use crate::lang::value::{Value, ValueType};
use dbus::Message;
use dbus::arg::{ArgType, IterAppend};
use dbus::blocking::{BlockingSender, Connection, LocalConnection};
use dbus::message::MatchRule;
use dbus::strings::{BusName, Interface as InterfaceName, Member};
use signature::signature;
use std::cell::RefCell;
use std::rc::Rc;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

/// Timeout for the calls made while loading namespaces. These happen during tab completion, so a
/// hung service should not be able to freeze the prompt for long.
const INTROSPECTION_TIMEOUT: Duration = Duration::from_secs(1);
/// Timeout for method calls and property access. Same as the D-Bus default.
const CALL_TIMEOUT: Duration = Duration::from_secs(25);
/// How long a signal subscription waits for a signal before checking whether it has been stopped.
const SIGNAL_POLL_INTERVAL: Duration = Duration::from_millis(250);

thread_local! {
    /// One connection per bus, per thread. Opening a connection is a round trip of its own, and
    /// following a path like `dbus:system:org:freedesktop:login1:org:freedesktop:login1` makes one
    /// call per level.
    static CONNECTIONS: RefCell<HashMap<Bus, Connection>> = RefCell::new(HashMap::new());
}

/// Interfaces implemented by nearly every object. Their methods are not exposed as members, since
/// introspection is done automatically and properties are exposed directly. Their signals, like
/// PropertiesChanged, are.
const HIDDEN_INTERFACES: [&str; 3] = [
    "org.freedesktop.DBus.Introspectable",
    "org.freedesktop.DBus.Peer",
    "org.freedesktop.DBus.Properties",
];

/// The long help of the dbus namespace.
const DBUS_LONG_HELP: &str = r##"D-Bus is the message bus that Linux system services and desktop applications use to talk to each other. A program that offers functionality on the bus is a service, identified by a name like `org.freedesktop.login1`. A service has objects, identified by paths like `/org/freedesktop/login1`. Each object implements one or more interfaces, made up of methods that can be called, properties that can be read and sometimes written, and signals that are sent when something happens.

There are two busses. The system bus, `dbus:system`, is shared by the whole machine and hosts services like systemd, logind and NetworkManager. The session bus, `dbus:session`, belongs to a single login session and hosts desktop services like notifications and media players. Much of what `systemctl`, `loginctl`, `hostnamectl` and `timedatectl` do, they do by calling these services, and all of it is available directly from crush.

# Finding your way around

A service name is split on periods, so the service `org.freedesktop.login1` is `dbus:system:org:freedesktop:login1`. A service contains its objects, with paths split on slashes, so the object `/org/freedesktop/login1` of that service is `dbus:system:org:freedesktop:login1:org:freedesktop:login1`. An object contains one member per method, property and signal. Some services, like the bus itself, answer on the root object, in which case the methods are members of the service directly, e.g. `dbus:system:org:freedesktop:DBus:GetId`.

Every level can be tab completed. Levels are loaded the first time they are used, one D-Bus call per level. Services that are started on demand are listed even when they are not running, and using one starts it. Use `dir` to list the members of a level and `help` on a method to see its arguments and their types. Run `dbus:refresh` to see services and objects that appeared after a bus was first used.

Object paths can only contain letters, digits and underscores, so services escape other characters. systemd, for example, names the object for `dbus.service` `dbus_2eservice`.

# Calling methods

Methods are called like commands. Arguments can be passed by name or by position. Services that don't name their arguments get them called `arg0`, `arg1`, and so on. A bare word containing periods or slashes, like `dbus.service` or `Europe/Stockholm`, is parsed as a file, but is accepted wherever a string is expected. A method returns nothing, a single value, or a struct with one field per return value.

If more than one interface of an object has a method, property or signal with the same name, those members are prefixed with the last part of their interface name, e.g. `Manager_ListSessions`. The methods of the standard Introspectable, Peer and Properties interfaces are not listed.

D-Bus types map to crush types as follows:

* `y n q i u x t` are integers of different sizes. Values are range checked when sent.
* `d` is a float. Integers are accepted when sending.
* `b` is a bool.
* `s o g` are strings, object paths and type signatures, which are all strings.
* `a...` is an array, which is a list. Arrays of bytes also accept binary data.
* `a{...}` is a dict. A struct is accepted too, which is convenient for the common options argument of type `a{sv}`.
* `(...)` is a struct, which is received as a list, and can be sent as a list or a struct.
* `v` is a variant, which is received as the value it contains. When sending, the D-Bus type is picked from the crush value, e.g. integers are sent as `x`.
* `h` is a Unix file descriptor. It is sent by passing a file, which is opened for reading, and received as a `binary_stream` that reads from it. The stream owns the descriptor, so it stays open for as long as the value is kept.

# Properties

A property is read by calling it without arguments, and written by calling it with the new value as the only argument, e.g. `$player:Volume 0.5`.

# Signals

A signal is subscribed to by calling it. It returns a table stream with one row per signal received, with one column per signal argument. The subscription keeps running until it is stopped, e.g. with Ctrl-C, or until a signal arrives after whatever reads the stream has stopped reading, e.g. `head`.

# Permissions

Methods that change the state of the machine are usually guarded by polkit. They work when run as root, and otherwise fail with a permission error unless the policy allows them. Many such methods take an `interactive` argument. Passing `$true` lets polkit ask for a password through an authentication agent, which a plain terminal usually doesn't have, so pass `$false` there.

# Common services on the system bus

Which services exist depends on the machine. These are found on many Linux systems.

* `org.freedesktop.DBus` is the bus itself. List services, and find the process id and credentials of the connection that owns a name.
* `org.freedesktop.systemd1` is the systemd service manager. List, start, stop, restart and reload units, enable and disable unit files, and read every unit setting and runtime value, like its state, main process id, memory use and start time.
* `org.freedesktop.login1` is logind. List and inspect users, sessions and seats, lock and terminate sessions, power off, reboot, suspend and hibernate, and check the idle state.
* `org.freedesktop.hostname1` is hostnamed. Read and set the hostname, and read the operating system, kernel, chassis type and hardware vendor and model.
* `org.freedesktop.timedate1` is timedated. Read and set the time zone, the time and whether NTP is used, and list all time zones.
* `org.freedesktop.locale1` is localed. Read and set the system locale and the console and X11 keyboard layouts.
* `org.freedesktop.resolve1` is systemd-resolved. Resolve host names, addresses and DNS records, inspect DNS settings per network link, and flush caches.
* `org.freedesktop.network1` is systemd-networkd. List network links and their state, and reconfigure or renew them.
* `org.freedesktop.timesync1` is systemd-timesyncd. Read the NTP server in use and the synchronization state.
* `org.freedesktop.NetworkManager` is NetworkManager. List devices, connections and wireless access points, and activate and deactivate connections.
* `org.freedesktop.UPower` is UPower. Read battery charge, time to empty or full, and whether the machine runs on battery.
* `org.freedesktop.UDisks2` is UDisks. List drives, partitions and file systems, mount, unmount and format them, and read SMART data.
* `org.freedesktop.Accounts` is AccountsService. List user accounts and read and change their real names, icons and languages.
* `org.bluez` is BlueZ. List Bluetooth adapters and devices, scan, pair, connect and disconnect.
* `org.freedesktop.PackageKit` is PackageKit. Search for, install, remove and update packages.
* `org.freedesktop.fwupd` is fwupd. List devices with updatable firmware and install firmware updates.

# Common services on the session bus

These are found on most graphical desktops. Desktop environments add their own as well, e.g. `org.gnome.Shell` and `org.kde.KWin`.

* `org.freedesktop.Notifications` shows desktop notifications.
* `org.mpris.MediaPlayer2.*` are media players, e.g. `org.mpris.MediaPlayer2.spotify`. Play, pause, skip, seek, set the volume and read the metadata of what is playing.
* `org.freedesktop.portal.Desktop` is the desktop portal, used for screenshots, file choosers and reading desktop settings like the color scheme.
* `org.freedesktop.secrets` is the secret service of the desktop keyring, e.g. GNOME Keyring or KWallet.
* `org.freedesktop.ScreenSaver` locks the screen and reports whether it is active.

# Examples

The bus itself:

```
$bus := $dbus:system:org:freedesktop:DBus
# The unique id of this machine's bus
$bus:GetId
# Is a service running?
$bus:NameHasOwner org.freedesktop.login1
# The process id of a service
$bus:GetConnectionUnixProcessID org.freedesktop.login1
# Wait for the next time a service or connection appears or disappears
$bus:NameOwnerChanged | head 1
```

systemd, the service manager:

```
$systemd := $dbus:system:org:freedesktop:systemd1:org:freedesktop:systemd1
$systemd:Version

# All active socket units. Each unit is a list of its name, description, load
# state, active state, sub state, following unit, object path, job id, job type
# and job object path.
$systemd:ListUnitsByPatterns states=$(list:of active) patterns=$(list:of "*.socket")

# The names of all running services
for unit=$($systemd:ListUnitsByPatterns $(list:of running) $(list:of "*.service")) {
    echo $unit[0]
}

# The object path of a unit
$systemd:GetUnit dbus.service

# Properties of a unit
$unit := $dbus:system:org:freedesktop:systemd1:org:freedesktop:systemd1:unit:dbus_2eservice
$unit:ActiveState
$unit:MainPID
$unit:MemoryCurrent
$unit:Description

# Restart and stop units. Requires root.
$systemd:RestartUnit name=ssh.service mode=replace
$systemd:StopUnit name=ssh.service mode=replace
```

logind, users, sessions and power:

```
$login := $dbus:system:org:freedesktop:login1:org:freedesktop:login1

# Logged in users, each a list of user id, name and object path
$login:ListUsers
for user=$($login:ListUsers) {
    echo $user[1]
}

# The session this shell runs in
$login:session:self:Name
$login:session:self:Type
$login:user:self:Name

# Is the machine idle?
$login:IdleHint

# Suspend or power off the machine
$login:Suspend interactive=$false
$login:PowerOff interactive=$false

# Delay suspending while a backup runs. The lock is a file descriptor, and is
# held until the value is gone.
$lock := $($login:Inhibit what=sleep who=crush why="Backup in progress" mode=delay)
$login:ListInhibitors
var:unset lock

# Show sessions as they are opened, and wait for the machine to suspend
$login:SessionNew
$login:PrepareForSleep | head 1
```

The host name, time and locale:

```
$host := $dbus:system:org:freedesktop:hostname1:org:freedesktop:hostname1
$host:Hostname
$host:OperatingSystemPrettyName
$host:KernelRelease
$host:Chassis
$host:SetStaticHostname hostname=build-server interactive=$false

$time := $dbus:system:org:freedesktop:timedate1:org:freedesktop:timedate1
$time:Timezone
$time:NTP
$time:SetTimezone timezone=Europe/Stockholm interactive=$false

$dbus:system:org:freedesktop:locale1:org:freedesktop:locale1:Locale
```

Networking and name resolution:

```
$network := $dbus:system:org:freedesktop:network1:org:freedesktop:network1
# Each link is a list of its index, name and object path
$network:ListLinks
$network:OperationalState

# Resolve a host name on any interface (0), for any address family (0), with no
# flags (0)
$dbus:system:org:freedesktop:resolve1:org:freedesktop:resolve1:ResolveHostname 0 example.com 0 0
```

The session bus, on a desktop:

```
# Show a notification. The arguments are the application name, the id of a
# notification to replace (0 for none), an icon, the summary, the body, a list of
# actions, a dict of hints, and a timeout in milliseconds (-1 for the default).
$notifications := $dbus:session:org:freedesktop:Notifications:org:freedesktop:Notifications
$notifications:Notify crush 0 "" "Build finished" "All tests passed" $($(list $string):new) $(struct:of) (-1)

# Control a media player
$player := $dbus:session:org:mpris:MediaPlayer2:spotify:org:mpris:MediaPlayer2
$player:PlayPause
$player:Next
$player:PlaybackStatus
$player:Metadata
$player:Volume 0.5
```
"##;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Bus {
    System,
    Session,
}

impl Bus {
    fn name(&self) -> &'static str {
        match self {
            Bus::System => "system",
            Bus::Session => "session",
        }
    }

    fn from_name(name: &str) -> CrushResult<Bus> {
        match name {
            "system" => Ok(Bus::System),
            "session" => Ok(Bus::Session),
            _ => data_error(format!("Unknown D-Bus bus `{}`", name)),
        }
    }

    fn connect(&self) -> CrushResult<Connection> {
        Ok(match self {
            Bus::System => Connection::new_system()?,
            Bus::Session => Connection::new_session()?,
        })
    }

    /// A new connection, not shared with anything else. Signal subscriptions use one each, so that
    /// their match rules go away with them.
    fn connect_local(&self) -> CrushResult<LocalConnection> {
        Ok(match self {
            Bus::System => LocalConnection::new_system()?,
            Bus::Session => LocalConnection::new_session()?,
        })
    }

    fn call(&self, message: Message, timeout: Duration) -> CrushResult<Message> {
        CONNECTIONS.with(|connections| {
            let mut connections = connections.borrow_mut();
            let connection = match connections.entry(*self) {
                std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
                std::collections::hash_map::Entry::Vacant(e) => e.insert(self.connect()?),
            };
            let res = connection.send_with_reply_and_block(message, timeout);
            // Only a broken connection is thrown away, e.g. because the bus was restarted. The
            // call itself is not retried, since it may not be safe to run a method twice.
            if !connection.channel().is_connected() {
                connections.remove(self);
            }
            Ok(res?)
        })
    }

    fn list_services(&self) -> CrushResult<Vec<String>> {
        // Services that are started on demand, like hostname1 and timedate1 on most systemd
        // machines, usually aren't running, so the activatable names are included too. Using such
        // a service starts it.
        let mut names = HashSet::new();
        for method in ["ListNames", "ListActivatableNames"] {
            let message = Message::new_method_call(
                "org.freedesktop.DBus",
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
                method,
            )?;
            let reply = self.call(message, INTROSPECTION_TIMEOUT)?;
            let list: Vec<String> = reply
                .get1()
                .ok_or_else(|| format!("Invalid reply to {}", method))?;
            // Unique connection names (":1.42") are not services anyone would want to browse.
            names.extend(list.into_iter().filter(|n| !n.starts_with(':')));
        }
        let mut names = names.into_iter().collect::<Vec<_>>();
        names.sort();
        Ok(names)
    }

    fn introspect(&self, service: &str, path: &str) -> CrushResult<IntrospectedNode> {
        let message = Message::new_method_call(
            service,
            path,
            "org.freedesktop.DBus.Introspectable",
            "Introspect",
        )?;
        let reply = self.call(message, INTROSPECTION_TIMEOUT)?;
        let xml: String = reply.get1().ok_or("Invalid reply to Introspect")?;
        parse_introspection(&xml)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    In,
    Out,
}

#[derive(Debug, Clone)]
struct MethodArgument {
    name: Option<String>,
    signature: String,
    direction: Direction,
}

#[derive(Debug, Clone)]
struct Method {
    name: String,
    arguments: Vec<MethodArgument>,
}

#[derive(Debug, Clone)]
struct Property {
    name: String,
    signature: String,
    access: String,
}

#[derive(Debug, Clone)]
struct Interface {
    name: String,
    methods: Vec<Method>,
    properties: Vec<Property>,
    /// Signals are described like methods whose arguments are all outputs.
    signals: Vec<Method>,
}

#[derive(Debug)]
struct IntrospectedNode {
    children: Vec<String>,
    interfaces: Vec<Interface>,
}

fn required_attribute<'a>(node: &roxmltree::Node<'a, '_>, name: &str) -> CrushResult<&'a str> {
    node.attribute(name).ok_or_else(|| {
        format!(
            "Invalid D-Bus introspection data: `{}` element without a `{}` attribute",
            node.tag_name().name(),
            name
        )
        .into()
    })
}

fn parse_introspection(xml: &str) -> CrushResult<IntrospectedNode> {
    // D-Bus introspection data starts with a DOCTYPE declaration, which roxmltree rejects
    // unless DTD parsing is explicitly allowed.
    let doc = roxmltree::Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    )?;
    let root = doc.root_element();
    if root.tag_name().name() != "node" {
        return data_error("Invalid D-Bus introspection data: root element is not a node");
    }

    let mut children = Vec::new();
    let mut interfaces = Vec::new();
    for child in root.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "node" => children.push(required_attribute(&child, "name")?.to_string()),
            "interface" => {
                let mut methods = Vec::new();
                let mut properties = Vec::new();
                let mut signals = Vec::new();
                for member in child.children().filter(|n| n.is_element()) {
                    match member.tag_name().name() {
                        "method" => {
                            let mut arguments = Vec::new();
                            for arg in member
                                .children()
                                .filter(|n| n.is_element() && n.tag_name().name() == "arg")
                            {
                                // The direction of method arguments defaults to "in".
                                let direction = match arg.attribute("direction") {
                                    None | Some("in") => Direction::In,
                                    Some("out") => Direction::Out,
                                    Some(d) => {
                                        return data_error(format!(
                                            "Invalid D-Bus argument direction `{}`",
                                            d
                                        ));
                                    }
                                };
                                arguments.push(MethodArgument {
                                    name: arg.attribute("name").map(|s| s.to_string()),
                                    signature: complete_type(required_attribute(&arg, "type")?)?,
                                    direction,
                                });
                            }
                            methods.push(Method {
                                name: required_attribute(&member, "name")?.to_string(),
                                arguments,
                            });
                        }
                        "signal" => {
                            let mut arguments = Vec::new();
                            for arg in member
                                .children()
                                .filter(|n| n.is_element() && n.tag_name().name() == "arg")
                            {
                                arguments.push(MethodArgument {
                                    name: arg.attribute("name").map(|s| s.to_string()),
                                    signature: complete_type(required_attribute(&arg, "type")?)?,
                                    direction: Direction::Out,
                                });
                            }
                            signals.push(Method {
                                name: required_attribute(&member, "name")?.to_string(),
                                arguments,
                            });
                        }
                        "property" => properties.push(Property {
                            name: required_attribute(&member, "name")?.to_string(),
                            signature: complete_type(required_attribute(&member, "type")?)?,
                            access: required_attribute(&member, "access")?.to_string(),
                        }),
                        _ => {}
                    }
                }
                interfaces.push(Interface {
                    name: required_attribute(&child, "name")?.to_string(),
                    methods,
                    properties,
                    signals,
                });
            }
            _ => {}
        }
    }
    Ok(IntrospectedNode {
        children,
        interfaces,
    })
}

/// Returns the length, in bytes, of the single complete type starting at `start` in `signature`.
fn complete_type_length(signature: &[u8], start: usize) -> CrushResult<usize> {
    let invalid = || {
        data_error(format!(
            "Invalid D-Bus signature `{}`",
            String::from_utf8_lossy(signature)
        ))
    };
    match signature.get(start) {
        Some(
            b'y' | b'b' | b'n' | b'q' | b'i' | b'u' | b'x' | b't' | b'd' | b'h' | b's' | b'o'
            | b'g' | b'v',
        ) => Ok(1),
        Some(b'a') => Ok(1 + complete_type_length(signature, start + 1)?),
        Some(open @ (b'(' | b'{')) => {
            let close = if *open == b'(' { b')' } else { b'}' };
            let mut idx = start + 1;
            loop {
                match signature.get(idx) {
                    None => return invalid(),
                    Some(c) if *c == close => return Ok(idx + 1 - start),
                    Some(_) => idx += complete_type_length(signature, idx)?,
                }
            }
        }
        _ => invalid(),
    }
}

/// Split a signature into its complete types, e.g. `sa{sv}(ii)` into `s`, `a{sv}` and `(ii)`.
fn split_signature(signature: &str) -> CrushResult<Vec<&str>> {
    // The D-Bus specification limits signatures to 255 bytes.
    if signature.len() > 255 {
        return data_error("Invalid D-Bus signature: longer than 255 bytes");
    }
    let bytes = signature.as_bytes();
    let mut res = Vec::new();
    let mut idx = 0;
    while idx < bytes.len() {
        let len = complete_type_length(bytes, idx)?;
        res.push(&signature[idx..idx + len]);
        idx += len;
    }
    Ok(res)
}

/// The key and value types of a dict signature, e.g. `sv` for `a{sv}`.
fn dict_entry_types(signature: &str) -> CrushResult<&str> {
    match signature.strip_prefix("a{").and_then(|s| s.strip_suffix('}')) {
        Some(types) => Ok(types),
        None => data_error(format!("Invalid D-Bus dict signature `{}`", signature)),
    }
}

/// The field types of a struct signature, e.g. `is` for `(is)`.
fn struct_field_types(signature: &str) -> CrushResult<&str> {
    match signature.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
        Some(types) => Ok(types),
        None => data_error(format!("Invalid D-Bus struct signature `{}`", signature)),
    }
}

/// Check that a signature from introspection data is a single complete type, so that malformed
/// data from a service is reported when the object is loaded rather than when it is used.
fn complete_type(signature: &str) -> CrushResult<String> {
    match split_signature(signature)?.as_slice() {
        [_] => Ok(signature.to_string()),
        _ => data_error(format!(
            "Invalid D-Bus introspection data: `{}` is not a single complete type",
            signature
        )),
    }
}

fn value_type_for_signature(signature: &str) -> ValueType {
    match signature.as_bytes().first() {
        Some(b'y' | b'n' | b'q' | b'i' | b'u' | b'x' | b't') => ValueType::Integer,
        Some(b'b') => ValueType::Bool,
        Some(b'd') => ValueType::Float,
        Some(b's' | b'o' | b'g') => ValueType::String,
        Some(b'h') => ValueType::BinaryInputStream,
        Some(b'a') if signature == "ay" => ValueType::Binary,
        Some(b'a') if signature.starts_with("a{") => {
            match dict_entry_types(signature)
                .and_then(split_signature)
                .as_deref()
            {
                Ok([key, value]) => ValueType::Dict(
                    Box::from(value_type_for_signature(key)),
                    Box::from(value_type_for_signature(value)),
                ),
                _ => ValueType::Any,
            }
        }
        Some(b'a') => ValueType::List(Box::from(value_type_for_signature(&signature[1..]))),
        Some(b'(') => ValueType::List(Box::from(ValueType::Any)),
        _ => ValueType::Any,
    }
}

/// The column type used for a signal argument. Containers get `any`, since the element types of a
/// received list or dict depend on its contents.
fn column_type_for_signature(signature: &str) -> ValueType {
    match value_type_for_signature(signature) {
        ValueType::List(_) | ValueType::Dict(_, _) => ValueType::Any,
        t => t,
    }
}

/// Pick a D-Bus signature for a crush value. Used for variants, where the method signature does
/// not say what type to send.
fn signature_for_value(value: &Value) -> CrushResult<String> {
    Ok(match value {
        Value::String(_) | Value::File(_) => "s".to_string(),
        Value::Integer(_) => "x".to_string(),
        Value::Float(_) => "d".to_string(),
        Value::Bool(_) => "b".to_string(),
        Value::Binary(_) => "ay".to_string(),
        Value::List(list) => match list.iter().next() {
            Some(first) if list.element_type() != ValueType::Any => {
                format!("a{}", signature_for_value(&first)?)
            }
            _ => "av".to_string(),
        },
        Value::Dict(dict) => match dict.elements().first() {
            Some((key, value)) if dict.value_type() != ValueType::Any => format!(
                "a{{{}{}}}",
                signature_for_value(key)?,
                signature_for_value(value)?
            ),
            Some((key, _)) => format!("a{{{}v}}", signature_for_value(key)?),
            None => "a{sv}".to_string(),
        },
        Value::Struct(s) => {
            let mut res = "(".to_string();
            for (_, v) in s.local_elements() {
                res.push_str(&signature_for_value(&v)?);
            }
            res.push(')');
            res
        }
        _ => {
            return command_error(format!(
                "Can't send a value of type `{}` over D-Bus",
                value.value_type()
            ));
        }
    })
}

fn type_error<T>(expected: &str, value: &Value) -> CrushResult<T> {
    command_error(format!(
        "Expected {}, got a value of type `{}`",
        expected,
        value.value_type()
    ))
}

fn integer<T: TryFrom<i128>>(value: &Value) -> CrushResult<T> {
    match value {
        Value::Integer(i) => T::try_from(*i).or_else(|_| {
            command_error(format!(
                "The integer {} is out of range for this D-Bus argument",
                i
            ))
        }),
        v => type_error("an integer", v),
    }
}

fn string(value: &Value) -> CrushResult<String> {
    match value {
        Value::String(s) => Ok(s.to_string()),
        // Bare words containing a period or a slash, like unit names or object paths, are
        // parsed as files, so accept those as strings as well.
        Value::File(f) => Ok(f.to_str().ok_or("Invalid file name")?.to_string()),
        v => type_error("a string", v),
    }
}

/// Run an append closure that can fail. The dbus crate's container append functions take closures
/// that can't return errors, so the error is smuggled out through a variable instead.
fn fallible<'a>(
    f: impl FnOnce(&mut dyn FnMut(&mut IterAppend<'a>)),
    mut body: impl FnMut(&mut IterAppend<'a>) -> CrushResult<()>,
) -> CrushResult<()> {
    let mut res = Ok(());
    f(&mut |iter| res = body(iter));
    res
}

/// Append `value` to a message as the single complete D-Bus type `signature`.
fn encode(iter: &mut IterAppend, signature: &str, value: Value) -> CrushResult<()> {
    match signature.as_bytes().first() {
        Some(b'y') => iter.append(integer::<u8>(&value)?),
        Some(b'n') => iter.append(integer::<i16>(&value)?),
        Some(b'q') => iter.append(integer::<u16>(&value)?),
        Some(b'i') => iter.append(integer::<i32>(&value)?),
        Some(b'u') => iter.append(integer::<u32>(&value)?),
        Some(b'x') => iter.append(integer::<i64>(&value)?),
        Some(b't') => iter.append(integer::<u64>(&value)?),
        Some(b'b') => match value {
            Value::Bool(b) => iter.append(b),
            v => return type_error("a boolean", &v),
        },
        Some(b'd') => match value {
            Value::Float(f) => iter.append(f),
            Value::Integer(i) => iter.append(i as f64),
            v => return type_error("a number", &v),
        },
        Some(b's') => iter.append(string(&value)?),
        Some(b'o') => iter.append(dbus::Path::new(string(&value)?)?),
        Some(b'g') => iter.append(dbus::Signature::new(string(&value)?)?),
        Some(b'v') => {
            let inner = signature_for_value(&value)?;
            let inner_signature = dbus::Signature::new(inner.clone())?;
            let mut value = Some(value);
            fallible(
                |f| iter.append_variant(&inner_signature, |i| f(i)),
                |i| encode(i, &inner, value.take().ok_or("Value already consumed")?),
            )?;
        }
        Some(b'a') if signature.starts_with("a{") => {
            let types = split_signature(dict_entry_types(signature)?)?;
            let [key_type, value_type] = types.as_slice() else {
                return data_error(format!("Invalid D-Bus dict signature `{}`", signature));
            };
            let entries = match value {
                Value::Dict(d) => d.elements(),
                // Allow e.g. `struct:of foo=1` for the common `a{sv}` options argument.
                Value::Struct(s) => s
                    .local_elements()
                    .into_iter()
                    .map(|(k, v)| (Value::from(k), v))
                    .collect(),
                v => return type_error("a dict", &v),
            };
            let key_signature = dbus::Signature::new(key_type.to_string())?;
            let value_signature = dbus::Signature::new(value_type.to_string())?;
            let mut entries = Some(entries);
            fallible(
                |f| iter.append_dict(&key_signature, &value_signature, |i| f(i)),
                |i| {
                    for (k, v) in entries.take().ok_or("Value already consumed")? {
                        let mut entry = Some((k, v));
                        fallible(
                            |f| i.append_dict_entry(|e| f(e)),
                            |e| {
                                let (k, v) = entry.take().ok_or("Value already consumed")?;
                                encode(e, key_type, k)?;
                                encode(e, value_type, v)
                            },
                        )?;
                    }
                    Ok(())
                },
            )?;
        }
        Some(b'a') => {
            let element_type = &signature[1..];
            let elements: Vec<Value> = match value {
                Value::List(l) => l.iter().collect(),
                Value::Binary(b) if element_type == "y" => {
                    iter.append(b.to_vec());
                    return Ok(());
                }
                v => return type_error("a list", &v),
            };
            let element_signature = dbus::Signature::new(element_type.to_string())?;
            let mut elements = Some(elements);
            fallible(
                |f| iter.append_array(&element_signature, |i| f(i)),
                |i| {
                    for element in elements.take().ok_or("Value already consumed")? {
                        encode(i, element_type, element)?;
                    }
                    Ok(())
                },
            )?;
        }
        Some(b'(') => {
            let field_types = split_signature(struct_field_types(signature)?)?;
            let fields: Vec<Value> = match value {
                Value::List(l) => l.iter().collect(),
                Value::Struct(s) => s.local_elements().into_iter().map(|(_, v)| v).collect(),
                v => return type_error("a list or a struct", &v),
            };
            if fields.len() != field_types.len() {
                return command_error(format!(
                    "Expected {} fields for the D-Bus struct `{}`, got {}",
                    field_types.len(),
                    signature,
                    fields.len()
                ));
            }
            let mut fields = Some(fields);
            fallible(
                |f| iter.append_struct(|i| f(i)),
                |i| {
                    for (field_type, field) in field_types
                        .iter()
                        .zip(fields.take().ok_or("Value already consumed")?)
                    {
                        encode(i, field_type, field)?;
                    }
                    Ok(())
                },
            )?;
        }
        // The descriptor is duplicated when it is appended, so the file is closed again right away.
        Some(b'h') => match value {
            Value::File(path) => iter.append(std::fs::File::open(&*path)?),
            v => return type_error("a file", &v),
        },
        _ => return data_error(format!("Invalid D-Bus signature `{}`", signature)),
    }
    Ok(())
}

fn collect_values(iter: &mut dbus::arg::Iter) -> CrushResult<Vec<Value>> {
    let mut res = Vec::new();
    loop {
        match decode(iter) {
            Ok(value) => {
                res.push(value);
                iter.next();
            }
            Err(e) if e.is_eof() => return Ok(res),
            Err(e) => return Err(e),
        }
    }
}

fn common_type(types: &HashSet<ValueType>) -> ValueType {
    if types.len() == 1 {
        types.iter().next().unwrap().clone()
    } else {
        ValueType::Any
    }
}

/// Read the value at the current position of a message.
fn decode(iter: &mut dbus::arg::Iter) -> CrushResult<Value> {
    let unexpected = "Unexpected type in D-Bus message";
    Ok(match iter.arg_type() {
        ArgType::String => Value::from(iter.get::<String>().ok_or(unexpected)?),
        ArgType::Boolean => Value::from(iter.get::<bool>().ok_or(unexpected)?),
        ArgType::Byte => Value::Integer(iter.get::<u8>().ok_or(unexpected)? as i128),
        ArgType::Int16 => Value::Integer(iter.get::<i16>().ok_or(unexpected)? as i128),
        ArgType::UInt16 => Value::Integer(iter.get::<u16>().ok_or(unexpected)? as i128),
        ArgType::Int32 => Value::Integer(iter.get::<i32>().ok_or(unexpected)? as i128),
        ArgType::UInt32 => Value::Integer(iter.get::<u32>().ok_or(unexpected)? as i128),
        ArgType::Int64 => Value::Integer(iter.get::<i64>().ok_or(unexpected)? as i128),
        ArgType::UInt64 => Value::Integer(iter.get::<u64>().ok_or(unexpected)? as i128),
        ArgType::Double => Value::Float(iter.get::<f64>().ok_or(unexpected)?),
        ArgType::ObjectPath => {
            Value::from(iter.get::<dbus::Path>().ok_or(unexpected)?.to_string())
        }
        ArgType::Signature => {
            Value::from(iter.get::<dbus::Signature>().ok_or(unexpected)?.to_string())
        }
        ArgType::Array if &*iter.signature() == "ay" => {
            Value::Binary(Arc::from(iter.get::<Vec<u8>>().ok_or(unexpected)?))
        }
        ArgType::Array => {
            let mut sub = iter.recurse(ArgType::Array).ok_or(unexpected)?;
            if sub.arg_type() == ArgType::DictEntry {
                let mut entries = Vec::new();
                let mut key_types = HashSet::new();
                let mut value_types = HashSet::new();
                while let Some(mut entry) = sub.recurse(ArgType::DictEntry) {
                    let key = decode(&mut entry)?;
                    entry.next();
                    let value = decode(&mut entry)?;
                    key_types.insert(key.value_type());
                    value_types.insert(value.value_type());
                    entries.push((key, value));
                    sub.next();
                }
                let dict = Dict::new(common_type(&key_types), common_type(&value_types))?;
                for (key, value) in entries {
                    dict.insert(key, value)?;
                }
                Value::Dict(dict)
            } else {
                let values = collect_values(&mut sub)?;
                let types = values.iter().map(|v| v.value_type()).collect();
                List::new(common_type(&types), values).into()
            }
        }
        ArgType::Variant => decode(&mut iter.recurse(ArgType::Variant).ok_or(unexpected)?)?,
        ArgType::Struct => {
            let values = collect_values(&mut iter.recurse(ArgType::Struct).ok_or(unexpected)?)?;
            List::new(ValueType::Any, values).into()
        }
        ArgType::DictEntry => return data_error("Invalid location for a D-Bus dict entry"),
        // A file descriptor is received as a binary stream reading from it. The stream owns the
        // descriptor, so e.g. an inhibitor lock from logind is held for as long as the value is.
        ArgType::UnixFd => Value::BinaryInputStream(Box::from(FileReader::new(
            iter.get::<std::fs::File>().ok_or(unexpected)?,
        ))),
        ArgType::Invalid => return eof_error(),
    })
}

fn string_field(s: &Struct, name: &str) -> CrushResult<String> {
    match s.get(name) {
        Some(Value::String(v)) => Ok(v.to_string()),
        _ => error(format!("Missing or invalid `{}` field", name)),
    }
}

fn string_list_field(s: &Struct, name: &str) -> CrushResult<Vec<String>> {
    match s.get(name) {
        Some(Value::List(l)) => l
            .iter()
            .map(|v| match v {
                Value::String(s) => Ok(s.to_string()),
                _ => error(format!("Invalid element in `{}` field", name)),
            })
            .collect(),
        _ => error(format!("Missing or invalid `{}` field", name)),
    }
}

fn string_list(values: impl IntoIterator<Item = String>) -> Value {
    List::new(
        ValueType::String,
        values.into_iter().map(Value::from).collect::<Vec<_>>(),
    )
    .into()
}

/// The part of a D-Bus interface name after the last period, e.g. `Manager` for
/// `org.freedesktop.login1.Manager`.
fn short_interface_name(interface: &str) -> &str {
    interface.rsplit('.').next().unwrap_or(interface)
}

fn argument_names(arguments: &[&MethodArgument]) -> Vec<String> {
    arguments
        .iter()
        .enumerate()
        .map(|(idx, a)| a.name.clone().unwrap_or_else(|| format!("arg{}", idx)))
        .collect()
}

fn method_value(
    bus: Bus,
    service: &str,
    path: &str,
    interface: &str,
    member_name: &str,
    method: &Method,
) -> Value {
    let inputs = method
        .arguments
        .iter()
        .filter(|a| a.direction == Direction::In)
        .collect::<Vec<_>>();
    let outputs = method
        .arguments
        .iter()
        .filter(|a| a.direction == Direction::Out)
        .collect::<Vec<_>>();
    let input_names = argument_names(&inputs);
    let output_names = argument_names(&outputs);

    let signature = format!(
        "{} {}",
        member_name,
        input_names
            .iter()
            .zip(&inputs)
            .map(|(name, a)| format!("{}={}", name, value_type_for_signature(&a.signature)))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let mut long_help = format!(
        "Calls the `{}` method of the `{}` interface on the D-Bus object `{}` of the service `{}`.\n\n\
         Arguments can be passed by name or by position.",
        method.name, interface, path, service
    );
    if !inputs.is_empty() {
        long_help.push_str("\n\nThis method accepts the following arguments:\n\n");
        for (name, a) in input_names.iter().zip(&inputs) {
            long_help.push_str(&format!("* `{}` (D-Bus type `{}`)\n", name, a.signature));
        }
    }
    match outputs.len() {
        0 => long_help.push_str("\n\nThis method returns nothing."),
        1 => long_help.push_str(&format!(
            "\n\nThis method returns a single value of D-Bus type `{}`.",
            outputs[0].signature
        )),
        _ => {
            long_help.push_str("\n\nThis method returns a struct with the following fields:\n\n");
            for (name, a) in output_names.iter().zip(&outputs) {
                long_help.push_str(&format!("* `{}` (D-Bus type `{}`)\n", name, a.signature));
            }
        }
    }

    let parameters = input_names
        .iter()
        .zip(&inputs)
        .map(|(name, a)| Parameter {
            name: name.clone(),
            value_type: value_type_for_signature(&a.signature),
            default: None,
            allowed: None,
            description: Some(format!("D-Bus type `{}`", a.signature)),
            complete: None,
            named: false,
            unnamed: false,
            dirs_only: false,
        })
        .collect::<Vec<_>>();

    Value::Struct(Struct::new(
        vec![
            ("bus", Value::from(bus.name())),
            ("service", Value::from(service)),
            ("path", Value::from(path)),
            ("interface", Value::from(interface)),
            ("method", Value::from(method.name.as_str())),
            ("input_names", string_list(input_names)),
            (
                "input_signatures",
                string_list(inputs.iter().map(|a| a.signature.clone())),
            ),
            ("output_names", string_list(output_names)),
            (
                "__call__",
                Value::Command(<dyn CrushCommand>::command(
                    call_method,
                    true,
                    ["global", "dbus", "__method__"],
                    signature,
                    format!("Call the D-Bus method {}.{}", interface, method.name),
                    Some(long_help),
                    Unknown,
                    parameters,
                )),
            ),
        ],
        None,
    ))
}

fn call_method(mut context: CommandContext) -> CrushResult<()> {
    let this = context.this.r#struct()?;
    let bus = Bus::from_name(&string_field(&this, "bus")?)?;
    let service = string_field(&this, "service")?;
    let path = string_field(&this, "path")?;
    let interface = string_field(&this, "interface")?;
    let method = string_field(&this, "method")?;
    let input_names = string_list_field(&this, "input_names")?;
    let input_signatures = string_list_field(&this, "input_signatures")?;
    let output_names = string_list_field(&this, "output_names")?;

    let mut inputs: Vec<Option<Value>> = vec![None; input_names.len()];
    let mut next_positional = 0;
    for argument in context.remove_arguments() {
        let idx = match &argument.argument_type {
            Some(name) => input_names
                .iter()
                .position(|n| n == name)
                .ok_or_else(|| format!("Unknown argument `{}`", name))?,
            None => {
                while next_positional < inputs.len() && inputs[next_positional].is_some() {
                    next_positional += 1;
                }
                if next_positional == inputs.len() {
                    return command_error(format!(
                        "Too many arguments, {} accepts {}",
                        method,
                        inputs.len()
                    ));
                }
                next_positional
            }
        };
        if inputs[idx].is_some() {
            return command_error(format!("Argument `{}` specified twice", input_names[idx]));
        }
        inputs[idx] = Some(argument.value);
    }

    let mut message = Message::new_method_call(service, path, interface, method)?;
    {
        let mut iter = IterAppend::new(&mut message);
        for ((value, signature), name) in
            inputs.into_iter().zip(&input_signatures).zip(&input_names)
        {
            let value = value.ok_or_else(|| format!("Missing argument `{}`", name))?;
            encode(&mut iter, signature, value)
                .map_err(|e| format!("Argument `{}`: {}", name, e.message()))?;
        }
    }

    let reply = bus.call(message, CALL_TIMEOUT)?;
    let mut outputs = collect_values(&mut reply.iter_init())?;
    match outputs.len() {
        0 => context.output.empty(),
        1 => context.output.send(outputs.remove(0)),
        _ => context.output.send(Value::Struct(Struct::new(
            output_names.into_iter().zip(outputs).collect(),
            None,
        ))),
    }
}

fn property_value(
    bus: Bus,
    service: &str,
    path: &str,
    interface: &str,
    member_name: &str,
    property: &Property,
) -> Value {
    let writable = property.access.contains("write");
    let readable = property.access.contains("read");
    let mut long_help = format!(
        "The `{}` property of the `{}` interface on the D-Bus object `{}` of the service `{}`, \
         of D-Bus type `{}`.\n\n",
        property.name, interface, path, service, property.signature
    );
    long_help.push_str(match (readable, writable) {
        (true, true) => {
            "Call it without arguments to read its current value, or with a single argument to set it."
        }
        (true, false) => "Call it without arguments to read its current value. It is read only.",
        (false, true) => "Call it with a single argument to set it. It is write only.",
        (false, false) => "It is neither readable nor writable.",
    });
    Value::Struct(Struct::new(
        vec![
            ("bus", Value::from(bus.name())),
            ("service", Value::from(service)),
            ("path", Value::from(path)),
            ("interface", Value::from(interface)),
            ("property", Value::from(property.name.as_str())),
            ("signature", Value::from(property.signature.as_str())),
            (
                "__call__",
                Value::Command(<dyn CrushCommand>::command(
                    call_property,
                    true,
                    ["global", "dbus", "__property__"],
                    format!("{} [value]", member_name),
                    format!("The D-Bus property {}.{}", interface, property.name),
                    Some(long_help),
                    Unknown,
                    [],
                )),
            ),
        ],
        None,
    ))
}

fn call_property(mut context: CommandContext) -> CrushResult<()> {
    let this = context.this.r#struct()?;
    let bus = Bus::from_name(&string_field(&this, "bus")?)?;
    let service = string_field(&this, "service")?;
    let path = string_field(&this, "path")?;
    let interface = string_field(&this, "interface")?;
    let property = string_field(&this, "property")?;
    let signature = string_field(&this, "signature")?;

    let mut arguments = context.remove_arguments();
    match arguments.len() {
        0 => {
            let mut message = Message::new_method_call(
                service,
                path,
                "org.freedesktop.DBus.Properties",
                "Get",
            )?;
            let mut iter = IterAppend::new(&mut message);
            iter.append(interface);
            iter.append(property);
            let reply = bus.call(message, CALL_TIMEOUT)?;
            context.output.send(decode(&mut reply.iter_init())?)
        }
        1 => {
            let value = arguments.remove(0).value;
            let mut message = Message::new_method_call(
                service,
                path,
                "org.freedesktop.DBus.Properties",
                "Set",
            )?;
            {
                let mut iter = IterAppend::new(&mut message);
                iter.append(interface);
                iter.append(property);
                let variant_signature = dbus::Signature::new(signature.clone())?;
                let mut value = Some(value);
                fallible(
                    |f| iter.append_variant(&variant_signature, |i| f(i)),
                    |i| encode(i, &signature, value.take().ok_or("Value already consumed")?),
                )?;
            }
            bus.call(message, CALL_TIMEOUT)?;
            context.output.empty()
        }
        _ => command_error("Expected at most one argument, the new value of the property"),
    }
}

fn signal_value(
    bus: Bus,
    service: &str,
    path: &str,
    interface: &str,
    member_name: &str,
    signal: &Method,
) -> Value {
    let arguments = signal.arguments.iter().collect::<Vec<_>>();
    let names = argument_names(&arguments);
    let columns = names
        .iter()
        .zip(&arguments)
        .map(|(name, a)| {
            ColumnType::new_from_string(name.clone(), column_type_for_signature(&a.signature))
        })
        .collect::<Vec<_>>();

    let mut long_help = format!(
        "Subscribes to the `{}` signal of the `{}` interface on the D-Bus object `{}` of the \
         service `{}`.\n\n\
         Returns a table stream with one row per signal received. It keeps running until it is \
         stopped, e.g. with Ctrl-C, or until a signal arrives after whatever reads the stream has \
         stopped reading, e.g. `head`.",
        signal.name, interface, path, service
    );
    if !arguments.is_empty() {
        long_help.push_str("\n\nThe stream has the following columns:\n\n");
        for (name, a) in names.iter().zip(&arguments) {
            long_help.push_str(&format!("* `{}` (D-Bus type `{}`)\n", name, a.signature));
        }
    }

    Value::Struct(Struct::new(
        vec![
            ("bus", Value::from(bus.name())),
            ("service", Value::from(service)),
            ("path", Value::from(path)),
            ("interface", Value::from(interface)),
            ("signal", Value::from(signal.name.as_str())),
            ("argument_names", string_list(names)),
            (
                "argument_signatures",
                string_list(arguments.iter().map(|a| a.signature.clone())),
            ),
            (
                "__call__",
                Value::Command(<dyn CrushCommand>::command(
                    call_signal,
                    true,
                    ["global", "dbus", "__signal__"],
                    member_name.to_string(),
                    format!("Subscribe to the D-Bus signal {}.{}", interface, signal.name),
                    Some(long_help),
                    Known(ValueType::TableInputStream(columns)),
                    [],
                )),
            ),
        ],
        None,
    ))
}

fn call_signal(mut context: CommandContext) -> CrushResult<()> {
    let this = context.this.r#struct()?;
    let bus = Bus::from_name(&string_field(&this, "bus")?)?;
    let service = string_field(&this, "service")?;
    let path = string_field(&this, "path")?;
    let interface = string_field(&this, "interface")?;
    let signal = string_field(&this, "signal")?;
    let names = string_list_field(&this, "argument_names")?;
    let signatures = string_list_field(&this, "argument_signatures")?;
    if !context.remove_arguments().is_empty() {
        return command_error("Signals take no arguments");
    }

    let columns = names
        .iter()
        .zip(&signatures)
        .map(|(name, signature)| {
            ColumnType::new_from_string(name.clone(), column_type_for_signature(signature))
        })
        .collect::<Vec<_>>();
    let output = context.initialize_output(&columns)?;

    let mut rule = MatchRule::new_signal(
        InterfaceName::new(interface)?,
        Member::new(signal.clone())?,
    )
    .with_sender(BusName::new(service)?);
    // Services that answer on the root object, like the bus itself, don't necessarily emit their
    // signals from it; the bus emits from /org/freedesktop/DBus. So signals of a root object are
    // matched on any path.
    if path != "/" {
        rule = rule.with_path(dbus::Path::new(path)?);
    }

    let connection = bus.connect_local()?;
    let received = Rc::new(RefCell::new(Vec::new()));
    let queue = received.clone();
    connection.add_match(
        rule,
        move |_: (), _: &LocalConnection, message: &Message| {
            if let Ok(message) = message.duplicate() {
                queue.borrow_mut().push(message);
            }
            true
        },
    )?;

    loop {
        connection.process(SIGNAL_POLL_INTERVAL)?;
        let messages = received.borrow_mut().drain(..).collect::<Vec<_>>();
        for message in messages {
            let values = collect_values(&mut message.iter_init())?;
            if values.len() != columns.len() {
                return data_error(format!(
                    "Received the signal {} with {} arguments, expected {}",
                    signal,
                    values.len(),
                    columns.len()
                ));
            }
            match output.send(Row::new(values)) {
                // Whatever read the stream has stopped reading.
                Err(e) if e.is_send_disconnected() => return Ok(()),
                res => res?,
            }
        }
        output.poll_control()?;
    }
}

/// Declare the contents of the object at `path`: one lazily loaded namespace per child object,
/// and one callable member per method and property.
fn load_object(
    env: &mut ScopeLoader,
    bus: Bus,
    service: &str,
    path: &str,
    declared: &mut HashSet<String>,
) -> CrushResult<()> {
    let node = bus.introspect(service, path)?;

    for child in &node.children {
        if !declared.insert(child.clone()) {
            continue;
        }
        let child_path = if path.ends_with('/') {
            format!("{}{}", path, child)
        } else {
            format!("{}/{}", path, child)
        };
        let service = service.to_string();
        env.create_namespace(
            child,
            format!("D-Bus object {} of the service {}", child_path, service),
            None,
            Box::new(move |env| {
                load_object(env, bus, &service, &child_path, &mut HashSet::new())
            }),
        )?;
    }

    let is_hidden = |interface: &Interface| HIDDEN_INTERFACES.contains(&interface.name.as_str());
    let interfaces = node.interfaces.iter().collect::<Vec<_>>();

    // A member name that is used by more than one interface, or that collides with a child object,
    // is prefixed with its interface name, e.g. `Manager_ListSessions`.
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for interface in &interfaces {
        let methods: &[Method] = if is_hidden(interface) {
            &[]
        } else {
            &interface.methods
        };
        for name in methods
            .iter()
            .map(|m| m.name.as_str())
            .chain(interface.properties.iter().map(|p| p.name.as_str()))
            .chain(interface.signals.iter().map(|s| s.name.as_str()))
        {
            *counts.entry(name).or_default() += 1;
        }
    }
    let member_name = |interface: &Interface, name: &str, declared: &HashSet<String>| {
        if counts[name] > 1 || declared.contains(name) {
            format!("{}_{}", short_interface_name(&interface.name), name)
        } else {
            name.to_string()
        }
    };

    for interface in &interfaces {
        let methods: &[Method] = if is_hidden(interface) {
            &[]
        } else {
            &interface.methods
        };
        for method in methods {
            let name = member_name(interface, &method.name, declared);
            if declared.insert(name.clone()) {
                env.declare(
                    &name,
                    method_value(bus, service, path, &interface.name, &name, method),
                )?;
            }
        }
        for property in &interface.properties {
            let name = member_name(interface, &property.name, declared);
            if declared.insert(name.clone()) {
                env.declare(
                    &name,
                    property_value(bus, service, path, &interface.name, &name, property),
                )?;
            }
        }
        for signal in &interface.signals {
            let name = member_name(interface, &signal.name, declared);
            if declared.insert(name.clone()) {
                env.declare(
                    &name,
                    signal_value(bus, service, path, &interface.name, &name, signal),
                )?;
            }
        }
    }
    Ok(())
}

/// Declare the level of the service name tree below `prefix`. For example, with the prefix
/// `org.freedesktop`, the service `org.freedesktop.login1` is declared as `login1`. If `prefix` is
/// itself the name of a service, the object tree of that service is declared as well.
fn load_service_level(
    env: &mut ScopeLoader,
    bus: Bus,
    prefix: &[String],
    services: &Arc<Vec<String>>,
) -> CrushResult<()> {
    let mut declared = HashSet::new();
    let prefix_name = prefix.join(".");
    let child_prefix = if prefix.is_empty() {
        String::new()
    } else {
        format!("{}.", prefix_name)
    };

    for service in services.iter() {
        let Some(rest) = service.strip_prefix(&child_prefix) else {
            continue;
        };
        let Some(segment) = rest.split('.').next() else {
            continue;
        };
        if segment.is_empty() || !declared.insert(segment.to_string()) {
            continue;
        }
        let mut child = prefix.to_vec();
        child.push(segment.to_string());
        let child_name = child.join(".");
        let description = if services.contains(&child_name) {
            format!("D-Bus service {} on the {} bus", child_name, bus.name())
        } else {
            format!(
                "D-Bus services under {} on the {} bus",
                child_name,
                bus.name()
            )
        };
        let services = services.clone();
        env.create_namespace(
            segment,
            description,
            None,
            Box::new(move |env| load_service_level(env, bus, &child, &services)),
        )?;
    }

    if !prefix.is_empty() && services.contains(&prefix_name) {
        let has_sub_services = !declared.is_empty();
        let res = load_object(env, bus, &prefix_name, "/", &mut declared);
        // A service that can't be introspected should not hide the services under it.
        if !has_sub_services {
            res?;
        }
    }
    Ok(())
}

fn declare_bus(dbus: &mut ScopeLoader, bus: Bus) -> CrushResult<()> {
    dbus.create_namespace(
        bus.name(),
        format!("The services on the D-Bus {} bus", bus.name()),
        Some(format!(
            "Every service on the {bus} bus, with its name split on periods, e.g. \
             `dbus:{bus}:org:freedesktop:DBus`. See `help $dbus` for how to find and call services, \
             the services commonly found on each bus, and examples.",
            bus = bus.name()
        )),
        Box::new(move |env| load_service_level(env, bus, &[], &Arc::new(bus.list_services()?))),
    )?;
    Ok(())
}

#[signature(
    dbus.refresh,
    can_block = true,
    output = Known(ValueType::Empty),
    short = "Reload the list of D-Bus services and objects",
    long = "The contents of `dbus:system` and `dbus:session` are loaded the first time they are used. Call this to pick up services and objects that have appeared or disappeared since. Values already stored in variables are not affected.",
    example = "dbus:refresh",
)]
struct Refresh {}

fn refresh(mut context: CommandContext) -> CrushResult<()> {
    Refresh::parse(context.remove_arguments(), &context.global_state.printer())?;
    for bus in [Bus::System, Bus::Session] {
        if let Value::Scope(scope) = context.scope.get_absolute_path(vec![
            "global".to_string(),
            "dbus".to_string(),
            bus.name().to_string(),
        ])? {
            scope.reload()?;
        }
    }
    context.output.empty()
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "dbus",
        "D-Bus services",
        Some(DBUS_LONG_HELP.to_string()),
        Box::new(move |dbus| {
            declare_bus(dbus, Bus::System)?;
            declare_bus(dbus, Bus::Session)?;
            Refresh::declare(dbus)?;
            // The commands behind the members of D-Bus objects. Each member refers to one of these
            // by name, which is what lets a member be serialized, e.g. when a closure that uses it
            // is sent to another host.
            dbus.declare(
                "__method__",
                Value::Command(<dyn CrushCommand>::command(
                    call_method,
                    true,
                    ["global", "dbus", "__method__"],
                    "__method__",
                    "Call a D-Bus method",
                    Some(
                        "The command behind the methods of D-Bus objects, e.g. \
                         `dbus:system:org:freedesktop:DBus:GetId`, which call it with themselves as \
                         `this`.",
                    ),
                    Unknown,
                    [],
                )),
            )?;
            dbus.declare(
                "__property__",
                Value::Command(<dyn CrushCommand>::command(
                    call_property,
                    true,
                    ["global", "dbus", "__property__"],
                    "__property__",
                    "Read or write a D-Bus property",
                    Some(
                        "The command behind the properties of D-Bus objects, e.g. \
                         `dbus:system:org:freedesktop:systemd1:org:freedesktop:systemd1:Version`, \
                         which call it with themselves as `this`.",
                    ),
                    Unknown,
                    [],
                )),
            )?;
            dbus.declare(
                "__signal__",
                Value::Command(<dyn CrushCommand>::command(
                    call_signal,
                    true,
                    ["global", "dbus", "__signal__"],
                    "__signal__",
                    "Subscribe to a D-Bus signal",
                    Some(
                        "The command behind the signals of D-Bus objects, e.g. \
                         `dbus:system:org:freedesktop:DBus:NameOwnerChanged`, which call it with \
                         themselves as `this`.",
                    ),
                    Unknown,
                    [],
                )),
            )?;
            Ok(())
        }),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(signature: &str, value: Value) -> CrushResult<Value> {
        let mut message =
            Message::new_method_call("org.example.Test", "/", "org.example.Test", "Test")?;
        encode(&mut IterAppend::new(&mut message), signature, value)?;
        let mut values = collect_values(&mut message.iter_init())?;
        assert_eq!(values.len(), 1);
        Ok(values.remove(0))
    }

    #[test]
    fn signatures_are_split_into_complete_types() -> CrushResult<()> {
        assert_eq!(
            split_signature("sa{sv}(ii)aay")?,
            vec!["s", "a{sv}", "(ii)", "aay"]
        );
        assert!(split_signature("a{sv").is_err());
        assert!(split_signature("(i").is_err());
        assert!(split_signature("z").is_err());
        Ok(())
    }

    #[test]
    fn malformed_signatures_are_errors() {
        for signature in ["", "a", "a{", "a{sv", "(", "(i", "z"] {
            assert!(complete_type(signature).is_err(), "{}", signature);
            assert!(round_trip(signature, Value::Integer(1)).is_err(), "{}", signature);
            value_type_for_signature(signature);
        }
        // More than one complete type is only rejected when validating, since encoding is only
        // ever given single types.
        assert!(complete_type("ii").is_err());
        assert!(complete_type(&"a".repeat(300)).is_err());
        assert!(parse_introspection(
            r#"<node><interface name="a.b"><method name="M"><arg type="a{"/></method></interface></node>"#
        )
        .is_err());
    }

    #[test]
    fn file_descriptors_are_sent_from_files_and_received_as_binary_streams() -> CrushResult<()> {
        use std::io::Read;
        let path = std::env::temp_dir().join(format!("crush-dbus-fd-test-{}", std::process::id()));
        std::fs::write(&path, "hello")?;
        let received = round_trip("h", Value::from(path.clone()));
        std::fs::remove_file(&path)?;
        let Value::BinaryInputStream(mut stream) = received? else {
            panic!("expected a binary stream");
        };
        let mut content = String::new();
        stream.read_to_string(&mut content)?;
        assert_eq!(content, "hello");
        assert!(round_trip("h", Value::from("not a file")).is_err());
        Ok(())
    }

    #[test]
    fn signals_are_parsed() -> CrushResult<()> {
        let node = parse_introspection(
            r#"<node><interface name="org.example.Manager">
                 <signal name="Changed"><arg name="name" type="s"/><arg type="a{sv}"/></signal>
               </interface></node>"#,
        )?;
        let signal = &node.interfaces[0].signals[0];
        assert_eq!(signal.name, "Changed");
        assert_eq!(signal.arguments.len(), 2);
        assert_eq!(signal.arguments[1].signature, "a{sv}");
        Ok(())
    }

    #[test]
    fn signatures_map_to_crush_types() {
        assert!(
            value_type_for_signature("a{sv}")
                == ValueType::Dict(Box::from(ValueType::String), Box::from(ValueType::Any))
        );
        assert!(value_type_for_signature("ao") == ValueType::List(Box::from(ValueType::String)));
        assert!(value_type_for_signature("t") == ValueType::Integer);
    }

    #[test]
    fn introspection_data_is_parsed() -> CrushResult<()> {
        let node = parse_introspection(
            r#"<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node>
  <interface name="org.example.Manager">
    <method name="Frob">
      <arg name="input" type="s"/>
      <arg name="flags" type="u" direction="in"/>
      <arg name="result" type="a{sv}" direction="out"/>
    </method>
    <property name="Idle" type="b" access="read"/>
  </interface>
  <node name="child"/>
</node>"#,
        )?;
        assert_eq!(node.children, vec!["child"]);
        assert_eq!(node.interfaces.len(), 1);
        let method = &node.interfaces[0].methods[0];
        assert_eq!(method.name, "Frob");
        assert_eq!(
            method
                .arguments
                .iter()
                .map(|a| a.direction)
                .collect::<Vec<_>>(),
            vec![Direction::In, Direction::In, Direction::Out]
        );
        assert_eq!(node.interfaces[0].properties[0].signature, "b");
        Ok(())
    }

    #[test]
    fn basic_values_round_trip() -> CrushResult<()> {
        assert!(round_trip("u", Value::Integer(42))? == Value::Integer(42));
        assert!(round_trip("s", Value::from("hello"))? == Value::from("hello"));
        assert!(round_trip("d", Value::Integer(2))? == Value::Float(2.0));
        assert!(round_trip("o", Value::from("/a/b"))? == Value::from("/a/b"));
        assert!(round_trip("v", Value::Bool(true))? == Value::Bool(true));
        assert!(round_trip("u", Value::Integer(-1)).is_err());
        assert!(round_trip("y", Value::Integer(256)).is_err());
        assert!(round_trip("o", Value::from("not a path")).is_err());
        assert!(round_trip("b", Value::from("true")).is_err());
        Ok(())
    }

    #[test]
    fn container_values_round_trip() -> CrushResult<()> {
        let list: Value = List::new(
            ValueType::Integer,
            vec![Value::Integer(1), Value::Integer(2)],
        )
        .into();
        assert!(round_trip("ai", list.clone())? == list);

        let bytes = Value::Binary(Arc::from(vec![1u8, 2]));
        assert!(round_trip("ay", bytes.clone())? == bytes);
        assert!(round_trip("ay", list.clone())? == bytes);

        let fields = round_trip(
            "(isb)",
            Struct::new(
                vec![
                    ("a", Value::Integer(1)),
                    ("b", Value::from("x")),
                    ("c", Value::Bool(false)),
                ],
                None,
            )
            .into(),
        )?;
        let expected: Value = List::new(
            ValueType::Any,
            vec![Value::Integer(1), Value::from("x"), Value::Bool(false)],
        )
        .into();
        assert!(fields == expected);
        assert!(round_trip("(is)", list.clone()).is_err());

        let options = round_trip(
            "a{sv}",
            Struct::new(vec![("n", Value::Integer(7)), ("s", Value::from("x"))], None).into(),
        )?;
        let Value::Dict(options) = options else {
            panic!("expected a dict");
        };
        assert!(options.get(&Value::from("n")) == Some(Value::Integer(7)));
        assert!(options.get(&Value::from("s")) == Some(Value::from("x")));
        Ok(())
    }
}
