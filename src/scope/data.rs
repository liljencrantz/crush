use std::collections::HashMap;
use crate::{
    errors::error,
    lang::Value,
};
use std::sync::{Mutex, Arc};
use crate::errors::CrushResult;
use crate::lang::ValueType;

#[derive(Debug)]
pub struct ScopeData {
    /** This is the parent scope used to perform variable name resolution. If a variable lookup
     fails in the current scope, it proceeds to this scope.*/
    pub parent_scope: Option<Arc<Mutex<ScopeData>>>,
    /** This is the scope in which the current scope was called. Since a closure can be called
     from inside any scope, it need not be the same as the parent scope. This scope is the one used
     for break/continue loop control. */
    pub calling_scope: Option<Arc<Mutex<ScopeData>>>,

    /** This is a list of scopes that are imported into the current scope. Anything directly inside one
    of these scopes is also considered part of this scope. */
    pub uses: Vec<Arc<Mutex<ScopeData>>>,

    /** The actual data of this scope. */
    pub data: HashMap<String, Value>,

    /** True if this scope is a loop. */
    pub is_loop: bool,

    /** True if this scope should stop execution, i.e. if the continue or break commands have been called.  */
    pub is_stopped: bool,

    pub is_readonly: bool,
}

impl ScopeData {
    pub fn new(parent_scope: Option<Arc<Mutex<ScopeData>>>, caller: Option<Arc<Mutex<ScopeData>>>, is_loop: bool) -> ScopeData {
        return ScopeData {
            parent_scope,
            calling_scope: caller,
            is_loop,
            uses: Vec::new(),
            data: HashMap::new(),
            is_stopped: false,
            is_readonly: false,
        };
    }
}
