use std::iter::Peekable;
use std::str::CharIndices;
use crate::lang::ast::{LexicalError, Spanned, Token};
use crate::lang::ast::location::Location;

enum LexerMode {
    Command, Expression
}

pub struct Lexer<'input> {
    mode: Vec<LexerMode>,
    full_str: &'input str,
    chars: Peekable<CharIndices<'input>>,
}

impl<'input> Lexer<'input> {
    pub fn new(input: &'input str) -> Self {
        Lexer {
            mode: vec![LexerMode::Command],
            full_str: input,
            chars: input.char_indices().peekable(),
        }
    }

    fn next_command(&mut self) -> Option<Spanned<'input>> {
        loop {
            let cc = self.chars.next();
            match cc {
                Some((i, '{')) => return Some(Token::JobStart(Location::from(i)).into()),
                Some((i, '}')) => return Some(Token::JobEnd(Location::from(i)).into()),
                Some((i, ':')) => {
                    let cc2 = self.chars.peek();
                    match cc2 {
                        Some((i2, '=')) => {
                            self.chars.next();
                            return Some(Token::Declare(Location::new(i, i + 2)).into());
                        }
                        _ => return Some(Token::MemberOperator(Location::from(i)).into()),
                    }
                }

                Some((i, '(')) => {
                    self.mode.push(LexerMode::Expression);
                    return Some(Token::ExprModeStart(Location::from(i)).into())
                },

                Some((i, ')')) => {
                    if (self.mode.len() == 1) {
                        return Some(Err(LexicalError::MismatchedSubEnd));
                    }
                    self.mode.pop();
                    return Some(Token::SubEnd(Location::from(i)).into())
                },

                Some((i, '[')) => return Some(Token::GetItemStart(Location::from(i)).into()),
                Some((i, ']')) => return Some(Token::GetItemEnd(Location::from(i)).into()),
                Some((i, '|')) => return Some(Token::Pipe(Location::from(i)).into()),
                Some((i, ';')) => return Some(Token::Separator(";", Location::from(i)).into()),
                Some((i, '\n')) => return Some(Token::Separator("\n", Location::from(i)).into()),
                Some((i, '<')) => return Some(Token::ComparisonOperator("<", Location::from(i)).into()),
                Some((i, '>')) => return Some(Token::ComparisonOperator(">", Location::from(i)).into()),
                Some((i, '!')) => {
                    match self.chars.peek() {
                        Some((i2, '=')) => {
                            self.chars.next();
                            return Some(Token::ComparisonOperator("!=", Location::new(i, i + 2)).into());
                        }
                        Some((i2, ch)) => return Some(Err(LexicalError::UnexpectedCharacterWithSuggestion(*ch, '='))),
                        _ => return Some(Err(LexicalError::UnexpectedEOFWithSuggestion('='))),
                    }
                }
                Some((i, '@')) => {
                    let cc2 = self.chars.peek();
                    match cc2 {
                        Some((i2, '@')) => {
                            self.chars.next();
                            return Some(Token::Named(Location::new(i, i + 2)).into());
                        }
                        _ => return Some(Token::Unnamed(Location::from(i)).into()),
                    }
                }

                Some((i, '=')) => {
                    let cc2 = self.chars.peek();
                    match cc2 {
                        Some((i2, '=')) => {
                            self.chars.next();
                            return Some(Token::LogicalOperator("==", Location::new(i, i + 2)).into());
                        }
                        _ => return Some(Token::Equals(Location::from(i)).into()),
                    }
                }

                Some((_, '#')) => {
                    loop {
                        let cc2 = self.chars.next();
                        match cc2 {
                            Some((_, '\n')) | None => break,
                            Some((_, _)) => {}
                        }
                    }
                }


                Some((i, ch)) if number_char(ch) => {
                    let mut end_idx = i;
                    let mut had_period = false;
                    loop {
                        let cc2 = self.chars.peek();
                        match cc2 {
                            Some((i2, '.')) => {
                                if had_period {
                                    break;
                                }
                                had_period = true;
                                end_idx = self.chars.next().unwrap().0;
                            }
                            Some((i2, ch2)) if number_or_underscore_char(*ch2) => {
                                end_idx = self.chars.next().unwrap().0;
                            }
                            _ => {
                                break
                            }
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
                        self.mode.push(LexerMode::Command);
                        self.chars.next();
                        return Some(Token::SubStart(Location::new(i, i + 2)).into());
                    }

                    let mut end_idx = i;

                    loop {
                        let cc2 = self.chars.peek();
                        match cc2 {
                            Some((i2, ch2)) if identifier_char(*ch2) => {
                                end_idx = self.chars.next().unwrap().0;
                            }
                            _ => {
                                break
                            }
                        }
                    }
                    return Some(Token::Identifier(&self.full_str[i..end_idx + 1], Location::new(i, end_idx + 1)).into());
                }

                Some((i, '-')) => {
                    if let Some((_, '-')) = self.chars.peek() {
                        self.chars.next();
                    }

                    let mut end_idx = i;
                    loop {
                        let cc2 = self.chars.peek();
                        match cc2 {
                            Some((i2, ch2)) if identifier_char(*ch2) => {
                                end_idx = self.chars.next().unwrap().0;
                            }
                            _ => {
                                break
                            }
                        }
                    }
                    return Some(Token::Flag(&self.full_str[i..end_idx + 1], Location::new(i, end_idx + 1)).into());
                }

                Some((i, ch)) if string_or_glob_first_char(ch) => {
                    let mut end_idx = i;
                    loop {
                        let cc2 = self.chars.peek();
                        match cc2 {
                            Some((i2, ch2)) if string_or_glob_char(*ch2) => {
                                end_idx = self.chars.next().unwrap().0;
                            }
                            _ => {
                                break
                            }
                        }
                    }

                    let s = &self.full_str[i..end_idx + 1];

                    return match s {
                        "and" => Some(Token::LogicalOperator(s, Location::new(i, end_idx + 1)).into()),
                        "or" => Some(Token::LogicalOperator(s, Location::new(i, end_idx + 1)).into()),
                        _ => Some(Token::StringOrGlob(s, Location::new(i, end_idx + 1)).into()),
                    };
                }

                Some((i, ch)) if file_or_glob_first_char(ch) => {
                    let mut end_idx = i;
                    loop {
                        let cc2 = self.chars.peek();
                        match cc2 {
                            Some((i2, ch2)) if file_or_glob_char(*ch2) => {
                                end_idx = self.chars.next().unwrap().0;
                            }
                            _ => {
                                break
                            }
                        }
                    }

                    let s = &self.full_str[i..end_idx + 1];
                    return Some(Token::FileOrGlob(s, Location::new(i, end_idx + 1)).into());
                }

                Some((i, '"')) => {
                    let mut end_idx = i;
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
                    let mut end_idx = i;
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

                /*
                                Regex(& 'input str, Location),
                                MathModeStart(& 'input str, Location),

                                FactorOperator(& 'input str, Location),
                                TermOperator(& 'input str, Location),
                */

                Some((i, ch)) if (ch == ' ') | (ch == '\n') | (ch == '\r') => continue,
                Some((i, ch)) => return Some(Err(LexicalError::UnexpectedCharacter(ch))),
                None => return None, // End of file
            }
        }
    }

    fn next_expr(&mut self) -> Option<Spanned<'input>> {
        loop {
            let cc = self.chars.next();
            match cc {
                Some((i, '{')) => return Some(Token::JobStart(Location::from(i)).into()),
                Some((i, '}')) => return Some(Token::JobEnd(Location::from(i)).into()),
                Some((i, '.')) => return Some(Token::MemberOperator(Location::from(i)).into()),
                Some((i, ':')) => {
                    let cc2 = self.chars.peek();
                    match cc2 {
                        Some((i2, '=')) => {
                            self.chars.next();
                            return Some(Token::Declare(Location::new(i, i + 2)).into());
                        }
                        _ => return Some(Token::MemberOperator(Location::from(i)).into()),
                    }
                }

                Some((i, '(')) => {
                    self.mode.push(LexerMode::Expression);
                    return Some(Token::ExprModeStart(Location::from(i)).into())
                },

                Some((i, '$')) => {
                    match self.chars.peek() {
                        Some((i2, '(')) => {
                            self.chars.next();
                            self.mode.push(LexerMode::Expression);
                            return Some(Token::SubStart(Location::new(i, i + 2)).into());
                        }
                        Some((_, ch2)) => return Some(Err(LexicalError::UnexpectedCharacterWithSuggestion(*ch2, '('))),
                        _ => return Some(Err(LexicalError::UnexpectedEOFWithSuggestion('('))),
                    }
                },

                Some((i, ')')) => {
                    if (self.mode.len() == 1) {
                        return Some(Err(LexicalError::MismatchedSubEnd));
                    }
                    self.mode.pop();
                    return Some(Token::SubEnd(Location::from(i)).into())
                },

                Some((i, '[')) => return Some(Token::GetItemStart(Location::from(i)).into()),
                Some((i, ']')) => return Some(Token::GetItemEnd(Location::from(i)).into()),
                Some((i, '|')) => return Some(Token::Pipe(Location::from(i)).into()),
                Some((i, ';')) => return Some(Token::Separator(";", Location::from(i)).into()),
                Some((i, ',')) => return Some(Token::Separator(",", Location::from(i)).into()),
                Some((i, '\n')) => return Some(Token::Separator("\n", Location::from(i)).into()),
                Some((i, '<')) => return Some(Token::ComparisonOperator("<", Location::from(i)).into()),
                Some((i, '>')) => return Some(Token::ComparisonOperator(">", Location::from(i)).into()),
                Some((i, '!')) => {
                    match self.chars.peek() {
                        Some((i2, '=')) => {
                            self.chars.next();
                            return Some(Token::ComparisonOperator("!=", Location::new(i, i + 2)).into());
                        }
                        _ => return Some(Token::Bang(Location::from(i)).into()),
                    }
                }
                Some((i, '@')) => {
                    let cc2 = self.chars.peek();
                    match cc2 {
                        Some((i2, '@')) => {
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
                        Some((i2, '=')) => {
                            self.chars.next();
                            return Some(Token::LogicalOperator("==", Location::new(i, i + 2)).into());
                        }
                        _ => return Some(Token::Equals(Location::from(i)).into()),
                    }
                }

                Some((_, '#')) => {
                    loop {
                        let cc2 = self.chars.next();
                        match cc2 {
                            Some((_, '\n')) | None => break,
                            Some((_, _)) => {}
                        }
                    }
                }


                Some((i, ch)) if number_char(ch) => {
                    let mut end_idx = i;
                    let mut had_period = false;
                    loop {
                        let cc2 = self.chars.peek();
                        match cc2 {
                            Some((i2, '.')) => {
                                if had_period {
                                    break;
                                }
                                had_period = true;
                                end_idx = self.chars.next().unwrap().0;
                            }
                            Some((i2, ch2)) if number_or_underscore_char(*ch2) => {
                                end_idx = self.chars.next().unwrap().0;
                            }
                            _ => {
                                break
                            }
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
                            Some((i2, ch2)) if identifier_char(*ch2) => {
                                end_idx = self.chars.next().unwrap().0;
                            }
                            _ => {
                                break
                            }
                        }
                    }

                    let s = &self.full_str[i..end_idx + 1];

                    return match s {
                        "and" => Some(Token::LogicalOperator(s, Location::new(i, end_idx + 1)).into()),
                        "or" => Some(Token::LogicalOperator(s, Location::new(i, end_idx + 1)).into()),
                        _ => Some(Token::Identifier(s, Location::new(i, end_idx + 1)).into()),
                    };
                }

                Some((i, '"')) => {
                    let mut end_idx = i;
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
                    let mut end_idx = i;
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

                /*
                                Regex(& 'input str, Location),
                                MathModeStart(& 'input str, Location),

                                FactorOperator(& 'input str, Location),
                                TermOperator(& 'input str, Location),
                */

                Some((i, ch)) if (ch == ' ') | (ch == '\n') | (ch == '\r') => continue,
                Some((i, ch)) => return Some(Err(LexicalError::UnexpectedCharacter(ch))),
                None => return None, // End of file
            }
        }
    }
}




fn string_or_glob_first_char(ch: char) -> bool {
    (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') || ch == '*' || ch == '?' || ch == '.' || ch == '_' || ch == '-'
}

fn string_or_glob_char(ch: char) -> bool {
    (ch >= '0' && ch <= '9') || (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') || ch == '*' || ch == '?' || ch == '.' || ch == '_' || ch == '-'
}

fn identifier_first_char(ch: char) -> bool {
    (ch >= '0' && ch <= '9') || (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') || ch == '_'
}

fn identifier_char(ch: char) -> bool {
    (ch >= '0' && ch <= '9') || identifier_first_char(ch)
}

fn number_char(ch: char) -> bool {
    (ch >= '0' && ch <= '9')
}

fn number_or_underscore_char(ch: char) -> bool {
    (ch >= '0' && ch <= '9') || ch == '_'
}

fn file_or_glob_first_char(ch: char) -> bool {
    ch == '.' || ch == '~' || ch == '/'
}

fn file_or_glob_char(ch: char) -> bool {
    string_or_glob_char(ch) || ch == '/'
}

impl<'input> Iterator for Lexer<'input> {
    type Item = Spanned<'input>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.mode.last() {
            Some(LexerMode::Expression) => self.next_expr(),
            Some(LexerMode::Command) => self.next_command(),
            None => Some(Err(LexicalError::MismatchedSubEnd))
        }
    }
}
