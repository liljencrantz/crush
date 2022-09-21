/**
Code for managing arguments passed in to commands
 */
use crate::lang::errors::{argument_error, argument_error_legacy, CrushResult, error};
use crate::lang::state::contexts::CompileContext;
use crate::lang::value::Value;
use crate::lang::value::ValueDefinition;
use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use crate::lang::ast::tracked_string::TrackedString;
use crate::lang::ast::location::Location;

#[derive(Debug, Clone)]
pub enum ArgumentType {
    Some(TrackedString),
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
            v.string == "this"
        } else {
            false
        }
    }
}

#[derive(Debug, Clone)]
pub struct BaseArgument<A: Clone, C: Clone> {
    pub argument_type: A,
    pub value: C,
    pub location: Location,
}

impl<A: Clone, C: Clone> BaseArgument<A, C> {
    pub fn error<T>(&self, message: impl Into<String>) -> CrushResult<T> {
        argument_error(message, self.location)
    }
}

pub type ArgumentDefinition = BaseArgument<ArgumentType, ValueDefinition>;

impl ArgumentDefinition {
    pub fn named(name: &TrackedString, value: ValueDefinition) -> ArgumentDefinition {
        ArgumentDefinition {
            argument_type: ArgumentType::Some(name.clone()),
            location: name.location.union(value.location()),
            value,
        }
    }

    pub fn unnamed(value: ValueDefinition) -> ArgumentDefinition {
        ArgumentDefinition {
            argument_type: ArgumentType::None,
            location: value.location(),
            value,
        }
    }

    pub fn list(value: ValueDefinition) -> ArgumentDefinition {
        BaseArgument {
            argument_type: ArgumentType::ArgumentList,
            location: value.location(),
            value,
        }
    }

    pub fn dict(value: ValueDefinition) -> ArgumentDefinition {
        BaseArgument {
            argument_type: ArgumentType::ArgumentDict,
            location: value.location(),
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
    pub fn new(name: Option<String>, value: Value, location: Location) -> Argument {
        Argument {
            argument_type: name,
            value,
            location,
        }
    }

    pub fn unnamed(value: Value, location: Location) -> Argument {
        Argument {
            argument_type: None,
            value,
            location,
        }
    }

    pub fn named(name: &str, value: Value, location: Location) -> Argument {
        BaseArgument {
            argument_type: Some(name.to_string()),
            value,
            location,
        }
    }
}

pub trait ArgumentEvaluator {
    fn eval(&self, context: &mut CompileContext) -> CrushResult<(Vec<Argument>, Option<Value>)>;
}

impl ArgumentEvaluator for Vec<ArgumentDefinition> {
    fn eval(&self, context: &mut CompileContext) -> CrushResult<(Vec<Argument>, Option<Value>)> {
        let mut this = None;
        let mut res = Vec::new();
        for a in self {
            if a.argument_type.is_this() {
                this = Some(a.value.eval_and_bind(context)?);
            } else {
                match &a.argument_type {
                    ArgumentType::Some(name) =>
                        res.push(Argument::named(
                            &name.string,
                            a.value.eval_and_bind(context)?,
                            a.location,
                        )),

                    ArgumentType::None =>
                        res.push(Argument::unnamed(
                            a.value.eval_and_bind(context)?,
                            a.location,
                        )),

                    ArgumentType::ArgumentList => match a.value.eval_and_bind(context)? {
                        Value::List(l) => {
                            let mut copy: Vec<_> = l.iter().collect();
                            for v in copy.drain(..) {
                                res.push(Argument::unnamed(
                                    v,
                                    a.location,
                                ));
                            }
                        }
                        _ => return argument_error_legacy("Argument list must be of type list"),
                    },

                    ArgumentType::ArgumentDict => match a.value.eval_and_bind(context)? {
                        Value::Dict(d) => {
                            let mut copy = d.elements();
                            for (key, value) in copy.drain(..) {
                                if let Value::String(name) = key {
                                    res.push(Argument::named(
                                        &name,
                                        value,
                                        a.location,
                                    ));
                                } else {
                                    return argument_error_legacy("Argument dict must have string keys");
                                }
                            }
                        }
                        _ => return argument_error_legacy("Argument list must be of type list"),
                    },
                }
            }
        }
        Ok((res, this))
    }
}

impl Display for ArgumentDefinition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(f)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::state::contexts::CommandContext;
    use crate::lang::data::list::List;
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
            vec![Argument::named("str_val", Value::from("aa"), Location::new(0, 0))],
            &printer,
        )
            .unwrap();
        assert_eq!(a.str_val, "aa");
        assert!(AllowedValuesStringSignature::parse(
            vec![Argument::named("str_val", Value::from("zz"), Location::new(0, 0))],
            &printer,
        )
            .is_err());

        let a = AllowedValuesCharSignature::parse(
            vec![Argument::named("char_val", Value::from("a"), Location::new(0, 0))],
            &printer,
        )
            .unwrap();
        assert_eq!(a.char_val, 'a');
        assert!(AllowedValuesCharSignature::parse(
            vec![Argument::named("char_val", Value::from("z"), Location::new(0, 0))],
            &printer,
        )
            .is_err());

        let a = AllowedValuesIntSignature::parse(
            vec![Argument::named("int_val", Value::Integer(1), Location::new(0, 0))],
            &printer,
        )
            .unwrap();
        assert_eq!(a.int_val, 1);

        assert!(AllowedValuesIntSignature::parse(
            vec![Argument::named("int_val", Value::Integer(9), Location::new(0, 0))],
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
                vec![Argument::named("int_val", Value::Integer(9), Location::new(0, 0))],
                &printer,
            )
                .unwrap()
                .int_val,
            Some(9)
        );

        assert_eq!(
            OptionSignature::parse(vec![], &printer).unwrap().int_val,
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
                vec![Argument::named("int_val", Value::Integer(9), Location::new(0, 0))],
                &printer,
            )
                .unwrap()
                .int_val,
            9
        );

        assert_eq!(
            DefaultSignature::parse(vec![], &printer).unwrap().int_val,
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
                    Argument::named("list_val", Value::from("a"), Location::new(0, 0)),
                    Argument::named("list_val", Value::from("b"), Location::new(0, 0)),
                    Argument::named("list_val", Value::from("c"), Location::new(0, 0)),
                ],
                &printer,
            )
                .unwrap()
                .list_val,
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );

        assert_eq!(
            ListSignature::parse(vec![], &printer).unwrap().list_val,
            Vec::<String>::new()
        );

        assert_eq!(
            ListSignature::parse(
                vec![
                    Argument::named("list_val", Value::from("a"), Location::new(0, 0)),
                    Argument::named(
                        "list_val",
                        List::new(
                            ValueType::String,
                            [Value::from("b"), Value::from("c")],
                        ).into(),
                        Location::new(0, 0),
                    ),
                    Argument::named("list_val", Value::from("d"), Location::new(0, 0)),
                ],
                &printer,
            )
                .unwrap()
                .list_val,
            vec![
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
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
                    Argument::named("a", Value::from("A"), Location::new(0, 0)),
                    Argument::named("b", Value::from("B"), Location::new(0, 0)),
                    Argument::named("c", Value::from("C"), Location::new(0, 0)),
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
            NamedSignature2::parse(vec![Argument::named("foo", Value::from("s"), Location::new(0, 0))], &printer)
                .unwrap();
        assert_eq!(s.foo, None);
        assert_eq!(
            s.unnamed_val.into_iter().collect::<Vec<_>>(),
            vec![("foo".to_string(), "s".to_string())]
        );
    }

    #[test]
    fn named_signature_with_bad_type() {
        let (printer, _) = crate::lang::printer::init();
        assert!(
            NamedSignature2::parse(vec![Argument::named("foo", Value::Bool(true), Location::new(0, 0))], &printer)
                .is_err()
        );
    }
}
