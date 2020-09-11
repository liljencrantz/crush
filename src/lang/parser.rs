use crate::lang::errors::{to_crush_error, CrushResult};
use crate::lang::job::Job;
use crate::lang::data::scope::Scope;
use crate::lang::ast::{TokenNode, JobListNode, TokenType};

lalrpop_mod!(pub lalrparser, "/lang/lalrparser.rs");

pub fn parse_name(s: &str) -> Option<Vec<String>> {
    let res = s.split('/').collect::<Vec<&str>>();
    for i in res.iter() {
        if i.is_empty() {
            return None;
        }
    }
    Some(res.iter().map(|e| e.to_string()).collect())
}

pub fn parse(s: &str, env: &Scope) -> CrushResult<Vec<Job>> {
    to_crush_error(lalrparser::JobListParser::new().parse(s))?.generate(env)
}

pub fn ast(s: &str) -> CrushResult<JobListNode> {
    to_crush_error(lalrparser::JobListParser::new().parse(s))
}

pub fn tokenize(s: &str) -> CrushResult<Vec<TokenNode>> {
    Ok(to_crush_error(lalrparser::TokenListParser::new().parse(s))?.tokens)
}

pub fn close_quote(input: &str) -> String {
    let mut was_backslash = false;
    let mut needs_trailing_quote = false;
    for ch in input.chars() {
        if was_backslash {
            was_backslash = false;
        } else {
            match ch {
                '\\' => was_backslash = true,
                '\"' => needs_trailing_quote = !needs_trailing_quote,
                _ => {}
            }
        }
    }
    if needs_trailing_quote {
        format!("{}\"", input)
    } else {
        input.to_string()
    }
}

pub fn close_command(input: &str) -> CrushResult<String> {
    let input = close_quote(input);
    let tokens = crate::lang::parser::tokenize(&input)?;
    let mut stack = Vec::new();

    let mut needs_trailing_arg = false;

    for tok in &tokens {
        needs_trailing_arg = false;
        match tok.token_type {
            TokenType::FactorOperator | TokenType::AssignmentOperator |
            TokenType::ComparisonOperator | TokenType::UnaryOperator |
            TokenType::LogicalOperator | TokenType::Named | TokenType::Unnamed |
            TokenType::TermOperator => { needs_trailing_arg = true }
            TokenType::SubStart => { stack.push(")"); }
            TokenType::SubEnd => { stack.pop(); }
            TokenType::JobStart => { stack.push("}"); }
            TokenType::JobEnd => { stack.pop(); }
            _ => {}
        }
    }
    stack.reverse();

    Ok(format!(
        "{}{}{}",
        input,
        if needs_trailing_arg { " x" } else { "" },
        stack.join("")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_token_offsets() {
        let tok = tokenize("123:123.4 foo=\"bar\"").unwrap();
        assert_eq!(tok.len(), 6);
        assert_eq!(tok[0].location(), (0usize, 3usize));
        assert_eq!(tok[1].location(), (3usize, 4usize));
        assert_eq!(tok[2].location(), (4usize, 9usize));
        assert_eq!(tok[3].location(), (10usize, 13usize));
        assert_eq!(tok[4].location(), (13usize, 14usize));
        assert_eq!(tok[5].location(), (14usize, 19usize));
    }

    #[test]
    fn check_token_newline() {
        let tok = tokenize("123# comment\nggg").unwrap();
        assert_eq!(tok.len(), 3);
        assert_eq!(tok[0].location(), (0usize, 3usize));
        assert_eq!(tok[1].location(), (12usize, 13usize));
        assert_eq!(tok[2].location(), (13usize, 16usize));
    }

    #[test]
    fn close_command_test() {
        assert_eq!(close_command("x (a").unwrap(), "x (a)");
        assert_eq!(close_command("x {a").unwrap(), "x {a}");
        assert_eq!(close_command("x (a) {b} {c (d) (e").unwrap(), "x (a) {b} {c (d) (e)}");
        assert_eq!(close_command("a b=").unwrap(), "a b= x");
        assert_eq!(close_command("a +").unwrap(), "a + x");
        assert_eq!(close_command("a \"").unwrap(), "a \"\"");
    }

}
