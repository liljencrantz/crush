use crate::lang::errors::{error, CrushResult};
use std::sync::{Arc, Mutex};
use crate::lang::{value::Value, value::ValueType};
use std::collections::HashMap;
use crate::lang::execution_context::ExecutionContext;
use crate::lang::command::CrushCommand;
use crate::lang::r#struct::Struct;

/**
  This is where we store variables, including functions.

  The data is protected by a mutex, in order to make sure that all threads can read and write
  concurrently.

  The data is protected by an Arc, in order to make sure that it gets deallocated and can be shared
  across threads.

  In order to ensure that there are no deadlocks, a given thread will only ever lock one scope at a
  time. This forces us to manually drop some variables.
*/
#[derive(Clone)]
pub struct Scope {
    data: Arc<Mutex<ScopeData>>,
}

struct ScopeData {
    /** This is the parent scope used to perform variable name resolution. If a variable lookup
     fails in the current scope, it proceeds to this scope. This is usually the scope in which this
     scope was *created*.

     Not that when scopes are used as namespaces, they do not use this scope.
     */
    pub parent_scope: Option<Scope>,
    /** This is the scope in which the current scope was called. Since a closure can be called
     from inside any scope, it need not be the same as the parent scope. This scope is the one used
     for break/continue loop control, and it is also the scope that builds up the namespace hierarchy. */
    pub calling_scope: Option<Scope>,

    /** This is a list of scopes that are imported into the current scope. Anything directly inside
    one of these scopes is also considered part of this scope. */
    pub uses: Vec<Scope>,

    /** The actual data of this scope. */
    pub mapping: HashMap<Box<str>, Value>,

    /** True if this scope is a loop. Required to implement the break/continue commands.*/
    pub is_loop: bool,

    /** True if this scope should stop execution, i.e. if the continue or break commands have been
    called.  */
    pub is_stopped: bool,

    /** True if this scope can not be further modified. Note that mutable variables in it, e.g.
    lists can still be modified. */
    pub is_readonly: bool,

    pub name: Option<Box<str>>,
}

impl ScopeData {
    fn anonymous(parent_scope: Option<Scope>, calling_scope: Option<Scope>, is_loop: bool) -> ScopeData {
        return ScopeData {
            parent_scope,
            calling_scope,
            is_loop,
            uses: Vec::new(),
            mapping: HashMap::new(),
            is_stopped: false,
            is_readonly: false,
            name: None,
        };
    }

    fn named(parent_scope: Option<Scope>, calling_scope: Option<Scope>, is_loop: bool, name: &str) -> ScopeData {
        return ScopeData {
            parent_scope,
            calling_scope,
            is_loop,
            uses: Vec::new(),
            mapping: HashMap::new(),
            is_stopped: false,
            is_readonly: false,
            name: Some(Box::from(name)),
        };
    }
}

impl Scope {
    pub fn new() -> Scope {
        Scope {
            data: Arc::from(Mutex::new(ScopeData::named(None, None, false, "global"))),
        }
    }

    pub fn create_child(&self, caller: &Scope, is_loop: bool) -> Scope {
        Scope {
            data: Arc::from(Mutex::new(ScopeData::anonymous(
                Some(self.clone()),
                Some(caller.clone()),
                is_loop))),
        }
    }

    pub fn do_continue(&self) -> bool {
        let data = self.data.lock().unwrap();
        if data.is_readonly {
            false
        } else if data.is_loop {
            true
        } else {
            let caller = data.calling_scope.clone();
            drop(data);
            let ok = caller
                .map(|p| p.do_continue())
                .unwrap_or(false);
            if !ok {
                false
            } else {
                self.data.lock().unwrap().is_stopped = true;
                true
            }
        }
    }

    pub fn do_break(&self) -> bool {
        let mut data = self.data.lock().unwrap();
        if data.is_readonly {
            false
        } else if data.is_loop {
            data.is_stopped = true;
            true
        } else {
            let caller = data.calling_scope.clone();
            drop(data);
            let ok = caller
                .map(|p| p.do_break())
                .unwrap_or(false);
            if !ok {
                false
            } else {
                self.data.lock().unwrap().is_stopped = true;
                true
            }
        }
    }

    pub fn is_stopped(&self) -> bool {
        self.data.lock().unwrap().is_stopped
    }

    pub fn create_namespace(&self, name: &str) -> CrushResult<Scope> {
        let res = Scope {
            data: Arc::from(Mutex::new(ScopeData::named(None, Some(self.clone()), false, name))),
        };
        self.declare(name, Value::Scope(res.clone()))?;
        Ok(res)
    }

    pub fn declare_command(
        &self, name: &str,
        call: fn(context: ExecutionContext) -> CrushResult<()>,
        can_block: bool,
        signature: &'static str,
        short_help: &'static str,
        long_help: Option<&'static str>,
    ) -> CrushResult<()> {
        let mut full_name = self.full_path()?;
        full_name.push(Box::from(name));
        let command = CrushCommand::command(call, can_block, full_name, signature, short_help, long_help);
        self.declare(name, Value::Command(command))
    }

    pub fn declare_condition_command(
        &self, name: &str,
        call: fn(context: ExecutionContext) -> CrushResult<()>,
        signature: &'static str,
        short_help: &'static str,
        long_help: Option<&'static str>,
    ) -> CrushResult<()> {
        let mut full_name = self.full_path()?;
        full_name.push(Box::from(name));
        let command = CrushCommand::condition(call, full_name, signature, short_help, long_help);
        self.declare(name, Value::Command(command))
    }

    fn full_path(&self) -> CrushResult<Vec<Box<str>>> {
        let data = self.data.lock().unwrap();
        match data.name.clone() {
            None => error("Tried to get full path of anonymous scope"),
            Some(name) => match data.calling_scope.clone() {
                None => Ok(vec![name]),
                Some(parent) => {
                    drop(data);
                    let mut full_path = parent.full_path()?;
                    full_path.push(name);
                    Ok(full_path)
                }
            },
        }
    }

    pub fn root_object(&self) -> Struct {
        match self.global_value(vec![Box::from("global"),Box::from("types"),Box::from("root"),]) {
            Ok(Value::Struct(s)) => s,
            _ => panic!("Root missing!"),
        }
    }

    pub fn global_static_cmd(&self, full_path: Vec<&str>) -> CrushResult<Box<dyn CrushCommand + Sync + Send>> {
        match self.global_value(full_path.iter().map(|p| Box::from(p.clone())).collect()) {
            Ok(Value::Command(cmd)) => Ok(cmd),
            Err(e) => Err(e),
            _ => error("Expected a command"),
        }
    }

    pub fn global_value(&self, full_path: Vec<Box<str>>) -> CrushResult<Value> {
        let data = self.data.lock().unwrap();
        match data.parent_scope.clone() {
            Some(parent) => {
                drop(data);
                parent.global_value(full_path)
            }
            None => {
                drop(data);
                self.cmd_path(&full_path[..])
            }
        }
    }

    fn cmd_path(&self, path: &[Box<str>]) -> CrushResult<Value> {
        if path.is_empty() {
            error("Invalid path for command")
        } else {
            let data = self.data.lock().unwrap();
            match data.name.clone() {
                None => error("Anonymous scope!"),
                Some(name) => {
                    if name != path[0] {
                        error("Invalid scope for command")
                    } else {
                        match path.len() {
                            2 => {
                                match data.mapping.get(&path[1]) {
                                    Some(v) => Ok(v.clone()),
                                    _ => error(format!(
                                        "Could not find command {} in scope {}",
                                    path[1],
                                    path[0]).as_str()),
                                }
                            }
                            _ => {
                                let s = data.mapping.get(&path[1]).map(|v| v.clone());
                                drop(data);
                                match s {
                                    Some(Value::Scope(s)) => {
                                        s.cmd_path(&path[1..])
                                    },
                                    _ => error(format!(
                                        "Could not find scope {} in scope {}",
                                        path[1],
                                        path[0]).as_str()),
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn declare(&self, name: &str, value: Value) -> CrushResult<()> {
        let mut data = self.data.lock().unwrap();
        if data.is_readonly {
            return error("Scope is read only");
        }
        if data.mapping.contains_key(name) {
            return error(format!("Variable ${{{}}} already exists", name).as_str());
        }
        data.mapping.insert(Box::from(name), value);
        Ok(())
    }

    pub fn redeclare(&self, name: &str, value: Value) -> CrushResult<()> {
        let mut data = self.data.lock().unwrap();
        if data.is_readonly {
            return error("Scope is read only");
        }
        data.mapping.insert(Box::from(name), value);
        Ok(())
    }

    pub fn set(&self, name: &str, value: Value) -> CrushResult<()> {
        let mut data = self.data.lock().unwrap();
        if !data.mapping.contains_key(name) {
            match data.parent_scope.clone() {
                Some(p) => {
                    drop(data);
                    p.set(name, value)
                }
                None => error(format!("Unknown variable {}", name).as_str()),
            }
        } else {
            if data.is_readonly {
                error("Scope is read only")
            } else if data.mapping[name].value_type() != value.value_type() {
                error(format!("Type mismatch when reassigning variable ${{{}}}. Use `unset ${{{}}}` to remove old variable.", name, name).as_str())
            } else {
                data.mapping.insert(Box::from(name), value);
                Ok(())
            }
        }
    }

    pub fn remove_str(&self, name: &str) -> Option<Value> {
        let n = &name.split(':').map(|e: &str| Box::from(e)).collect::<Vec<Box<str>>>()[..];
        return self.remove(n);
    }

    pub fn remove(&self, name: &[Box<str>]) -> Option<Value> {
        if name.is_empty() {
            return None;
        }
        if name.len() == 1 {
            self.remove_here(name[0].as_ref())
        } else {
            match self.get(name[0].as_ref()) {
                None => None,
                Some(Value::Scope(env)) => env.remove(&name[1..name.len()]),
                _ => None,
            }
        }
    }

    fn remove_here(&self, key: &str) -> Option<Value> {
        let mut data = self.data.lock().unwrap();
        if !data.mapping.contains_key(key) {
            match data.parent_scope.clone() {
                Some(p) => {
                    drop(data);
                    p.remove_here(key)
                }
                None => None,
            }
        } else {
            if data.is_readonly {
                return None;
            }
            data.mapping.remove(key)
        }
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        let data = self.data.lock().unwrap();
        match data.mapping.get(&Box::from(name)) {
            Some(v) => Some(v.clone()),
            None => {
                let uses = data.uses.clone();
                drop(data);
                for used in &uses {
                    if let Some(res) = used.get(name) {
                        return Some(res);
                    }
                }

                let data2 = self.data.lock().unwrap();

                match data2.parent_scope.clone() {
                    Some(p) => {
                        drop(data2);
                        p.get(name)
                    }
                    None => {
                        None
                    }
                }
            }
        }
    }

    pub fn r#use(&self, other: &Scope) {
        self.data.lock().unwrap().uses.push(other.clone());
    }

    pub fn dump(&self, map: &mut HashMap<String, ValueType>) {
        match self.data.lock().unwrap().parent_scope.clone() {
            Some(p) => p.dump(map),
            None => {}
        }

        for u in self.data.lock().unwrap().uses.clone().iter().rev() {
            u.dump(map);
        }

        let data = self.data.lock().unwrap();
        for (k, v) in data.mapping.iter() {
            map.insert(k.to_string(), v.value_type());
        }
    }


    pub fn readonly(&self) {
        self.data.lock().unwrap().is_readonly = true;
    }

    pub fn to_string(&self) -> String {
        let mut map = HashMap::new();
        self.dump(&mut map);
        map.iter().map(|(k, _v)| k.clone()).collect::<Vec<String>>().join(", ")
    }
}
