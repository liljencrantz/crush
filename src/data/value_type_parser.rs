use crate::data::{ValueType, ColumnType};
use crate::data::value_type_lexer::{ValueTypeLexer, ValueTypeToken};
use crate::errors::{CrushResult, error};
use ValueTypeToken::*;

pub fn parse(s: &str) -> CrushResult<ValueType> {
    let mut lexer = ValueTypeLexer::new(s);
    let res = parse_type(&mut lexer);
    match (&res, lexer.peek().0) {
        (Ok(_), EOF) => res,
        (Err(_), _) => res,
        _ => error(format!("End of type expected, got {:?}", lexer.peek().0).as_str())
    }
}

fn parse_begin_token(lexer: &mut ValueTypeLexer) -> CrushResult<()> {
    match lexer.pop().0 {
        Begin => Ok(()),
        _ => error("Unexpected token, expected '<'")
    }
}

fn parse_end_token(lexer: &mut ValueTypeLexer) -> CrushResult<()> {
    match lexer.pop().0 {
        End => Ok(()),
        _ => error("Unexpected token, expected '>'")
    }
}

fn parse_sep_token(lexer: &mut ValueTypeLexer) -> CrushResult<()> {
    match lexer.pop().0 {
        Sep => Ok(()),
        _ => error("Unexpected token, expected ','")
    }
}

fn parse_to_token(lexer: &mut ValueTypeLexer) -> CrushResult<()> {
    match lexer.pop().0 {
        To => Ok(()),
        _ => error("Unexpected token, expected ':'")
    }
}

fn parse_name_token(lexer: &mut ValueTypeLexer) -> CrushResult<String> {
    match lexer.pop() {
        (Name, name) => Ok(name.to_string()),
        _ => error("Unexpected token, expected ','")
    }
}

fn parse_named_parameter(lexer: &mut ValueTypeLexer) -> CrushResult<ColumnType> {
    let name = parse_name_token(lexer)?;
    parse_to_token(lexer)?;
    let t = parse_type(lexer)?;
    Ok(ColumnType::named(name.as_str(), t))
}

fn parse_named_parameters(lexer: &mut ValueTypeLexer) -> CrushResult<Vec<ColumnType>> {
    let mut res = Vec::new();
    parse_begin_token(lexer)?;

    loop {
        match lexer.peek().0 {
            End => break,
            _ => {},
        };
        res.push(parse_named_parameter(lexer)?);
        match lexer.peek().0 {
            Sep => lexer.pop(),
            _ => break,
        };
    }
    parse_end_token(lexer)?;
    Ok(res)
}

fn parse_type(lexer: &mut ValueTypeLexer) -> CrushResult<ValueType> {
    Ok(match parse_name_token(lexer)?.as_str() {
        "text" => ValueType::Text,
        "integer" => ValueType::Integer,
        "time" => ValueType::Time,
        "duration" => ValueType::Duration,
        "field" => ValueType::Field,
        "glob" => ValueType::Glob,
        "regex" => ValueType::Regex,
        "command" => ValueType::Command,
        "closure" => ValueType::Command,
        "file" => ValueType::File,
        "env" => ValueType::Env,
        "bool" => ValueType::Bool,
        "list" => {
            parse_begin_token(lexer)?;
            let sub_type = parse_type(lexer)?;
            parse_end_token(lexer)?;
            ValueType::List(Box::from(sub_type))
        }
        "dict" => {
            parse_begin_token(lexer)?;
            let key_type = parse_type(lexer)?;
            parse_sep_token(lexer)?;
            let value_type = parse_type(lexer)?;
            parse_end_token(lexer)?;
            ValueType::Dict(Box::from(key_type), Box::from(value_type))
        }
        "output" => {
            ValueType::Stream(parse_named_parameters(lexer)?)
        }
        "rows" => {
            ValueType::Rows(parse_named_parameters(lexer)?)
        }
        "row" => {
            ValueType::Row(parse_named_parameters(lexer)?)
        }
        nam => return error(format!("Unknown type \"{}\"", nam).as_str()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ValueType::*;
    use crate::data::ColumnType;

    #[test]
    fn parse_test() {
        assert_eq!(parse("text").unwrap(), Text);
        assert_eq!(parse("list<text>").unwrap(), List(Box::from(Text)));
        assert_eq!(parse("dict<integer, list<file>>").unwrap(), Dict(Box::from(Integer), Box::from(List(Box::from(File)))));
        assert!(parse("list<text,text>").is_err());
        assert!(parse("hello").is_err());
        assert_eq!(parse("output<>").unwrap(), Stream(vec![]));
        assert_eq!(parse("output<pie:text>").unwrap(), Stream(vec![ColumnType::named("pie", Text)]));
        assert_eq!(parse("rows<pie:text>").unwrap(), Rows(vec![ColumnType::named("pie", Text)]));
        assert_eq!(parse("rows<pie:text,custard:bool,>").unwrap(),
                   Rows(vec![
                       ColumnType::named("pie", Text),
                       ColumnType::named("custard", Bool),
                   ]));
//        assert_eq!(parse("output<list<bool>>").unwrap(), Output(vec![ColumnType::unnamed(List(Box::from(Text)))]));
    }
}
