use std::str::Chars;
use std::iter::Peekable;

pub fn glob(g: &str, v: &str) -> bool {
    return glob_match(&mut g.chars(), &mut v.chars().peekable());
}

fn glob_match(glob: &mut Chars, value: &mut Peekable<Chars>) -> bool {
    match (glob.next(), value.peek()) {
        (None, None) => return true,
        (None, Some(_)) => return false,
        (Some('*'), _) => {
            let mut i = value.clone();
            loop {
                match i.peek() {
                    Some(_) => {
                        if glob_match(&mut glob.clone(), &mut i.clone()) {
                            return true;
                        }
                        i.next();
                    }
                    None => {
                        if glob_match(&mut glob.clone(), &mut i.clone()) {
                            return true;
                        }
                        break;
                    }
                }
            }
        }
        (Some('?'), Some(v)) => {
            value.next();
            return glob_match(glob, value);
        }
        (Some(g), Some(v)) => {
            if g == *v {
                value.next();
                return glob_match(glob, value);
            }
        }
        (Some(_), None) => {}
    }
    return false;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_that_globs_match_themselves() {
        assert!(glob("foo.txt", "foo.txt"));
        assert!(glob("", ""));
        assert!(!glob("foo", "bar"));
    }

    #[test]
    fn test_that_basic_wildcards_work() {
        assert!(glob("*.txt", "foo.txt"));
        assert!(!glob("*.txt", "foo.txb"));
        assert!(!glob("*.txt", "footxt"));
    }

    #[test]
    fn test_that_single_character_wildcards_work() {
        assert!(glob("??.txt", "aa.txt"));
        assert!(!glob("??.txt", "aaa.txt"));
        assert!(glob("???", "aaa"));
        assert!(glob("?", "a"));
    }

    #[test]
    fn test_that_wildcards_work_at_the_end() {
        assert!(glob("*", "aaa"));
        assert!(glob("aaa*", "aaa"));
        assert!(glob("aaa*", "aaaa"));
        assert!(glob("aaa*", "aaab"));
        assert!(glob("aaa*?", "aaab"));
        assert!(glob("aaa*?", "aaaab"));
        assert!(glob("*a*", "aaaa"));
        assert!(!glob("*a*", "bbb"));
    }

    #[test]
    fn test_that_multiple_wildcards_work() {
        assert!(glob("a*b*c", "abc"));
        assert!(glob("a*b*c?", "aabcc"));
        assert!(!glob("a*b*c?", "acb"));
    }
}
