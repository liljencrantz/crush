use crate::lang::ast::lexer::TokenizerMode::SkipComments;
use crate::lang::ast::lexer::{LanguageMode, TokenizerMode};
use crate::lang::ast::{JobListNode, lexer::Lexer, token::Token};
/**
    The API for compiling Crush code into a `Vec<Job>`. Internally, this will tokenize the text,
    turn the token list into an AST, and finally compiling the AST into a list of jobs.
*/
use crate::lang::errors::{CrushError, CrushResult};
use crate::lang::job::Job;
use crate::lang::state::scope::Scope;
use std::sync::{Arc, Mutex};

/*
    The AST parser is written in `lalrpop`, and is located in the file `lalrparser.lalrpop`.
    There is a build rule that converts that into this file.
*/
lalrpop_mod!(pub lalrparser, "/lang/lalrparser.rs");

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
    expr_parser: Arc<Mutex<lalrparser::ExprJobListParser>>,
}

impl Parser {
    /// Create a new parser
    pub fn new() -> Parser {
        Parser {
            parser: Arc::from(Mutex::new(lalrparser::JobListParser::new())),
            expr_parser: Arc::from(Mutex::new(lalrparser::ExprJobListParser::new())),
        }
    }

    /// Parse the given string into a `Vec<Job>`, that we can directly evaluate.
    pub fn parse(&self, s: &str, env: &Scope, initial_mode: LanguageMode) -> CrushResult<Vec<Job>> {
        self.ast(s, initial_mode)?.compile(env)
    }

    /// Return the abstract syntax tree (AST) for the supplied command. This is used by the
    /// completion engine.
    pub fn ast(&self, s: &str, initial_mode: LanguageMode) -> CrushResult<JobListNode> {
        let lex = Lexer::new(s, initial_mode, SkipComments);
        match initial_mode {
            LanguageMode::Command => Ok(self.parser.lock().unwrap().parse(s, lex)?),
            LanguageMode::Expression => Ok(self.expr_parser.lock().unwrap().parse(s, lex)?),
        }
    }

    /// Return the list of tokens making up this string. This is used by the syntax highlighting
    /// engine.
    pub fn tokenize<'a>(
        &self,
        s: &'a str,
        initial_mode: LanguageMode,
        tokenizer_mode: TokenizerMode,
    ) -> CrushResult<Vec<Token<'a>>> {
        let l = Lexer::new(s, initial_mode, tokenizer_mode);
        l.into_iter()
            .map(|item| item.map(|it| it.1).map_err(|e| CrushError::from(e)))
            .collect()
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
    during e.g. tab completion and generate a string that can be parsed into an abstract
    syntax tree.
    */
    pub fn close_command(&self, input: &str) -> CrushResult<String> {
        let input = self.close_token(input);
        let tokens = self.tokenize(&input, LanguageMode::Command, SkipComments)?;
        let mut stack = Vec::new();

        let mut needs_trailing_arg = false;

        for tok in &tokens {
            needs_trailing_arg = false;
            match tok {
                Token::Plus(_)
                | Token::Minus(_)
                | Token::Star(_)
                | Token::Slash(_)
                | Token::Bang(_)
                | Token::Equals(_)
                | Token::Declare(_)
                | Token::ComparisonOperator(_, _)
                | Token::UnaryOperator(_, _)
                | Token::LogicalOperator(_, _)
                | Token::Named(_)
                | Token::Unnamed(_)
                | Token::Pipe(_)
                | Token::MemberOperator(_) => needs_trailing_arg = true,
                Token::SubStart(_) => {
                    stack.push(")");
                }
                Token::ExprModeStart(_) => {
                    stack.push(")");
                }
                Token::BlockStart(_) => {
                    stack.push("}");
                }
                Token::GetItemStart(_) => {
                    stack.push("]");
                }
                Token::SubEnd(_) | Token::BlockEnd(_) | Token::GetItemEnd(_) => {
                    stack.pop();
                }
                Token::QuotedString(_, _) => {}
                Token::String(_, _) => {}
                Token::File(_, _) => {}
                Token::Glob(_, _) => {}
                Token::Identifier(_, _) => {}
                Token::Flag(_, _) => {}
                Token::QuotedFile(_, _) => {}
                Token::Regex(_, _) => {}
                Token::Integer(_, _) => {}
                Token::Float(_, _) => {}
                Token::Separator(_, _) => {}
                Token::For(_) => {}
                Token::While(_) => {}
                Token::Loop(_) => {}
                Token::If(_) => {}
                Token::Else(_) => {}
                Token::Return(_) => {}
                Token::Break(_) => {}
                Token::Continue(_) => {}
                Token::Comment(_, _) => {}
                Token::Background(_) => {}
            }
        }
        stack.reverse();

        Ok(format!(
            "{}{}{}",
            input,
            if needs_trailing_arg { " x" } else { "" },
            stack.join("")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::ast::location::Location;

    fn p() -> Parser {
        Parser::new()
    }

    #[test]
    fn check_simple_tokens() {
        let tok = p()
            .tokenize("{aaa}\n", LanguageMode::Command, SkipComments)
            .unwrap();
        assert_eq!(
            tok,
            vec![
                Token::BlockStart(Location::from(0)),
                Token::String("aaa", Location::new(1, 4)),
                Token::BlockEnd(Location::from(4)),
                Token::Separator("\n", Location::from(5)),
            ]
        );
    }

    #[test]
    fn check_expression_tokens() {
        let tok = p()
            .tokenize("e(foo(5, 3.3))\n", LanguageMode::Command, SkipComments)
            .unwrap();
        assert_eq!(
            tok,
            vec![
                Token::ExprModeStart(Location::new(0, 2)),
                Token::Identifier("foo", Location::new(2, 5)),
                Token::ExprModeStart(Location::from(5)),
                Token::Integer("5", Location::from(6)),
                Token::Separator(",", Location::from(7)),
                Token::Float("3.3", Location::new(9, 12)),
                Token::SubEnd(Location::from(12)),
                Token::SubEnd(Location::from(13)),
                Token::Separator("\n", Location::from(14)),
            ]
        );
    }

    #[test]
    fn check_token_offsets() {
        let tok = p()
            .tokenize("123:123.4 foo=\"bar\"", LanguageMode::Command, SkipComments)
            .unwrap();
        assert_eq!(tok.len(), 6);
        assert_eq!(tok[0].location(), Location::new(0, 3));
        assert_eq!(tok[1].location(), Location::new(3usize, 4usize));
        assert_eq!(tok[2].location(), Location::new(4usize, 9usize));
        assert_eq!(tok[3].location(), Location::new(10usize, 13usize));
        assert_eq!(tok[4].location(), Location::new(13usize, 14usize));
        assert_eq!(tok[5].location(), Location::new(14usize, 19usize));
    }

    #[test]
    fn check_token_newline() {
        let tok = p()
            .tokenize("123# comment\nggg", LanguageMode::Command, SkipComments)
            .unwrap();
        assert_eq!(tok.len(), 3);
        assert_eq!(tok[0].location(), Location::new(0usize, 3usize));
        assert_eq!(tok[1].location(), Location::new(12usize, 13usize));
        assert_eq!(tok[2].location(), Location::new(13usize, 16usize));
    }

    #[test]
    fn close_command_test() {
        let p = Parser::new();
        assert_eq!(p.close_command("a --").unwrap(), "a --x");
        assert_eq!(p.close_command("a:").unwrap(), "a: x");
        assert_eq!(p.close_command("a >").unwrap(), "a > x");
        assert_eq!(p.close_command("neg").unwrap(), "neg x");
        assert_eq!(p.close_command("a |").unwrap(), "a | x");
        assert_eq!(p.close_command("x [a").unwrap(), "x [a]");
        assert_eq!(p.close_command("x (a").unwrap(), "x (a)");
        assert_eq!(p.close_command("x {a").unwrap(), "x {a}");
        assert_eq!(
            p.close_command("x (a) {b} {c (d) (e").unwrap(),
            "x (a) {b} {c (d) (e)}"
        );
        assert_eq!(p.close_command("a b=").unwrap(), "a b= x");
        assert_eq!(p.close_command("a +").unwrap(), "a + x");
        assert_eq!(p.close_command("a \"").unwrap(), "a \"\"");
    }

    #[test]
    fn close_quote_test() {
        assert_eq!(close_quote(""), "");
        assert_eq!(close_quote("a"), "a");
        assert_eq!(close_quote("\"a"), "\"a\"");
        assert_eq!(close_quote("\"a'"), "\"a'\"");
        assert_eq!(close_quote("\"a\\\""), "\"a\\\"\"");
        assert_eq!(close_quote("'"), "''");
        assert_eq!(close_quote("'a"), "'a'");
        assert_eq!(close_quote("'a\""), "'a\"'");
        assert_eq!(close_quote("'a\\'"), "'a\\''");
    }
}
