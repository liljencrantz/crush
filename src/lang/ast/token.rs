use super::lexer::Spanned;
use super::location::Location;
use super::tracked_string::TrackedString;
use std::fmt::{Display, Formatter};

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Token<'input> {
    LogicalOperator(&'input str, Location),
    UnaryOperator(&'input str, Location),
    ComparisonOperator(&'input str, Location),
    Bang(Location),
    Plus(Location),
    Minus(Location),
    Star(Location),
    Slash(Location),
    QuotedString(&'input str, Location),
    Comment(&'input str, Location),
    Identifier(&'input str, Location),
    Flag(&'input str, Location),
    QuotedFile(&'input str, Location),
    Glob(&'input str, Location),
    File(&'input str, Location),
    String(&'input str, Location),
    Regex(&'input str, Location),
    Integer(&'input str, Location),
    Float(&'input str, Location),
    Duration(&'input str, Location),
    MemberOperator(Location),
    Equals(Location),
    Declare(Location),
    Separator(&'input str, Location),
    SubStart(Location),
    SubEnd(Location),
    BlockStart(Location),
    BlockEnd(Location),
    GetItemStart(Location),
    GetItemEnd(Location),
    Pipe(Location),
    Unnamed(Location),
    Named(Location),
    ExprModeStart(Location),
    For(Location),
    While(Location),
    Loop(Location),
    If(Location),
    Else(Location),
    Match(Location),
    Default(Location),
    Try(Location),
    Catch(Location),
    Return(Location),
    Break(Location),
    Continue(Location),
    Background(Location),
}

impl Token<'_> {
    pub fn location(&self) -> Location {
        match self {
            Token::LogicalOperator(_, l)
            | Token::UnaryOperator(_, l)
            | Token::ComparisonOperator(_, l)
            | Token::QuotedString(_, l)
            | Token::String(_, l)
            | Token::Comment(_, l)
            | Token::File(_, l)
            | Token::Glob(_, l)
            | Token::Identifier(_, l)
            | Token::Flag(_, l)
            | Token::QuotedFile(_, l)
            | Token::Regex(_, l)
            | Token::Integer(_, l)
            | Token::Float(_, l)
            | Token::Duration(_, l)
            | Token::MemberOperator(l)
            | Token::Equals(l)
            | Token::Declare(l)
            | Token::Separator(_, l)
            | Token::SubStart(l)
            | Token::SubEnd(l)
            | Token::BlockStart(l)
            | Token::BlockEnd(l)
            | Token::GetItemStart(l)
            | Token::GetItemEnd(l)
            | Token::Pipe(l)
            | Token::Unnamed(l)
            | Token::Named(l)
            | Token::Bang(l)
            | Token::Plus(l)
            | Token::Minus(l)
            | Token::Star(l)
            | Token::Slash(l)
            | Token::For(l)
            | Token::While(l)
            | Token::Loop(l)
            | Token::If(l)
            | Token::Else(l)
            | Token::Match(l)
            | Token::Default(l)
            | Token::Try(l)
            | Token::Catch(l)
            | Token::Return(l)
            | Token::Break(l)
            | Token::Continue(l)
            | Token::ExprModeStart(l)
            | Token::Background(l) => *l,
        }
    }

    pub fn as_string(&self) -> &str {
        match self {
            Token::LogicalOperator(s, _)
            | Token::UnaryOperator(s, _)
            | Token::ComparisonOperator(s, _)
            | Token::QuotedString(s, _)
            | Token::String(s, _)
            | Token::Comment(s, _)
            | Token::File(s, _)
            | Token::Glob(s, _)
            | Token::Identifier(s, _)
            | Token::Flag(s, _)
            | Token::QuotedFile(s, _)
            | Token::Regex(s, _)
            | Token::Integer(s, _)
            | Token::Separator(s, _)
            | Token::Duration(s, _)
            | Token::Float(s, _) => s,
            Token::MemberOperator(_) => ":",
            Token::Equals(_) => "=",
            Token::Declare(_) => ":=",
            Token::SubStart(_) => "(",
            Token::SubEnd(_) => "_",
            Token::BlockStart(_) => "{",
            Token::BlockEnd(_) => "}",
            Token::GetItemStart(_) => "[",
            Token::GetItemEnd(_) => "]",
            Token::Pipe(_) => "|",
            Token::Unnamed(_) => "@",
            Token::Named(_) => "@@",
            Token::ExprModeStart(_) => "m(",
            Token::Bang(_) => "!",
            Token::Plus(_) => "+",
            Token::Minus(_) => "-",
            Token::Star(_) => "*",
            Token::Slash(_) => "/",
            Token::For(_) => "for",
            Token::While(_) => "while",
            Token::Loop(_) => "loop",
            Token::If(_) => "if",
            Token::Else(_) => "else",
            Token::Match(_) => "match",
            Token::Default(_) => "default",
            Token::Try(_) => "try",
            Token::Catch(_) => "catch",
            Token::Return(_) => "return",
            Token::Break(_) => "break",
            Token::Continue(_) => "continue",
            Token::Background(_) => "&",
        }
    }
}

impl Display for Token<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&TrackedString::from(self.clone()).string)
    }
}
impl From<Token<'_>> for String {
    fn from(token: Token) -> String {
        TrackedString::from(token).string
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use super::super::lexer::{Lexer, LanguageMode, TokenizerMode};

    /// True if lexing `source`, starting in `mode`, produces at least one token whose
    /// variant matches `expected` (location and string content are ignored).
    fn lexes_to(source: &str, mode: LanguageMode, expected: &Token) -> bool {
        Lexer::new(source, mode, TokenizerMode::IncludeComments).any(|r| {
            matches!(&r, Ok((_, t, _)) if std::mem::discriminant(t) == std::mem::discriminant(expected))
        })
    }

    #[test]
    fn lexer_produces_every_reachable_token_variant() {
        use LanguageMode::{Command, Expression};
        let loc = Location::from(0);

        // One recipe (source text + starting LanguageMode) that reaches each token
        // variant. Several variants only lex in one specific mode -- e.g. keywords
        // (for/if/...) and single-char arithmetic operators (+/-/*//) only exist in
        // Expression mode, while Glob/File/Flag/Background/Comment only exist in
        // Command mode -- so the mode chosen here isn't arbitrary.
        let cases: Vec<(&str, LanguageMode, Token)> = vec![
            ("and", Expression, Token::LogicalOperator("", loc)),
            ("<", Expression, Token::ComparisonOperator("", loc)),
            ("!", Expression, Token::Bang(loc)),
            ("+", Expression, Token::Plus(loc)),
            ("-", Expression, Token::Minus(loc)),
            ("*", Expression, Token::Star(loc)),
            ("/", Expression, Token::Slash(loc)),
            ("\"hi\"", Command, Token::QuotedString("", loc)),
            ("# hi", Command, Token::Comment("", loc)),
            ("$foo", Command, Token::Identifier("", loc)),
            ("-foo", Command, Token::Flag("", loc)),
            ("'hi'", Command, Token::QuotedFile("", loc)),
            ("*.txt", Command, Token::Glob("", loc)),
            ("foo.txt", Command, Token::File("", loc)),
            ("foo", Command, Token::String("", loc)),
            ("^(foo)", Command, Token::Regex("", loc)),
            ("5", Command, Token::Integer("", loc)),
            ("5.0", Command, Token::Float("", loc)),
            ("5s", Command, Token::Duration("", loc)),
            (":", Command, Token::MemberOperator(loc)),
            ("=", Command, Token::Equals(loc)),
            (":=", Command, Token::Declare(loc)),
            (";", Command, Token::Separator("", loc)),
            ("$(", Command, Token::SubStart(loc)),
            ("$()", Command, Token::SubEnd(loc)),
            ("{", Command, Token::BlockStart(loc)),
            ("}", Command, Token::BlockEnd(loc)),
            ("[", Command, Token::GetItemStart(loc)),
            ("]", Command, Token::GetItemEnd(loc)),
            ("|", Command, Token::Pipe(loc)),
            ("@", Command, Token::Unnamed(loc)),
            ("@@", Command, Token::Named(loc)),
            ("(", Command, Token::ExprModeStart(loc)),
            ("for", Expression, Token::For(loc)),
            ("while", Expression, Token::While(loc)),
            ("loop", Expression, Token::Loop(loc)),
            ("if", Expression, Token::If(loc)),
            ("else", Expression, Token::Else(loc)),
            ("match", Expression, Token::Match(loc)),
            ("default", Expression, Token::Default(loc)),
            ("try", Expression, Token::Try(loc)),
            ("catch", Expression, Token::Catch(loc)),
            ("return", Expression, Token::Return(loc)),
            ("break", Expression, Token::Break(loc)),
            ("continue", Expression, Token::Continue(loc)),
            ("&", Command, Token::Background(loc)),
        ];

        for (src, mode, expected) in &cases {
            assert!(
                lexes_to(src, *mode, expected),
                "expected lexing `{}` starting in {:?} mode to produce a {:?}-shaped token",
                src,
                mode,
                expected
            );
        }

        // UnaryOperator is a real, declared token variant -- referenced by the grammar's
        // `extern` token block and handled defensively in a few match arms (see
        // highlight.rs, parser.rs, tracked_string.rs) -- but the lexer itself never
        // actually constructs one anywhere in either next_command or next_expr. It's
        // dead code today. Constructed directly here (not lexed) purely so the
        // exhaustive match below still has to account for it.
        let unreachable_unary_operator = Token::UnaryOperator("", loc);

        // Exhaustive match with no wildcard arm: fails to *compile* the moment a new
        // Token variant is added, until a case -- and, ideally, a lexing recipe in
        // `cases` above -- is added for it here.
        fn assert_accounted_for(t: &Token) {
            match t {
                Token::LogicalOperator(..)
                | Token::UnaryOperator(..)
                | Token::ComparisonOperator(..)
                | Token::Bang(..)
                | Token::Plus(..)
                | Token::Minus(..)
                | Token::Star(..)
                | Token::Slash(..)
                | Token::QuotedString(..)
                | Token::Comment(..)
                | Token::Identifier(..)
                | Token::Flag(..)
                | Token::QuotedFile(..)
                | Token::Glob(..)
                | Token::File(..)
                | Token::String(..)
                | Token::Regex(..)
                | Token::Integer(..)
                | Token::Float(..)
                | Token::Duration(..)
                | Token::MemberOperator(..)
                | Token::Equals(..)
                | Token::Declare(..)
                | Token::Separator(..)
                | Token::SubStart(..)
                | Token::SubEnd(..)
                | Token::BlockStart(..)
                | Token::BlockEnd(..)
                | Token::GetItemStart(..)
                | Token::GetItemEnd(..)
                | Token::Pipe(..)
                | Token::Unnamed(..)
                | Token::Named(..)
                | Token::ExprModeStart(..)
                | Token::For(..)
                | Token::While(..)
                | Token::Loop(..)
                | Token::If(..)
                | Token::Else(..)
                | Token::Match(..)
                | Token::Default(..)
                | Token::Try(..)
                | Token::Catch(..)
                | Token::Return(..)
                | Token::Break(..)
                | Token::Continue(..)
                | Token::Background(..) => {}
            }
        }
        assert_accounted_for(&unreachable_unary_operator);
        for (_, _, expected) in &cases {
            assert_accounted_for(expected);
        }
    }
}

impl<'a> Into<Spanned<'a>> for Token<'a> {
    fn into(self) -> Spanned<'a> {
        let loc = match &self {
            Token::LogicalOperator(_, l)
            | Token::UnaryOperator(_, l)
            | Token::QuotedString(_, l)
            | Token::String(_, l)
            | Token::Comment(_, l)
            | Token::File(_, l)
            | Token::Glob(_, l)
            | Token::Identifier(_, l)
            | Token::Flag(_, l)
            | Token::QuotedFile(_, l)
            | Token::Regex(_, l)
            | Token::Integer(_, l)
            | Token::ComparisonOperator(_, l)
            | Token::Float(_, l)
            | Token::Duration(_, l)
            | Token::MemberOperator(l)
            | Token::Equals(l)
            | Token::Declare(l)
            | Token::Separator(_, l)
            | Token::SubStart(l)
            | Token::SubEnd(l)
            | Token::BlockStart(l)
            | Token::BlockEnd(l)
            | Token::GetItemStart(l)
            | Token::GetItemEnd(l)
            | Token::Pipe(l)
            | Token::Unnamed(l)
            | Token::Named(l)
            | Token::Bang(l)
            | Token::Plus(l)
            | Token::Minus(l)
            | Token::Star(l)
            | Token::Slash(l)
            | Token::For(l)
            | Token::While(l)
            | Token::Loop(l)
            | Token::If(l)
            | Token::Else(l)
            | Token::Match(l)
            | Token::Default(l)
            | Token::Try(l)
            | Token::Catch(l)
            | Token::Return(l)
            | Token::Break(l)
            | Token::Continue(l)
            | Token::ExprModeStart(l)
            | Token::Background(l) => l,
        };
        Ok((loc.start, self, loc.end))
    }
}
