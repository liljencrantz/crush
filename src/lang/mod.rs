/**
Core language implementation lives in this crate
*/
pub mod argument;
pub mod ast;
pub mod command;
pub mod command_invocation;
pub mod completion;
pub mod errors;
pub mod execute;
pub mod execution_context;
pub mod files;
pub mod global_state;
pub mod help;
pub mod job;
pub mod number;
pub mod ordered_string_map;
pub mod parser;
pub mod patterns;
pub mod pretty;
pub mod printer;
pub mod serialization;
pub mod pipe;
pub mod threads;
pub mod value;
pub mod data;
pub mod interactive;
