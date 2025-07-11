use super::location::Location;
use super::token::Token;
use std::fmt::{Display, Formatter, Write};
use std::iter::Peekable;
use std::str::CharIndices;

#[derive(Clone, Copy)]
pub enum LanguageMode {
    Command,
    Expression,
}

#[derive(Clone, Copy)]
pub enum TokenizerMode {
    SkipComments,
    IncludeComments,
}

pub struct Lexer<'input> {
    mode: Vec<LanguageMode>,
    tokenizer_mode: TokenizerMode,
    full_str: &'input str,
    chars: Peekable<CharIndices<'input>>,
}

pub type Spanned<'input> = Result<(usize, Token<'input>, usize), LexicalError>;

impl<'input> Lexer<'input> {
    pub fn new(
        input: &'input str,
        initial_mode: LanguageMode,
        tokenizer_mode: TokenizerMode,
    ) -> Self {
        Lexer {
            mode: vec![initial_mode],
            tokenizer_mode,
            full_str: input,
            chars: input.char_indices().peekable(),
        }
    }

    fn next_command(&mut self) -> Option<Spanned<'input>> {
        loop {
            let cc = self.chars.next();
            match cc {
                Some((i, '{')) => return Some(Token::BlockStart(Location::from(i)).into()),
                Some((i, '}')) => return Some(Token::BlockEnd(Location::from(i)).into()),
                Some((i, ':')) => {
                    let cc2 = self.chars.peek();
                    match cc2 {
                        Some((_, '=')) => {
                            self.chars.next();
                            return Some(Token::Declare(Location::new(i, i + 2)).into());
                        }
                        _ => return Some(Token::MemberOperator(Location::from(i)).into()),
                    }
                }

                Some((i, '(')) => {
                    self.mode.push(LanguageMode::Expression);
                    return Some(Token::ExprModeStart(Location::from(i)).into());
                }

                Some((i, ')')) => {
                    if self.mode.len() == 1 {
                        return Some(Err(LexicalError::MismatchedSubEnd));
                    }
                    self.mode.pop();
                    return Some(Token::SubEnd(Location::from(i)).into());
                }

                Some((i, '[')) => return Some(Token::GetItemStart(Location::from(i)).into()),
                Some((i, ']')) => return Some(Token::GetItemEnd(Location::from(i)).into()),
                Some((i, '|')) => return Some(Token::Pipe(Location::from(i)).into()),
                Some((i, ';')) => return Some(Token::Separator(";", Location::from(i)).into()),
                Some((i, '\n')) => return Some(Token::Separator("\n", Location::from(i)).into()),
                Some((_, '\\')) => match self.chars.peek() {
                    Some((_, '\n')) => {
                        self.chars.next();
                        continue;
                    }
                    Some((_, ch)) => return Some(Err(LexicalError::UnexpectedCharacter(*ch))),
                    None => return Some(Err(LexicalError::UnexpectedEOFWithSuggestion('\n'))),
                },

                Some((_, '!')) => return Some(Err(LexicalError::UnexpectedCharacter('!'))),

                Some((i, '@')) => {
                    let cc2 = self.chars.peek();
                    match cc2 {
                        Some((_, '@')) => {
                            self.chars.next();
                            return Some(Token::Named(Location::new(i, i + 2)).into());
                        }
                        _ => return Some(Token::Unnamed(Location::from(i)).into()),
                    }
                }

                Some((i, '=')) => 
                    return Some(Token::Equals(Location::from(i)).into()),

                Some((i, '#')) => {
                    let mut end_idx = i;
                    loop {
                        let cc2 = self.chars.next();
                        match cc2 {
                            None => {
                                break;
                            }
                            Some((_, '\n')) => {
                                end_idx += 1;
                                break;
                            }
                            Some((_, _)) => {
                                end_idx += 1;
                            }
                        }
                    }
                    if matches!(self.tokenizer_mode, TokenizerMode::IncludeComments) {
                        return Some(
                            Token::Comment(
                                &self.full_str[i..end_idx + 1],
                                Location::new(i, end_idx + 1),
                            )
                                .into(),
                        );
                    }
                }

                Some((i, ch)) if number_char(ch) => {
                    let mut end_idx = i;
                    let mut had_period = false;
                    loop {
                        let cc2 = self.chars.peek();
                        match cc2 {
                            Some((_, '.')) => {
                                if had_period {
                                    break;
                                }
                                had_period = true;
                                end_idx = self.chars.next().unwrap().0;
                            }
                            Some((_, ch2)) if number_or_underscore_char(*ch2) => {
                                end_idx = self.chars.next().unwrap().0;
                            }
                            _ => break,
                        }
                    }

                    let s = &self.full_str[i..end_idx + 1];
                    if had_period {
                        return Some(Token::Float(s, Location::new(i, end_idx + 1)).into());
                    } else {
                        return Some(Token::Integer(s, Location::new(i, end_idx + 1)).into());
                    }
                }

                Some((i, '$')) => {
                    if let Some((_, '(')) = self.chars.peek() {
                        self.mode.push(LanguageMode::Command);
                        self.chars.next();
                        return Some(Token::SubStart(Location::new(i, i + 2)).into());
                    }

                    let mut end_idx = i;

                    loop {
                        let cc2 = self.chars.peek();
                        match cc2 {
                            Some((_, ch2)) if identifier_char(*ch2) => {
                                end_idx = self.chars.next().unwrap().0;
                            }
                            _ => break,
                        }
                    }
                    return Some(
                        Token::Identifier(
                            &self.full_str[i..end_idx + 1],
                            Location::new(i, end_idx + 1),
                        )
                            .into(),
                    );
                }

                Some((i, '^')) => match self.chars.peek() {
                    Some((_, '(')) => {
                        self.chars.next();
                        let mut was_backslash = false;
                        let mut paren_count = 1;
                        let end;
                        loop {
                            if was_backslash {
                                match self.chars.next() {
                                    Some(_) => was_backslash = false,
                                    None => {
                                        return Some(Err(
                                            LexicalError::UnexpectedEOFWithSuggestion(')'),
                                        ));
                                    }
                                }
                            } else {
                                match self.chars.next() {
                                    Some((_, '/')) => was_backslash = true,
                                    Some((i2, ')')) => {
                                        paren_count -= 1;
                                        if paren_count == 0 {
                                            end = i2;
                                            break;
                                        }
                                    }
                                    Some((_, '(')) => paren_count += 1,
                                    Some((_, _)) => {}
                                    None => {
                                        return Some(Err(
                                            LexicalError::UnexpectedEOFWithSuggestion(')'),
                                        ));
                                    }
                                }
                            }
                        }
                        let s = &self.full_str[i + 2..end];
                        return Some(Token::Regex(s, Location::new(i, end + 1)).into());
                    }
                    Some((_, ch2)) => {
                        return Some(Err(LexicalError::UnexpectedCharacterWithSuggestion(
                            *ch2, '(',
                        )));
                    }
                    _ => return Some(Err(LexicalError::UnexpectedEOFWithSuggestion('('))),
                },

                Some((i, '-')) => {
                    if let Some((_, '-')) = self.chars.peek() {
                        self.chars.next();
                    }

                    let mut end_idx = i;
                    loop {
                        let cc2 = self.chars.peek();
                        match cc2 {
                            Some((_, ch2)) if identifier_char(*ch2) => {
                                end_idx = self.chars.next().unwrap().0;
                            }
                            _ => break,
                        }
                    }
                    return Some(
                        Token::Flag(
                            &self.full_str[i..end_idx + 1],
                            Location::new(i, end_idx + 1),
                        )
                            .into(),
                    );
                }

                Some((i, ch)) if string_or_file_or_glob_first_char(ch) => {
                    let mut end_idx = i;
                    loop {
                        let cc2 = self.chars.peek();
                        match cc2 {
                            Some((_, ch2)) if string_or_file_or_glob_char(*ch2) => {
                                end_idx = self.chars.next().unwrap().0;
                            }
                            _ => break,
                        }
                    }

                    let s = &self.full_str[i..end_idx + 1];

                    return
                        if s.contains('*') || s.contains('?') {
                            Some(Token::Glob(s, Location::new(i, end_idx + 1)).into())
                        } else if s.contains('/') || s.contains('.') || s.starts_with('~') {
                            Some(Token::File(s, Location::new(i, end_idx + 1)).into())
                        } else {
                            Some(Token::String(s, Location::new(i, end_idx + 1)).into())
                        };
                }

                Some((i, '"')) => {
                    let end_idx;
                    loop {
                        let cc2 = self.chars.next();
                        match cc2 {
                            Some((i2, '"')) => {
                                end_idx = i2;
                                break;
                            }

                            Some((_, '\\')) => {
                                self.chars.next();
                            }

                            None => return Some(Err(LexicalError::MismatchedDoubleQuote)),

                            _ => {}
                        }
                    }

                    let s = &self.full_str[i..end_idx + 1];
                    return Some(Token::QuotedString(s, Location::new(i, end_idx + 1)).into());
                }

                Some((i, '\'')) => {
                    let end_idx;
                    loop {
                        let cc2 = self.chars.next();
                        match cc2 {
                            Some((i2, '\'')) => {
                                end_idx = i2;
                                break;
                            }

                            Some((_, '\\')) => {
                                self.chars.next();
                            }

                            None => return Some(Err(LexicalError::MismatchedSingleQuote)),

                            _ => {}
                        }
                    }

                    let s = &self.full_str[i..end_idx + 1];
                    return Some(Token::QuotedFile(s, Location::new(i, end_idx + 1)).into());
                }

                Some((_, ch)) if whitespace_char(ch) => continue,
                Some((_, ch)) => return Some(Err(LexicalError::UnexpectedCharacter(ch))),
                None => return None, // End of file
            }
        }
    }

    fn next_expr(&mut self) -> Option<Spanned<'input>> {
        loop {
            let cc = self.chars.next();
            match cc {
                Some((i, '{')) => return Some(Token::BlockStart(Location::from(i)).into()),
                Some((i, '}')) => return Some(Token::BlockEnd(Location::from(i)).into()),
                Some((i, '.')) => return Some(Token::MemberOperator(Location::from(i)).into()),
                Some((i, ':')) => {
                    let cc2 = self.chars.peek();
                    match cc2 {
                        Some((_, '=')) => {
                            self.chars.next();
                            return Some(Token::Declare(Location::new(i, i + 2)).into());
                        }
                        _ => return Some(Token::MemberOperator(Location::from(i)).into()),
                    }
                }

                Some((i, '(')) => {
                    self.mode.push(LanguageMode::Expression);
                    return Some(Token::ExprModeStart(Location::from(i)).into());
                }

                Some((i, '$')) => match self.chars.peek() {
                    Some((_, '(')) => {
                        self.chars.next();
                        self.mode.push(LanguageMode::Command);
                        return Some(Token::SubStart(Location::new(i, i + 2)).into());
                    }
                    Some((_, ch2)) if identifier_first_char(*ch2) => {
                        let mut end_idx = i;
                        loop {
                            let cc2 = self.chars.peek();
                            match cc2 {
                                Some((_, ch3)) if identifier_char(*ch3) => {
                                    end_idx = self.chars.next().unwrap().0;
                                }
                                _ => break,
                            }
                        }

                        let s = &self.full_str[i..end_idx + 1];
                        return Some(Token::Identifier(s, Location::new(i, end_idx + 1)).into());
                    }
                    Some((_, ch2)) => {
                        return Some(Err(LexicalError::UnexpectedCharacterWithSuggestion(
                            *ch2, '(',
                        )));
                    }
                    _ => return Some(Err(LexicalError::UnexpectedEOF)),
                },

                Some((i, '^')) => match self.chars.peek() {
                    Some((_, '(')) => {
                        self.chars.next();
                        let mut was_backslash = false;
                        let mut paren_count = 1;
                        let end;
                        loop {
                            if was_backslash {
                                match self.chars.next() {
                                    Some(_) => was_backslash = false,
                                    None => {
                                        return Some(Err(
                                            LexicalError::UnexpectedEOFWithSuggestion(')'),
                                        ));
                                    }
                                }
                            } else {
                                match self.chars.next() {
                                    Some((_, '/')) => was_backslash = true,
                                    Some((i2, ')')) => {
                                        paren_count -= 1;
                                        if paren_count == 0 {
                                            end = i2;
                                            break;
                                        }
                                    }
                                    Some((_, '(')) => paren_count += 1,
                                    Some((_, _)) => {}
                                    None => {
                                        return Some(Err(
                                            LexicalError::UnexpectedEOFWithSuggestion(')'),
                                        ));
                                    }
                                }
                            }
                        }
                        let s = &self.full_str[i + 2..end];
                        return Some(Token::Regex(s, Location::new(i, end + 1)).into());
                    }
                    Some((_, ch2)) => {
                        return Some(Err(LexicalError::UnexpectedCharacterWithSuggestion(
                            *ch2, '(',
                        )));
                    }
                    _ => return Some(Err(LexicalError::UnexpectedEOFWithSuggestion('('))),
                },

                Some((i, ')')) => {
                    if self.mode.len() == 1 {
                        return Some(Err(LexicalError::MismatchedSubEnd));
                    }
                    self.mode.pop();
                    return Some(Token::SubEnd(Location::from(i)).into());
                }

                Some((i, '[')) => return Some(Token::GetItemStart(Location::from(i)).into()),
                Some((i, ']')) => return Some(Token::GetItemEnd(Location::from(i)).into()),
                Some((i, '|')) => return Some(Token::Pipe(Location::from(i)).into()),
                Some((i, ';')) => return Some(Token::Separator(";", Location::from(i)).into()),
                Some((i, ',')) => return Some(Token::Separator(",", Location::from(i)).into()),
                Some((i, '\n')) => return Some(Token::Separator("\n", Location::from(i)).into()),
                Some((_, '\\')) => match self.chars.peek() {
                    Some((_, '\n')) => {
                        self.chars.next();
                        continue;
                    }
                    Some((_, ch)) => return Some(Err(LexicalError::UnexpectedCharacter(*ch))),
                    None => return Some(Err(LexicalError::UnexpectedEOFWithSuggestion('\n'))),
                },

                Some((i, '<')) => match self.chars.peek() {
                    Some((_, '=')) => {
                        self.chars.next();
                        return Some(
                            Token::ComparisonOperator("<=", Location::new(i, i + 2)).into(),
                        );
                    }
                    _ => return Some(Token::ComparisonOperator("<", Location::from(i)).into()),
                },

                Some((i, '>')) => match self.chars.peek() {
                    Some((_, '=')) => {
                        self.chars.next();
                        return Some(
                            Token::ComparisonOperator(">=", Location::new(i, i + 2)).into(),
                        );
                    }
                    _ => return Some(Token::ComparisonOperator(">", Location::from(i)).into()),
                },

                Some((i, '!')) => match self.chars.peek() {
                    Some((_, '=')) => {
                        self.chars.next();
                        return Some(
                            Token::ComparisonOperator("!=", Location::new(i, i + 2)).into(),
                        );
                    }
                    _ => return Some(Token::Bang(Location::from(i)).into()),
                },
                Some((i, '@')) => {
                    let cc2 = self.chars.peek();
                    match cc2 {
                        Some((_, '@')) => {
                            self.chars.next();
                            return Some(Token::Named(Location::new(i, i + 2)).into());
                        }
                        _ => return Some(Token::Unnamed(Location::from(i)).into()),
                    }
                }
                Some((i, '+')) => return Some(Token::Plus(Location::from(i)).into()),
                Some((i, '-')) => return Some(Token::Minus(Location::from(i)).into()),
                Some((i, '*')) => return Some(Token::Star(Location::from(i)).into()),
                Some((i, '/')) => return Some(Token::Slash(Location::from(i)).into()),

                Some((i, '=')) => {
                    let cc2 = self.chars.peek();
                    match cc2 {
                        Some((_, '=')) => {
                            self.chars.next();
                            return Some(
                                Token::LogicalOperator("==", Location::new(i, i + 2)).into(),
                            );
                        }
                        Some((_, '~')) => {
                            self.chars.next();
                            return Some(
                                Token::ComparisonOperator("=~", Location::new(i, i + 2)).into(),
                            );
                        }
                        _ => return Some(Token::Equals(Location::from(i)).into()),
                    }
                }

                Some((_, '#')) => loop {
                    let cc2 = self.chars.next();
                    match cc2 {
                        Some((_, '\n')) | None => break,
                        Some((_, _)) => {}
                    }
                },

                Some((i, ch)) if number_char(ch) => {
                    let mut end_idx = i;
                    let mut had_period = false;
                    loop {
                        let cc2 = self.chars.peek();
                        match cc2 {
                            Some((_, '.')) => {
                                if had_period {
                                    break;
                                }
                                had_period = true;
                                end_idx = self.chars.next().unwrap().0;
                            }
                            Some((_, ch2)) if number_or_underscore_char(*ch2) => {
                                end_idx = self.chars.next().unwrap().0;
                            }
                            _ => break,
                        }
                    }

                    let s = &self.full_str[i..end_idx + 1];
                    if had_period {
                        return Some(Token::Float(s, Location::new(i, end_idx + 1)).into());
                    } else {
                        return Some(Token::Integer(s, Location::new(i, end_idx + 1)).into());
                    }
                }

                Some((i, ch)) if identifier_first_char(ch) => {
                    let mut end_idx = i;
                    loop {
                        let cc2 = self.chars.peek();
                        match cc2 {
                            Some((_, ch2)) if identifier_char(*ch2) => {
                                end_idx = self.chars.next().unwrap().0;
                            }
                            _ => break,
                        }
                    }

                    let s = &self.full_str[i..end_idx + 1];

                    return match s {
                        "and" => {
                            Some(Token::LogicalOperator(s, Location::new(i, end_idx + 1)).into())
                        }
                        "or" => {
                            Some(Token::LogicalOperator(s, Location::new(i, end_idx + 1)).into())
                        }
                        "for" => Some(Token::For(Location::new(i, end_idx + 1)).into()),
                        "while" => Some(Token::While(Location::new(i, end_idx + 1)).into()),
                        "loop" => Some(Token::Loop(Location::new(i, end_idx + 1)).into()),
                        "if" => Some(Token::If(Location::new(i, end_idx + 1)).into()),
                        "else" => Some(Token::Else(Location::new(i, end_idx + 1)).into()),
                        "return" => Some(Token::Return(Location::new(i, end_idx + 1)).into()),
                        "break" => Some(Token::Break(Location::new(i, end_idx + 1)).into()),
                        "continue" => Some(Token::Continue(Location::new(i, end_idx + 1)).into()),
                        _ => Some(Token::String(s, Location::new(i, end_idx + 1)).into()),
                    };
                }

                Some((i, '"')) => {
                    let end_idx;
                    loop {
                        let cc2 = self.chars.next();
                        match cc2 {
                            Some((i2, '"')) => {
                                end_idx = i2;
                                break;
                            }

                            Some((_, '\\')) => {
                                self.chars.next();
                            }

                            None => return Some(Err(LexicalError::MismatchedDoubleQuote)),

                            _ => {}
                        }
                    }

                    let s = &self.full_str[i..end_idx + 1];
                    return Some(Token::QuotedString(s, Location::new(i, end_idx + 1)).into());
                }

                Some((i, '\'')) => {
                    let end_idx;
                    loop {
                        let cc2 = self.chars.next();
                        match cc2 {
                            Some((i2, '\'')) => {
                                end_idx = i2;
                                break;
                            }

                            Some((_, '\\')) => {
                                self.chars.next();
                            }

                            None => return Some(Err(LexicalError::MismatchedSingleQuote)),

                            _ => {}
                        }
                    }

                    let s = &self.full_str[i..end_idx + 1];
                    return Some(Token::QuotedFile(s, Location::new(i, end_idx + 1)).into());
                }

                Some((_, ch)) if whitespace_char(ch) => continue,
                Some((_, ch)) => return Some(Err(LexicalError::UnexpectedCharacter(ch))),
                None => return None, // End of file
            }
        }
    }
}

fn string_or_file_or_glob_first_char(ch: char) -> bool {
    (ch >= 'a' && ch <= 'z')
        || (ch >= 'A' && ch <= 'Z')
        || ch == '*'
        || ch == '?'
        || ch == '_'
        || ch == '-'
        || ch == '.'
        || ch == '~'
        || ch == '/'
}

fn string_or_file_or_glob_char(ch: char) -> bool {
    string_or_file_or_glob_first_char(ch) || (ch >= '0' && ch <= '9')
}

fn identifier_first_char(ch: char) -> bool {
    (ch >= '0' && ch <= '9') || (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') || ch == '_'
}

fn identifier_char(ch: char) -> bool {
    (ch >= '0' && ch <= '9') || identifier_first_char(ch)
}

fn number_char(ch: char) -> bool {
    ch >= '0' && ch <= '9'
}

fn number_or_underscore_char(ch: char) -> bool {
    (ch >= '0' && ch <= '9') || ch == '_'
}

fn whitespace_char(ch: char) -> bool {
    (ch == ' ') || (ch == '\r')
}

impl<'input> Iterator for Lexer<'input> {
    type Item = Spanned<'input>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.mode.last() {
            Some(LanguageMode::Expression) => self.next_expr(),
            Some(LanguageMode::Command) => self.next_command(),
            None => Some(Err(LexicalError::MismatchedSubEnd)),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub enum LexicalError {
    #[default]
    MismatchedSubEnd,
    MismatchedDoubleQuote,
    MismatchedSingleQuote,
    UnexpectedCharacter(char),
    UnexpectedCharacterWithSuggestion(char, char),
    UnexpectedEOF,
    UnexpectedEOFWithSuggestion(char),
}

impl Display for LexicalError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            LexicalError::MismatchedSubEnd => f.write_str("Mismatched ) (ending parenthesis)"),
            LexicalError::MismatchedDoubleQuote => f.write_str("Mismatched \" (double quote)"),
            LexicalError::MismatchedSingleQuote => f.write_str("Mismatched ' (single quote)"),
            LexicalError::UnexpectedCharacter(c) => {
                f.write_str("Unexpected character '")?;
                f.write_char(*c)?;
                f.write_str("'")
            }
            LexicalError::UnexpectedCharacterWithSuggestion(actual, expected) => {
                f.write_str("Unexpected character '")?;
                f.write_char(*actual)?;
                f.write_str("', expected ")?;
                f.write_char(*expected)
            }
            LexicalError::UnexpectedEOF => f.write_str("Unexpected end of input"),
            LexicalError::UnexpectedEOFWithSuggestion(expected) => {
                f.write_str("Unexpected end of input, expected ")?;
                f.write_char(*expected)
            }
        }
    }
}
