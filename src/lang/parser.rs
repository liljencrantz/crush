use crate::lang::errors::{to_crush_error, CrushResult};
use crate::lang::job::Job;
use crate::lang::data::scope::Scope;
use crate::lang::ast::{TokenNode, JobListNode, TokenType};
use std::sync::{Arc, Mutex};

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

fn close_quote(input: &str) -> String {
    let mut was_backslash = false;
    let mut current_quote = None;

    for ch in input.chars() {
        if was_backslash {
            was_backslash = false;
        } else {
            match (ch, current_quote) {
                ('\\', _) => was_backslash = true,
                ('\"', Some('\"')) => current_quote = None,
                ('\'', Some('\'')) => current_quote = None,
                ('\"', None) => current_quote = Some('\"'),
                ('\'', None) => current_quote = Some('\''),
                _ => {}
            }
        }
    }

    if let Some(missing_quote) = current_quote {
        format!("{}{}", input, missing_quote)
    } else {
        input.to_string()
    }
}

fn close_switch(input: &str) -> String {
    if input.ends_with("--") {
        format!("{}x", input)
    } else {
        input.to_string()
    }
}

#[derive(Clone)]
pub struct Parser {
    parser: Arc<Mutex<lalrparser::JobListParser>>,
    tokenizer: Arc<Mutex<lalrparser::TokenListParser>>,
}

impl Parser {
    pub fn new() -> Parser {
        Parser {
            parser: Arc::from(Mutex::new(lalrparser::JobListParser::new())),
            tokenizer: Arc::from(Mutex::new(lalrparser::TokenListParser::new())),
        }
    }

    pub fn parse(&self, s: &str, env: &Scope) -> CrushResult<Vec<Job>> {
        self.ast(s)?.generate(env)
    }

    pub fn ast(&self, s: &str) -> CrushResult<JobListNode> {
        to_crush_error(self.parser.lock().unwrap().parse(s))
    }

    pub fn tokenize(&self, s: &str) -> CrushResult<Vec<TokenNode>> {
        Ok(to_crush_error(self.tokenizer.lock().unwrap().parse(s))?.tokens)
    }

    /**
    Takes a string and possibly appends a few characters at the end to make the string
    into a valid command. The intent of this command is to take a partial command
    during tab completion and generate a string that can be passed into an abstract
    syntax tree.
    */
    pub fn close_token(&self, input: &str) -> String {
        close_switch(&close_quote(input))
    }
    /**
    Takes a string and possibly appends a few characters at the end to make the string
    into a valid command. The intent of this command is to take a partial command
    during tab completion and generate a string that can be passed into an abstract
    syntax tree.
    */
    pub fn close_command(&self, input: &str) -> CrushResult<String> {
        let input = self.close_token(input);
        let tokens = self.tokenize(&input)?;
        let mut stack = Vec::new();

        let mut needs_trailing_arg = false;

        for tok in &tokens {
            needs_trailing_arg = false;
            match tok.token_type {
                TokenType::FactorOperator | TokenType::AssignmentOperator |
                TokenType::ComparisonOperator | TokenType::UnaryOperator |
                TokenType::LogicalOperator | TokenType::Named | TokenType::Unnamed |
                TokenType::TermOperator | TokenType::Pipe | TokenType::Colon => { needs_trailing_arg = true }
                TokenType::SubStart => { stack.push(")"); }
                TokenType::JobStart => { stack.push("}"); }
                TokenType::GetItemStart => { stack.push("]"); }
                TokenType::SubEnd | TokenType::JobEnd | TokenType::GetItemEnd => { stack.pop(); }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p() -> Parser {
        Parser::new()
    }

    #[test]
    fn check_token_offsets() {
        let tok = p().tokenize("123:123.4 foo=\"bar\"").unwrap();
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
        let tok = p().tokenize("123# comment\nggg").unwrap();
        assert_eq!(tok.len(), 3);
        assert_eq!(tok[0].location(), (0usize, 3usize));
        assert_eq!(tok[1].location(), (12usize, 13usize));
        assert_eq!(tok[2].location(), (13usize, 16usize));
    }

    #[test]
    fn close_command_test() {
        assert_eq!(p().close_command("a --").unwrap(), "a --x");
        assert_eq!(p().close_command("a:").unwrap(), "a: x");
        assert_eq!(p().close_command("a >").unwrap(), "a > x");
        assert_eq!(p().close_command("neg").unwrap(), "neg x");
        assert_eq!(p().close_command("a |").unwrap(), "a | x");
        assert_eq!(p().close_command("x [a").unwrap(), "x [a]");
        assert_eq!(p().close_command("x (a").unwrap(), "x (a)");
        assert_eq!(p().close_command("x {a").unwrap(), "x {a}");
        assert_eq!(p().close_command("x (a) {b} {c (d) (e").unwrap(), "x (a) {b} {c (d) (e)}");
        assert_eq!(p().close_command("a b=").unwrap(), "a b= x");
        assert_eq!(p().close_command("a +").unwrap(), "a + x");
        assert_eq!(p().close_command("a \"").unwrap(), "a \"\"");
    }
}
