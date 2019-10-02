use crate::lexer::Lexer;

pub struct JobError {
    pub message: String,
}

impl JobError {
    pub fn parse_error(message: &String, lexer: &Lexer) -> JobError {
        return JobError {
            message: message.clone(),
        }
    }
}
