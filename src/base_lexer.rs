use regex::Regex;
use std::collections::HashSet;
use std::hash::Hash;

pub struct BaseLexer<T: 'static + Copy + Clone> {
    input: String,
    idx: usize,
    peeked: Option<(T, usize, usize)>,
    lex_data: &'static Vec<(T, Regex)>,
    error_type: T,
    eof_type: T,
    ignored: &'static HashSet<T>,
}

impl<T: 'static + Copy + Clone + Eq + Hash> BaseLexer<T> {
    pub fn construct(input: &str, lex_data: &'static Vec<(T, Regex)>, error_type: T, eof_type: T, ignored: &'static HashSet<T>) -> BaseLexer<T> {
        return BaseLexer {
            input: input.to_string(),
            idx: 0,
            peeked: None,
            lex_data,
            error_type,
            eof_type,
            ignored,
        };
    }

    fn next_of_any(&mut self) -> (T, usize, usize) {
        let mut max_len = 0;
        let mut token_type = None;
        if self.idx >= self.input.len() {
            return (self.eof_type, 0, 0);
        }
        for (token, re) in self.lex_data.iter() {
            //let re = Regex::new(r".");
            match re.find(&self.input[self.idx..]) {
                Some(mat) => {
                    if mat.end() > max_len {
                        max_len = mat.end();
                        token_type = Some(token.clone());
                    }
                }
                None => {}
            }
        }
        if let Some(tt) = token_type {
            self.idx += max_len;
            return (tt, self.idx - max_len, self.idx);
        }
        return (self.error_type, 0, 0);
    }

    pub fn peek(&mut self) -> (T, &str) {
        let (tt, from, to) = self.next_span();
        self.peeked = Some((tt, from, to));
        return (tt, &self.input[from..to]);
    }

    fn next_span(&mut self) -> (T, usize, usize) {
        let s = self.peeked;
        match s {
            None => {
                loop {
                    let (token_type, from, to) = self.next_of_any();
                    if self.ignored.contains(&token_type) {
                        continue;
                    }
                    return (token_type, from, to);
                }
            }

            Some(val) => {
                self.peeked = None;
                return val;
            }
        }
    }

    pub fn pop<'a> (&'a mut self) -> (T, &'a str) {
        let (tt, from, to) = self.next_span();
        return (tt, &self.input[from..to]);
    }
}
