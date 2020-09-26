pub fn escape_without_quotes(s: &str) -> String {
    let mut res = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\"' => res += "\\\"",
            '\n' => res += "\\n",
            '\r' => res += "\\r",
            '\t' => res += "\\t",
            _ => if c < '\x20' {
                res.push_str(&format!("\\x{:02}", u32::from(c)));
            } else {
                res.push(c);
            },
        }
    }
    res
}

pub fn escape(s: &str) -> String {
    let mut res = "\"".to_string();
    res += &escape_without_quotes(s);
    res += "\"";
    res
}

pub fn unescape(s: &str) -> String {
    let mut res = "".to_string();
    let mut was_backslash = false;
    for c in s[1..s.len() - 1].chars() {
        if was_backslash {
            match c {
                'n' => res += "\n",
                'r' => res += "\r",
                't' => res += "\t",
                _ => res += &c.to_string(),
            }
            was_backslash = false;
        } else {
            if c == '\\' {
                was_backslash = true;
            } else {
                res += &c.to_string();
            }
        }
    }
    res
}
