use crate::lang::command::Command;
use crate::lang::errors::{argument_error, error, CrushResult};
use crate::lang::execution_context::CompileContext;
use crate::lang::printer::Printer;
use crate::lang::scope::ScopeLoader;
use crate::lang::value::Value;
use crate::lang::value::ValueDefinition;
use ordered_map::OrderedMap;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub enum ArgumentType {
    Some(String),
    None,
    ArgumentList,
    ArgumentDict,
}

impl ArgumentType {
    pub fn is_some(&self) -> bool {
        matches!(self, ArgumentType::Some(_))
    }

    pub fn is_this(&self) -> bool {
        if let ArgumentType::Some(v) = self {
            v == "this"
        } else {
            false
        }
    }
}

#[derive(Debug, Clone)]
pub struct BaseArgument<A: Clone, C: Clone> {
    pub argument_type: A,
    pub value: C,
}

pub type ArgumentDefinition = BaseArgument<ArgumentType, ValueDefinition>;

impl ArgumentDefinition {
    pub fn named(name: &str, value: ValueDefinition) -> ArgumentDefinition {
        ArgumentDefinition {
            argument_type: ArgumentType::Some(name.to_string()),
            value,
        }
    }

    pub fn unnamed(value: ValueDefinition) -> ArgumentDefinition {
        ArgumentDefinition {
            argument_type: ArgumentType::None,
            value,
        }
    }

    pub fn list(value: ValueDefinition) -> ArgumentDefinition {
        BaseArgument {
            argument_type: ArgumentType::ArgumentList,
            value,
        }
    }

    pub fn dict(value: ValueDefinition) -> ArgumentDefinition {
        BaseArgument {
            argument_type: ArgumentType::ArgumentDict,
            value,
        }
    }

    pub fn unnamed_value(&self) -> CrushResult<ValueDefinition> {
        if self.argument_type.is_some() {
            error("Expected an unnamed argument")
        } else {
            Ok(self.value.clone())
        }
    }
}

pub type Argument = BaseArgument<Option<String>, Value>;

impl Argument {
    pub fn new(name: Option<String>, value: Value) -> Argument {
        Argument {
            argument_type: name,
            value,
        }
    }

    pub fn unnamed(value: Value) -> Argument {
        Argument {
            argument_type: None,
            value,
        }
    }

    pub fn named(name: &str, value: Value) -> Argument {
        BaseArgument {
            argument_type: Some(name.to_string()),
            value,
        }
    }
}

pub trait ArgumentVecCompiler {
    fn compile(&self, context: &mut CompileContext) -> CrushResult<(Vec<Argument>, Option<Value>)>;
}

impl ArgumentVecCompiler for Vec<ArgumentDefinition> {
    fn compile(&self, context: &mut CompileContext) -> CrushResult<(Vec<Argument>, Option<Value>)> {
        let mut this = None;
        let mut res = Vec::new();
        for a in self {
            if a.argument_type.is_this() {
                this = Some(a.value.compile_bound(context)?);
            } else {
                match &a.argument_type {
                    ArgumentType::Some(name) => {
                        res.push(Argument::named(&name, a.value.compile_bound(context)?))
                    }

                    ArgumentType::None => {
                        res.push(Argument::unnamed(a.value.compile_bound(context)?))
                    }

                    ArgumentType::ArgumentList => match a.value.compile_bound(context)? {
                        Value::List(l) => {
                            let mut copy = l.dump();
                            for v in copy.drain(..) {
                                res.push(Argument::unnamed(v));
                            }
                        }
                        _ => return argument_error("Argument list must be of type list"),
                    },

                    ArgumentType::ArgumentDict => match a.value.compile_bound(context)? {
                        Value::Dict(d) => {
                            let mut copy = d.elements();
                            for (key, value) in copy.drain(..) {
                                if let Value::String(name) = key {
                                    res.push(Argument::named(&name, value));
                                } else {
                                    return argument_error("Argument dict must have string keys");
                                }
                            }
                        }
                        _ => return argument_error("Argument list must be of type list"),
                    },
                }
            }
        }
        Ok((res, this))
    }
}

pub fn column_names(arguments: &Vec<Argument>) -> Vec<String> {
    let mut taken = HashSet::new();
    taken.insert("_".to_string());
    let mut res = Vec::new();
    let mut tmp = String::new();
    for arg in arguments {
        let mut name = match &arg.argument_type {
            None => "_",
            Some(name) => name,
        };
        if taken.contains(name) {
            let mut idx = 1;
            tmp.truncate(0);
            tmp.push_str(name);
            loop {
                tmp.push_str(idx.to_string().as_str());
                idx += 1;
                if !taken.contains(tmp.as_str()) {
                    name = tmp.as_str();
                    break;
                }
                tmp.truncate(name.len());
            }
        }
        taken.insert(name.to_string());
        res.push(name.to_string());
    }

    res
}

pub trait ArgumentHandler: Sized {
    fn declare(env: &mut ScopeLoader) -> CrushResult<()>;
    fn declare_method(env: &mut OrderedMap<String, Command>, path: &Vec<&str>) -> CrushResult<()>;
    fn parse(arguments: Vec<Argument>, printer: &Printer) -> CrushResult<Self>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::execution_context::CommandContext;
    use crate::lang::list::List;
    use crate::lang::ordered_string_map::OrderedStringMap;
    use crate::lang::value::ValueType;
    use signature::signature;

    fn x(_context: CommandContext) -> CrushResult<()> {
        Ok(())
    }

    #[signature(x)]
    struct AllowedValuesStringSignature {
        #[values("aa", "bb", "cc")]
        str_val: String,
    }

    #[signature(x)]
    struct AllowedValuesCharSignature {
        #[values('a', 'b', 'c')]
        char_val: char,
    }

    #[signature(x)]
    struct AllowedValuesIntSignature {
        #[values(1, 2, 3)]
        int_val: i128,
    }

    #[test]
    fn allowed_values() {
        let (printer, _) = crate::lang::printer::init();
        let a = AllowedValuesStringSignature::parse(
            vec![Argument::named("str_val", Value::string("aa"))],
            &printer,
        )
        .unwrap();
        assert_eq!(a.str_val, "aa");
        assert!(AllowedValuesStringSignature::parse(
            vec![Argument::named("str_val", Value::string("zz")),],
            &printer,
        )
        .is_err());

        let a = AllowedValuesCharSignature::parse(
            vec![Argument::named("char_val", Value::string("a"))],
            &printer,
        )
        .unwrap();
        assert_eq!(a.char_val, 'a');
        assert!(AllowedValuesCharSignature::parse(
            vec![Argument::named("char_val", Value::string("z")),],
            &printer,
        )
        .is_err());

        let a = AllowedValuesIntSignature::parse(
            vec![Argument::named("int_val", Value::Integer(1))],
            &printer,
        )
        .unwrap();
        assert_eq!(a.int_val, 1);

        assert!(AllowedValuesIntSignature::parse(
            vec![Argument::named("int_val", Value::Integer(9)),],
            &printer,
        )
        .is_err());
    }

    #[signature(x)]
    struct OptionSignature {
        int_val: Option<i128>,
    }

    #[test]
    fn option_signature() {
        let (printer, _) = crate::lang::printer::init();
        assert_eq!(
            OptionSignature::parse(
                vec![Argument::named("int_val", Value::Integer(9)),],
                &printer,
            )
            .unwrap()
            .int_val,
            Some(9)
        );

        assert_eq!(
            OptionSignature::parse(vec![], &printer,).unwrap().int_val,
            None
        );
    }

    #[signature(x)]
    struct DefaultSignature {
        #[default(8)]
        int_val: i128,
    }

    #[test]
    fn default_signature() {
        let (printer, _) = crate::lang::printer::init();
        assert_eq!(
            DefaultSignature::parse(
                vec![Argument::named("int_val", Value::Integer(9)),],
                &printer,
            )
            .unwrap()
            .int_val,
            9
        );

        assert_eq!(
            DefaultSignature::parse(vec![], &printer,).unwrap().int_val,
            8
        );
    }

    #[signature(x)]
    struct ListSignature {
        list_val: Vec<String>,
    }

    #[test]
    fn list_signature() {
        let (printer, _) = crate::lang::printer::init();
        assert_eq!(
            ListSignature::parse(
                vec![
                    Argument::named("list_val", Value::string("a")),
                    Argument::named("list_val", Value::string("b")),
                    Argument::named("list_val", Value::string("c")),
                ],
                &printer,
            )
            .unwrap()
            .list_val,
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );

        assert_eq!(
            ListSignature::parse(vec![], &printer,).unwrap().list_val,
            Vec::<String>::new()
        );

        assert_eq!(
            ListSignature::parse(
                vec![
                    Argument::named("list_val", Value::string("a")),
                    Argument::named(
                        "list_val",
                        Value::List(List::new(
                            ValueType::String,
                            vec![Value::string("b"), Value::string("c")]
                        ))
                    ),
                    Argument::named("list_val", Value::string("d")),
                ],
                &printer,
            )
            .unwrap()
            .list_val,
            vec![
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string()
            ]
        );
    }

    #[signature(x)]
    struct NamedSignature {
        #[named]
        unnamed_val: OrderedStringMap<String>,
    }

    #[test]
    fn named_signature() {
        let (printer, _) = crate::lang::printer::init();
        assert_eq!(
            NamedSignature::parse(
                vec![
                    Argument::named("a", Value::string("A")),
                    Argument::named("b", Value::string("B")),
                    Argument::named("c", Value::string("C")),
                ],
                &printer,
            )
            .unwrap()
            .unnamed_val
            .into_iter()
            .collect::<Vec<_>>(),
            vec![
                ("a".to_string(), "A".to_string()),
                ("b".to_string(), "B".to_string()),
                ("c".to_string(), "C".to_string()),
            ]
        );
    }

    #[signature(x)]
    struct NamedSignature2 {
        foo: Option<i128>,
        #[named]
        unnamed_val: OrderedStringMap<String>,
    }

    #[test]
    fn named_signature_type_check() {
        let (printer, _) = crate::lang::printer::init();
        let s: NamedSignature2 =
            NamedSignature2::parse(vec![Argument::named("foo", Value::string("s"))], &printer)
                .unwrap();
        assert_eq!(s.foo, None);
        assert_eq!(
            s.unnamed_val.into_iter().collect::<Vec<_>>(),
            vec![("foo".to_string(), "s".to_string()),]
        );
    }

    #[test]
    fn named_signature_with_bad_type() {
        let (printer, _) = crate::lang::printer::init();
        assert!(
            NamedSignature2::parse(vec![Argument::named("foo", Value::Bool(true)),], &printer,)
                .is_err()
        );
    }
}
