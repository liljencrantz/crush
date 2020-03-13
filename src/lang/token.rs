

pub struct Error {
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token{
    OpenParen,
    CloseParen,
    Number,
}
