use crate::lexer::Lexer;

pub struct JobError {
    pub message: String,
}

pub fn parse_error(message: &str, lexer: &Lexer) -> JobError {
    return JobError {
        message: String::from(message),
    };
}
