use crate::lang::value::Value;
use crate::util::glob::Glob;
use regex::Regex;
use std::fmt::{Display, Formatter};

pub enum Number {
    Float(f64),
    Integer(i128),
}

impl Number {
    pub fn as_float(&self) -> f64 {
        match self {
            Number::Float(f) => *f,
            Number::Integer(i) => *i as f64,
        }
    }
}
