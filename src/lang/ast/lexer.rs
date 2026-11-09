use super::location::Location;
use super::token::Token;
use std::fmt::{Display, Formatter, Write};
use std::iter::Peekable;
use std::str::CharIndices;

#[derive(Clone, Copy, Debug)]
pub enum LanguageMode {
    Command,
    Expression,
}

#[derive(Clone, Copy, Debug)]
pub enum TokenizerMode {
    SkipComments,
    IncludeComments,
}

pub struct Lexer<'input> {
    mode: Vec<LanguageMode>,
    tokenizer_mode: TokenizerMode,
    full_str: &'input str,
    chars: Peekable<CharIndices<'input>>,
    /// Byte offset right after the end of the last token returned. Used, together with
    /// `prev_item_end`, to decide whether a bare `.` encountered in command mode is
    /// immediately adjacent to something a member can be accessed on (e.g. `$foo.bar`)
    /// rather than the start of an unrelated bareword/file literal (e.g. `.crush`,
    /// `cat.txt`) -- those already consume any embedded `.` themselves before control
    /// ever returns here, so this only matters for a `.` encountered as the very first
    /// character of a fresh token.
    prev_end: usize,
    /// Whether the last token returned is something a member can be accessed on
    /// (a `$variable`, a closing `)`/`]`/`}`, a quoted literal, or a number) -- see
    /// `prev_end`.
    prev_item_end: bool,
    /// Whether the last token returned was a `MemberOperator`. The token immediately
    /// following it is the member name, and must be scanned with a narrower,
    /// identifier-style charset that excludes `.` -- otherwise `$foo.bar.baz` would
    /// swallow `bar.baz` as a single member name instead of stopping at the second
    /// `.` so it can chain.
    prev_was_member_operator: bool,
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
            prev_end: 0,
            prev_item_end: false,
            prev_was_member_operator: false,
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
                        // `:` is no longer the member-access operator (`.` is) -- its
                        // only remaining job is a closure parameter's type annotation
                        // (`|x: integer|`), a distinct token so removing `.`'s dual
                        // role here doesn't take that unrelated use of `:` away too.
                        _ => return Some(Token::TypeAnnotationColon(Location::from(i)).into()),
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
                Some((i, '&')) => return Some(Token::Background(Location::from(i)).into()),
                Some((i, ']')) => return Some(Token::GetItemEnd(Location::from(i)).into()),
                Some((i, '|')) => return Some(Token::Pipe(Location::from(i)).into()),
                Some((i, ';')) => return Some(Token::Separator(";", Location::from(i)).into()),
                Some((i, '\n')) => return Some(Token::Separator("\n", Location::from(i)).into()),
                Some((i, '\\')) => match self.chars.peek() {
                    Some((_, '\n')) => {
                        self.chars.next();
                        continue;
                    }
                    // A `\` immediately followed by anything other than a newline
                    // starts a bareword/glob/file literal with that escaped
                    // character, e.g. `\ hello` (a literal leading space) -- see
                    // `scan_string_or_file_or_glob`.
                    Some(_) => {
                        let (idx2, _) = self.chars.next().unwrap();
                        return self.scan_string_or_file_or_glob(i, idx2);
                    }
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

                Some((i, '=')) => return Some(Token::Equals(Location::from(i)).into()),

                Some((i, '#')) => {
                    let mut end_idx = i;
                    loop {
                        let cc2 = self.chars.peek();
                        match cc2 {
                            None => {
                                self.chars.next();
                                break;
                            }
                            Some((_, '\n')) => {
                                break;
                            }
                            Some((_, _)) => {
                                self.chars.next();
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
                        let cc2 = self.chars.peek().copied();
                        match cc2 {
                            // Only consume the `.` as the start of a fractional part
                            // if a digit actually follows -- `5.3` is a real float,
                            // but `5.__neg__`/`5.len` is a number followed by member
                            // access, not a malformed float glued onto an identifier.
                            // `Peekable` only looks one character ahead directly, so
                            // check the one after that via a cloned iterator instead
                            // of consuming the `.` before knowing which case this is.
                            Some((_, '.'))
                                if !had_period && {
                                    let mut lookahead = self.chars.clone();
                                    lookahead.next();
                                    matches!(lookahead.peek(), Some((_, ch3)) if number_char(*ch3))
                                } =>
                            {
                                had_period = true;
                                end_idx = self.chars.next().unwrap().0;
                            }
                            Some((_, ch2)) if number_or_underscore_char(ch2) => {
                                end_idx = self.chars.next().unwrap().0;
                            }
                            _ => break,
                        }
                    }

                    if !had_period {
                        if let Some((n_chars, suffix_end)) =
                            match_duration_suffix(&self.chars, string_or_file_or_glob_char)
                        {
                            for _ in 0..n_chars {
                                self.chars.next();
                            }
                            let s = &self.full_str[i..suffix_end + 1];
                            return Some(Token::Duration(s, Location::new(i, suffix_end + 1)).into());
                        }
                    }

                    // A letter/digit/underscore glued directly onto the end of a
                    // number, with no recognized duration suffix to explain it (e.g.
                    // `123abc`), is one malformed token, not a number immediately
                    // followed by an unrelated word -- consume the rest of it too, so
                    // the error names the whole thing rather than just its leading
                    // digits.
                    if let Some((_, ch2)) = self.chars.peek() {
                        if identifier_first_char(*ch2) {
                            let mut err_end = end_idx;
                            loop {
                                match self.chars.peek() {
                                    Some((_, ch3)) if identifier_char(*ch3) => {
                                        err_end = self.chars.next().unwrap().0;
                                    }
                                    _ => break,
                                }
                            }
                            return Some(Err(LexicalError::InvalidNumericLiteral(
                                self.full_str[i..err_end + 1].to_string(),
                            )));
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
                    let mut end_idx = i;
                    // The optional second `-` (`--foo`/bare `--`) is consumed here
                    // unconditionally -- `end_idx` must track it explicitly, since
                    // nothing else will: if nothing follows (a bare `--`), the loop
                    // below never runs at all, and the token's own reported span
                    // would otherwise still end at the first dash, one character
                    // short of what was actually consumed from `self.chars`.
                    if let Some((i2, '-')) = self.chars.peek() {
                        end_idx = *i2;
                        self.chars.next();
                    }

                    loop {
                        let cc2 = self.chars.peek();
                        match cc2 {
                            Some((_, ch2)) if identifier_char(*ch2) || *ch2 == '-' => {
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

                // A `.` immediately following something a member can be accessed on
                // (no whitespace gap -- `prev_end` is exactly `i`) is member access, e.g.
                // `$foo.bar`. Checked before the generic bareword arm below, which would
                // otherwise swallow it as the start of a file literal like `.crush`; if
                // the guard fails, match falls through to that arm as usual, so `.crush`
                // and `cat.txt` are completely unaffected -- this only ever fires for a
                // `.` encountered as the very first character of a fresh token.
                Some((i, '.')) if i == self.prev_end && self.prev_item_end => {
                    return Some(Token::MemberOperator(Location::from(i)).into());
                }

                // The member name right after a `MemberOperator` (see
                // `prev_was_member_operator`'s doc comment): a narrower, identifier-style
                // scan so a second `.` isn't swallowed into the same token, letting
                // `$foo.bar.baz` chain instead of reading `bar.baz` as one member name.
                Some((i, ch)) if self.prev_was_member_operator && identifier_first_char(ch) => {
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
                        Token::String(
                            &self.full_str[i..end_idx + 1],
                            Location::new(i, end_idx + 1),
                        )
                        .into(),
                    );
                }

                Some((i, ch)) if string_or_file_or_glob_first_char(ch) => {
                    return self.scan_string_or_file_or_glob(i, i);
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

    /// Scans the rest of a bareword/glob/file literal in command mode, given that
    /// the token's first character has already been consumed from `self.chars` at
    /// byte offset `start` -- `end_idx` is that same offset, unless the first
    /// character was itself the second half of a `\`-escape (see the two call
    /// sites), in which case it's the escaped character's offset instead.
    ///
    /// Continues consuming ordinary bareword characters (`string_or_file_or_glob_char`)
    /// and `\`-escape pairs -- a backslash followed by any other character at all,
    /// consumed as an atomic, never-terminating unit the same way a quoted
    /// string's own `\`-escapes are -- until a real terminator. This is what lets
    /// `\ ` embed a literal space in an otherwise bare token instead of ending it
    /// early, the same escape syntax (`\n`, `\xHH`, `\uHHHH`, `\UHHHHHHHH`, `\` +
    /// anything else literally) `unescape`/`unescape_file` give a *quoted*
    /// string/file -- actually decoding those escapes happens later, once this
    /// raw, still-escaped text is turned into a value (`Node::compile`), exactly
    /// like it already does for a quoted literal.
    ///
    /// Classifying the result as `Glob`/`File`/`String` still happens on this raw
    /// text, via the same substring-contains rules used elsewhere in this lexer --
    /// a literal `.`/`/`/`~`/`*`/`?` produced by escaping (e.g. `\~` not meaning
    /// home-dir expansion) still makes the raw text *contain* that character, so
    /// escaping a classifying character changes what it means once unescaped, but
    /// not which token kind it's classified as here.
    fn scan_string_or_file_or_glob(
        &mut self,
        start: usize,
        mut end_idx: usize,
    ) -> Option<Spanned<'input>> {
        loop {
            match self.chars.peek() {
                Some((_, '\\')) => {
                    self.chars.next();
                    match self.chars.next() {
                        Some((idx2, _)) => end_idx = idx2,
                        None => return Some(Err(LexicalError::UnexpectedEOF)),
                    }
                }
                Some((_, ch2)) if string_or_file_or_glob_char(*ch2) => {
                    end_idx = self.chars.next().unwrap().0;
                }
                _ => break,
            }
        }

        let s = &self.full_str[start..end_idx + 1];

        Some(if s.contains('*') || s.contains('?') {
            Token::Glob(s, Location::new(start, end_idx + 1)).into()
        } else if s.contains('/') || s.contains('.') || s.starts_with('~') {
            Token::File(s, Location::new(start, end_idx + 1)).into()
        } else {
            Token::String(s, Location::new(start, end_idx + 1)).into()
        })
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
                        // `:` is no longer the member-access operator (`.` is) -- its
                        // only remaining job is a closure parameter's type annotation
                        // (`|x: integer|`), a distinct token so removing `.`'s dual
                        // role here doesn't take that unrelated use of `:` away too.
                        _ => return Some(Token::TypeAnnotationColon(Location::from(i)).into()),
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
                    Some((_, '~')) => {
                        self.chars.next();
                        return Some(
                            Token::ComparisonOperator("!~", Location::new(i, i + 2)).into(),
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
                        let cc2 = self.chars.peek().copied();
                        match cc2 {
                            // Only consume the `.` as the start of a fractional part
                            // if a digit actually follows -- `5.3` is a real float,
                            // but `5.__neg__`/`5.len` is a number followed by member
                            // access, not a malformed float glued onto an identifier.
                            // `Peekable` only looks one character ahead directly, so
                            // check the one after that via a cloned iterator instead
                            // of consuming the `.` before knowing which case this is.
                            Some((_, '.'))
                                if !had_period && {
                                    let mut lookahead = self.chars.clone();
                                    lookahead.next();
                                    matches!(lookahead.peek(), Some((_, ch3)) if number_char(*ch3))
                                } =>
                            {
                                had_period = true;
                                end_idx = self.chars.next().unwrap().0;
                            }
                            Some((_, ch2)) if number_or_underscore_char(ch2) => {
                                end_idx = self.chars.next().unwrap().0;
                            }
                            _ => break,
                        }
                    }

                    if !had_period {
                        if let Some((n_chars, suffix_end)) =
                            match_duration_suffix(&self.chars, identifier_char)
                        {
                            for _ in 0..n_chars {
                                self.chars.next();
                            }
                            let s = &self.full_str[i..suffix_end + 1];
                            return Some(Token::Duration(s, Location::new(i, suffix_end + 1)).into());
                        }
                    }

                    // See the identical check in `next_command`'s own number arm for
                    // why this is one malformed token (e.g. `123abc`), not a number
                    // immediately followed by an unrelated identifier.
                    if let Some((_, ch2)) = self.chars.peek() {
                        if identifier_first_char(*ch2) {
                            let mut err_end = end_idx;
                            loop {
                                match self.chars.peek() {
                                    Some((_, ch3)) if identifier_char(*ch3) => {
                                        err_end = self.chars.next().unwrap().0;
                                    }
                                    _ => break,
                                }
                            }
                            return Some(Err(LexicalError::InvalidNumericLiteral(
                                self.full_str[i..err_end + 1].to_string(),
                            )));
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
                        "mod" => Some(Token::Mod(Location::new(i, end_idx + 1)).into()),
                        "rem" => Some(Token::Rem(Location::new(i, end_idx + 1)).into()),
                        "for" => Some(Token::For(Location::new(i, end_idx + 1)).into()),
                        "while" => Some(Token::While(Location::new(i, end_idx + 1)).into()),
                        "loop" => Some(Token::Loop(Location::new(i, end_idx + 1)).into()),
                        "if" => Some(Token::If(Location::new(i, end_idx + 1)).into()),
                        "else" => Some(Token::Else(Location::new(i, end_idx + 1)).into()),
                        "match" => Some(Token::Match(Location::new(i, end_idx + 1)).into()),
                        "default" => Some(Token::Default(Location::new(i, end_idx + 1)).into()),
                        "try" => Some(Token::Try(Location::new(i, end_idx + 1)).into()),
                        "catch" => Some(Token::Catch(Location::new(i, end_idx + 1)).into()),
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

/// If `chars`, positioned right after an integer literal's last digit, begins with one
/// of the duration unit suffixes (nanoseconds/milliseconds/minutes/hours/seconds) and
/// that suffix isn't itself the start of a longer word (e.g. "5seconds" must lex as the
/// integer `5` followed by the bareword `seconds`, not `5s` glued to a stray `econds`,
/// per `is_word_char` -- the same continuation-character predicate the calling mode
/// already uses for barewords/identifiers), returns the suffix's length in chars and
/// the byte index of its last char. Two-char suffixes are tried before their one-char
/// prefixes (`ms`/`ns` before `m`/`s`) so a real `ms`/`ns` is never cut short.
fn match_duration_suffix(
    chars: &Peekable<CharIndices>,
    is_word_char: fn(char) -> bool,
) -> Option<(usize, usize)> {
    for suffix in ["ns", "ms", "h", "m", "s"] {
        let mut lookahead = chars.clone();
        let mut last_idx = None;
        let mut matched = true;
        for expected in suffix.chars() {
            match lookahead.next() {
                Some((idx, ch)) if ch == expected => last_idx = Some(idx),
                _ => {
                    matched = false;
                    break;
                }
            }
        }
        if matched {
            let boundary_ok = match lookahead.peek() {
                Some((_, ch)) => !is_word_char(*ch),
                None => true,
            };
            if boundary_ok {
                return Some((suffix.chars().count(), last_idx.unwrap()));
            }
        }
    }
    None
}

impl<'input> Iterator for Lexer<'input> {
    type Item = Spanned<'input>;

    fn next(&mut self) -> Option<Self::Item> {
        let was_after_member_operator = self.prev_was_member_operator;
        let result = match self.mode.last() {
            Some(LanguageMode::Expression) => self.next_expr(),
            Some(LanguageMode::Command) => self.next_command(),
            None => Some(Err(LexicalError::MismatchedSubEnd)),
        };
        if let Some(Ok((_, ref token, end))) = result {
            self.prev_end = end;
            self.prev_was_member_operator = matches!(token, Token::MemberOperator(..));
            // A member name scanned right after a `MemberOperator` is itself eligible
            // to have a further `.member` chained onto it (see
            // `prev_was_member_operator`'s doc comment) -- but only when it's actually
            // that scan's output (a `String`), not some unrelated token that happens
            // to follow a dangling `.` for other reasons (e.g. a lexical error).
            self.prev_item_end = if was_after_member_operator && matches!(token, Token::String(..)) {
                true
            } else {
                is_item_end_token(token)
            };
        }
        result
    }
}

/// Token kinds a member can be accessed on with an adjacent `.` -- see
/// `Lexer::prev_item_end`.
fn is_item_end_token(token: &Token) -> bool {
    matches!(
        token,
        Token::Identifier(..)
            | Token::SubEnd(..)
            | Token::GetItemEnd(..)
            | Token::BlockEnd(..)
            | Token::QuotedString(..)
            | Token::QuotedFile(..)
            | Token::Integer(..)
            | Token::Float(..)
            | Token::Duration(..)
            | Token::Regex(..)
    )
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
    /// A numeric literal (integer or float) immediately followed by a letter/digit/
    /// underscore with no separator, and not a recognized duration suffix (`5s`,
    /// `3h`, ...) either -- e.g. `123abc`. Previously lexed as two separate tokens (the
    /// number, then a bareword starting right where it left off), silently accepting
    /// what's almost always a typo as two unrelated arguments/values glued together
    /// with nothing between them.
    InvalidNumericLiteral(String),
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
            LexicalError::InvalidNumericLiteral(s) => {
                f.write_str("Invalid numeric literal `")?;
                f.write_str(s)?;
                f.write_str("`")
            }
        }
    }
}
