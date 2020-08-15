use signature::signature;
use crate::lang::argument::ArgumentHandler;
use crate::lang::command::CrushCommand;
use crate::lang::command::OutputType::*;
use crate::lang::errors::{mandate, CrushResult, to_crush_error, data_error, argument_error, error, CrushError};
use crate::lang::execution_context::{ArgumentVector, ExecutionContext};
use crate::lang::scope::Scope;
use crate::lang::value::{Value, ValueType};
use dbus::blocking::{Connection, Proxy, BlockingSender};
use std::time::Duration;
use crate::lang::table::{Row};
use crate::lang::r#struct::Struct;
use crate::lang::list::List;
use dbus::Message;
use dbus::arg::{ArgType, IterAppend};
use std::collections::HashSet;
use std::iter::Peekable;
use std::str::Chars;
use std::convert::{TryInto, TryFrom};

struct DBusThing {
    connection: Connection,
}

impl DBusThing {
    pub fn new(connection: Connection) -> DBusThing {
        DBusThing { connection }
    }

    fn proxy<'a>(&'a self, service: &'a str, object: &'a str) -> Proxy<&'a Connection> {
        self.connection.with_proxy(service, object, Duration::from_millis(5000))
    }

    fn call(&self, service: &str, object: &DBusObject, interface: &DBusInterface, method: &DBusMethod, mut input: Vec<Value>) -> CrushResult<Vec<Value>> {
        let mut msg = Message::new_method_call(service, &object.path, &interface.name, &method.name).unwrap();

        let input_arguments = method.arguments.iter()
            .filter(|a| a.direction == DBusArgumentDirection::In)
            .collect::<Vec<_>>();
        for (a, value) in input_arguments.iter().zip(input.drain(..)) {
            a.serialize(value, &mut msg)?;
        }

        let reply = to_crush_error(self.connection.send_with_reply_and_block(msg, Duration::from_secs(5)))?;
        let values = method.deserialize(&reply)?;
        Ok(values)
    }

    pub fn list_services(&self) -> CrushResult<Vec<String>> {
        let proxy = self.proxy("org.freedesktop.DBus", "/");
        let (mut names, ): (Vec<String>, ) = to_crush_error(proxy.method_call("org.freedesktop.DBus", "ListNames", ()))?;
        Ok(names
            .drain(..)
            .filter(|n| !n.starts_with(':'))
            .collect())
    }

    pub fn list_objects(&self, service: &str) -> CrushResult<Vec<DBusObject>> {
        let mut queue = Vec::new();
        queue.push("/".to_string());
        let mut res = Vec::new();
        while !queue.is_empty() {
            let path = queue.pop().unwrap();
            let sub_proxy = self.proxy(service, &path);
            let (intro_xml, ): (String, ) = to_crush_error(sub_proxy.method_call(
                "org.freedesktop.DBus.Introspectable",//&name,
                "Introspect",
                ()))?;
            let node = parse_interface(&path, &intro_xml)?;
            for o in &node.objects {
                let mut child = path.clone();
                if !child.ends_with('/') {
                    if !o.starts_with('/') {
                        child.push('/');
                    }
                } else if o.starts_with('/') {
                    child = child.trim_end_matches('/').to_string();
                }
                child.push_str(o);
                queue.push(child);
            }
            if !node.interfaces.is_empty() {
                res.push(DBusObject { path, interfaces: node.interfaces })
            }
        }
        Ok(res)
    }
}

fn parse_interface(path: &str, xml: &str) -> CrushResult<DBusParsedInterface> {
    let mut objects = Vec::new();
    let mut interfaces = Vec::new();
    let doc = to_crush_error(roxmltree::Document::parse(xml))?;
    for node in doc.root().children() {
        if !node.is_element() {
            continue;
        }
        if node.tag_name().name() != "node" {
            return data_error("Invalid interface");
        }
        for child in node.children() {
            if !child.is_element() {
                continue;
            }

            match child.tag_name().name() {
                "interface" => {
                    let name = mandate(child.attribute("name"), "Invalid object definition")?.to_string();

                    let mut methods = Vec::new();

                    for method in child.children() {
                        if !method.is_element() {
                            continue;
                        }
                        if method.tag_name().name() != "method" {
                            continue;
                        }
                        let name = mandate(method.attribute("name"), "Invalid object definition")?.to_string();
                        let mut arguments = Vec::new();

                        for argument in method.children() {
                            if !argument.is_element() {
                                continue;
                            }
                            if argument.tag_name().name() != "arg" {
                                continue;
                            }
                            let name = argument.attribute("name").map(|s| s.to_string());
                            let argument_type = mandate(argument.attribute("type"), "Missing argument type attribute")?.to_string();
                            let direction = match mandate(argument.attribute("direction"), "Missing argument direction attribute")?.to_lowercase().as_str() {
                                "in" => DBusArgumentDirection::In,
                                "out" => DBusArgumentDirection::Out,
                                _ => return data_error("Invalid argument direction"),
                            };
                            arguments.push(DBusArgument { name, argument_type, direction });
                        }
                        methods.push(DBusMethod { name, arguments });
                    }

                    interfaces.push(DBusInterface { name, methods });
                }
                "node" => {
                    let object_name = mandate(child.attribute("name"), "Invalid object definition")?;
                    objects.push(object_name.to_string());
                }
                _ => {}
            }
        }
    }

    Ok(DBusParsedInterface {
        path: path.to_string(),
        objects,
        interfaces,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DBusArgumentDirection {
    In,
    Out,
}

#[derive(Debug, Clone)]
struct DBusArgument {
    name: Option<String>,
    argument_type: String,
    direction: DBusArgumentDirection,
}

enum DBusType {
    String,
    Boolean,
    Byte,
    Int16,
    UInt16,
    Int32,
    UInt32,
    Int64,
    UInt64,
    Double,
    Array(Box<DBusType>),
    Variant,
    Invalid,
    DictEntry,
    UnixFd,
    Struct(Vec<DBusType>),
    ObjectPath,
    Signature,
}

impl DBusType {
    fn parse(s: &str) -> CrushResult<DBusType> {
        let mut iter = s.chars().peekable();
        match DBusType::parse_internal(&mut iter) {
            Ok(Some(o)) => Ok(o),
            Ok(None) => argument_error("Missing type"),
            Err(e) => Err(e),
        }
    }

    fn parse_internal(i: &mut Peekable<Chars>) -> CrushResult<Option<DBusType>> {
        match i.next() {
            None => Ok(None),
            Some('b') => Ok(Some(DBusType::Boolean)),
            Some('s') => Ok(Some(DBusType::String)),
            Some('y') => Ok(Some(DBusType::Byte)),
            Some('n') => Ok(Some(DBusType::Int16)),
            Some('q') => Ok(Some(DBusType::UInt16)),
            Some('i') => Ok(Some(DBusType::Int32)),
            Some('u') => Ok(Some(DBusType::UInt32)),
            Some('x') => Ok(Some(DBusType::Int64)),
            Some('t') => Ok(Some(DBusType::UInt64)),
            Some('d') => Ok(Some(DBusType::Double)),
            Some('o') => Ok(Some(DBusType::ObjectPath)),
            Some('g') => Ok(Some(DBusType::Signature)),
            Some('a') => {
                Ok(Some(DBusType::Array(
                    Box::from(mandate(DBusType::parse_internal(i)?, "Expected an array subtype")?)
                )))
            }
            Some('(') => {
                let mut sub = Vec::new();
                while mandate(i.peek(), "Unexpected end of type")? != &')' {
                    sub.push(mandate(DBusType::parse_internal(i)?, "Expected an array subtype")?);
                }
                Ok(Some(DBusType::Struct(sub)))
            }

        Some(ch) => error(&format!("Unknown dbus type '{}'", ch)),
    }
        }
}


impl DBusArgument {
    fn serialize(&self, value: Value, message: &mut Message) -> CrushResult<()> {
        println!("Serialize");
        let t = DBusType::parse(&self.argument_type)?;
        let mut a = IterAppend::new(message);
        match t {
            DBusType::String => {
                if let Value::String(value) = value {
                    a.append(value);
                } else {
                    return argument_error(&format!("Expected a string value, got a {}", value.value_type().to_string()));
                }
            }
            DBusType::Boolean => {
                if let Value::Bool(value) = value {
                    a.append(value);
                } else {
                    return argument_error(&format!("Expected a boolean value, got a {}", value.value_type().to_string()));
                }
            }
            DBusType::Byte => {
                if let Value::Integer(value) = value {
                    a.append(to_crush_error(u8::try_from(value))?);
                } else {
                    return argument_error(&format!("Expected a number, got a {}", value.value_type().to_string()));
                }
            }
            DBusType::Int16 => {
                if let Value::Integer(value) = value {
                    a.append(to_crush_error(i16::try_from(value))?);
                } else {
                    return argument_error(&format!("Expected a number, got a {}", value.value_type().to_string()));
                }
            }
            DBusType::UInt16 => {
                if let Value::Integer(value) = value {
                    a.append(to_crush_error(u16::try_from(value))?);
                } else {
                    return argument_error(&format!("Expected a number, got a {}", value.value_type().to_string()));
                }
            }
            DBusType::Int32 => {
                if let Value::Integer(value) = value {
                    a.append(to_crush_error(i32::try_from(value))?);
                } else {
                    return argument_error(&format!("Expected a number, got a {}", value.value_type().to_string()));
                }
            }
            DBusType::UInt32 => {
                if let Value::Integer(value) = value {
                    a.append(to_crush_error(u32::try_from(value))?);
                } else {
                    return argument_error(&format!("Expected a number, got a {}", value.value_type().to_string()));
                }
            }
            DBusType::Int64 => {
                if let Value::Integer(value) = value {
                    a.append(to_crush_error(i64::try_from(value))?);
                } else {
                    return argument_error(&format!("Expected a number, got a {}", value.value_type().to_string()));
                }
            }
            DBusType::UInt64 => {
                if let Value::Integer(value) = value {
                    a.append(to_crush_error(u64::try_from(value))?);
                } else {
                    return argument_error(&format!("Expected a number value, got a {}", value.value_type().to_string()));
                }
            }
            DBusType::Double => {
                if let Value::Float(value) = value {
                    a.append(value);
                } else {
                    return argument_error(&format!("Expected a floating point number value, got a {}", value.value_type().to_string()));
                }
            }
            DBusType::Array(_) => {}
            DBusType::Variant => {}
            DBusType::Invalid => {}
            DBusType::DictEntry => {}
            DBusType::UnixFd => {}
            DBusType::Struct(fields) => {}
            DBusType::ObjectPath => {}
            DBusType::Signature => {}
        }
        Ok(())
    }
}

fn deserialize(iter: &mut dbus::arg::Iter) -> CrushResult<Value> {
    Ok(match iter.arg_type() {
        ArgType::String => { Value::String(mandate(iter.get(), "Unexpected type")?) }
        ArgType::Boolean => { Value::Bool(mandate(iter.get(), "Unexpected type")?) }
        ArgType::Byte => { Value::Integer(mandate(iter.get::<u8>(), "Unexpected type")? as i128) }
        ArgType::Int16 => { Value::Integer(mandate(iter.get::<i16>(), "Unexpected type")? as i128) }
        ArgType::UInt16 => { Value::Integer(mandate(iter.get::<u16>(), "Unexpected type")? as i128) }
        ArgType::Int32 => { Value::Integer(mandate(iter.get::<i32>(), "Unexpected type")? as i128) }
        ArgType::UInt32 => { Value::Integer(mandate(iter.get::<u32>(), "Unexpected type")? as i128) }
        ArgType::Int64 => { Value::Integer(mandate(iter.get::<i64>(), "Unexpected type")? as i128) }
        ArgType::UInt64 => { Value::Integer(mandate(iter.get::<u64>(), "Unexpected type")? as i128) }
        ArgType::Double => { Value::Float(mandate(iter.get::<f64>(), "Unexpected type")?) }
        ArgType::Array => {
            let mut sub = iter.recurse(ArgType::Array).unwrap();
            let mut res = Vec::new();
            let mut types = HashSet::new();
            while let Ok(it) = deserialize(&mut sub) {
                types.insert(it.value_type());
                res.push(it);
                if !sub.next() {
                    break;
                }
            }
            let list_type = if types.len() == 1 {
                res[0].value_type()
            } else {
                ValueType::Any
            };
            Value::List(List::new(list_type, res))
        }
        ArgType::Variant => panic!("unimplemented"),
        ArgType::Invalid => panic!("unimplemented"),
        ArgType::DictEntry => panic!("unimplemented"),
        ArgType::UnixFd => panic!("unimplemented"),
        ArgType::Struct => panic!("unimplemented"),
        ArgType::ObjectPath => panic!("unimplemented"),
        ArgType::Signature => panic!("unimplemented"),
    })
}


#[derive(Debug, Clone)]
struct DBusMethod {
    name: String,
    arguments: Vec<DBusArgument>,
}

impl DBusMethod {
    fn deserialize(&self, message: &Message) -> CrushResult<Vec<Value>> {
        let mut iter = message.iter_init();
        let mut res = Vec::new();
        for arg in self.arguments.iter()
            .filter(|a| a.direction == DBusArgumentDirection::Out) {
            res.push(deserialize(&mut iter)?);
            iter.next();
        }
        Ok(res)
    }
}

#[derive(Debug, Clone)]
struct DBusInterface {
    name: String,
    methods: Vec<DBusMethod>,
}

#[derive(Debug)]
struct DBusParsedInterface {
    path: String,
    objects: Vec<String>,
    interfaces: Vec<DBusInterface>,
}

#[derive(Debug)]
struct DBusObject {
    path: String,
    interfaces: Vec<DBusInterface>,
}

#[signature(service_call, can_block = false, output = Known(ValueType::Struct), short = "A struct containing all dbus session-level services")]
struct ServiceCall {
    object: Option<Value>,
    method: Option<Value>,
    #[unnamed()]
    arguments: Vec<Value>,
}

fn filter_object(mut input: Vec<DBusObject>, filter: Value) -> CrushResult<DBusObject> {
    let mut res: Vec<_>;
    match &filter {
        Value::File(p) => res = input.drain(..).filter(|o| &o.path == p.to_str().unwrap()).collect(),
        Value::Glob(p) => res = input.drain(..).filter(|o| p.matches(&o.path)).collect(),
        Value::Regex(_, re) => res = input.drain(..).filter(|o| re.is_match(&o.path)).collect(),
        _ => return error("Invalid filter type")
    }
    match res.len() {
        0 => error(&format!("No match for filter {}", filter.to_string())),
        1 => Ok(res.remove(0)),
        _ => error(&format!("Multiple matches for filter {}", filter.to_string())),
    }
}

fn filter_method(mut input: Vec<DBusInterface>, filter: Value) -> CrushResult<(DBusInterface, DBusMethod)> {
    let mut res: Vec<_>;
    let mut flattened = input.drain(..)
        .flat_map(|mut i|
            i.methods.iter().map(|m| (i.clone(), m.clone()))
                .collect::<Vec<_>>())
        .collect::<Vec<_>>();

    match &filter {
        Value::String(p) => res = flattened.drain(..)
            .filter(|(i, m)| &format!("{}.{}", &i.name, &m.name) == p)
            .collect(),
        Value::Glob(p) => res = flattened.drain(..)
            .filter(|(i, m)| p.matches(&format!("{}.{}", &i.name, &m.name)))
            .collect(),
        Value::Regex(_, re) => res = flattened.drain(..)
            .filter(|(i, m)| re.is_match(&format!("{}.{}", &i.name, &m.name)))
            .collect(),
        _ => return error("Invalid filter type")
    }
    match res.len() {
        0 => error(&format!("No match for filter {}", filter.to_string())),
        1 => Ok(res.remove(0)),
        _ => error(&format!("Multiple matches for filter {}", filter.to_string())),
    }
}

fn service_call(context: ExecutionContext) -> CrushResult<()> {
    let cfg: ServiceCall = ServiceCall::parse(context.arguments, &context.printer)?;
    if let Value::Struct(service_obj) = mandate(context.this, "Missing this parameter for method")? {
        if let Value::String(service) = mandate(service_obj.get("service"), "Missing service field in struct")? {
            let dbus = DBusThing::new(to_crush_error(Connection::new_session())?);
            let mut objects = dbus.list_objects(&service)?;
            match (cfg.object, cfg.method) {
                (None, None) => {
                    context.output.send(Value::List(List::new(ValueType::String, objects.drain(..).map(|d| Value::String(d.path)).collect())))
                }
                (Some(object), None) => {
                    let mut object = filter_object(objects, object)?;
                    context.output.send(Value::List(List::new(
                        ValueType::String,
                        object.interfaces.drain(..)
                            .flat_map(|i| i.methods.iter()
                                .map(|m| Value::String(format!("{}.{}", &i.name, &m.name)))
                                .collect::<Vec<_>>())
                            .collect())))
                }
                (Some(object), Some(method)) => {
                    let mut object = filter_object(objects, object)?;
                    let (interface, method) = filter_method(object.interfaces.clone(), method)?;
                    let result = dbus.call(&service, &object, &interface, &method, cfg.arguments)?;
                    context.output.send(Value::List(List::new(ValueType::Any, result)))
                }
                (None, Some(method)) => {
                    argument_error("Missing object")
                }
            }
        } else {
            argument_error("Wrong type of service field")
        }
    } else {
        argument_error("Wrong type of this object")
    }
}

#[signature(session, can_block = false, output = Known(ValueType::Struct), short = "A struct containing all dbus session-level services")]
struct Session {}


fn session(context: ExecutionContext) -> CrushResult<()> {
    let dbus = DBusThing::new(to_crush_error(Connection::new_session())?);

    let mut members = Vec::new();

    for service in dbus.list_services()? {
        members.push((
            service.clone(),
            Value::Struct(Struct::new(
                vec![
                    ("service".to_string(), Value::String(service)),
                    ("__call__".to_string(), Value::Command(CrushCommand::command(
                        service_call, true,
                        vec!["global".to_string(), "dbus".to_string(), "service".to_string(), "__call__".to_string()],
                        "service",
                        "Access object in the specified service",
                        None, Known(ValueType::Empty)))),
                ],
                None,
            ))));
    }

    let res = Struct::new(members, None);

    context.output.send(Value::Struct(res))
}
/*
Usage example

dbus:session and dbus:system are two structs that represent the two "regular" busses.

# Print all services on the bus
dbus:session

Each bus has one member for every known service on the bus, e.g.

# Print all objects on the service
dbus:session:org.pulseaudio.Server

Each service is a method that takes a path or glob to an object. If a service only has one object,
it means you can use '%%' as the object.

# Print all methods on that object
dbus:session:org.pulseaudio.Server /org/pulseaudio/Server

# Invoke a method
dbus:session:org.gnome.Shell %%/ScreenSaver %.setActive true

*/

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_lazy_namespace(
        "dbus",
        Box::new(move |dbus| {
            Session::declare(dbus)?;
            Ok(())
        }),
    )?;
    Ok(())
}
