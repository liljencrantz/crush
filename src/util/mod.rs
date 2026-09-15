pub mod byte_unit;
pub mod directory_lister;
pub mod display_non_recursive;
pub mod escape;
pub mod file;
pub mod glob;
pub mod hex;
pub mod highlight;
pub mod identity_arc;
pub mod integer_formater;
pub mod logins;
pub mod md;
pub mod regex;
pub mod replace;
pub mod repr;
pub mod temperature;
pub mod time;
pub mod user_map;
pub mod env;

/// Escapes text for safe embedding between HTML tags or inside a
/// double-quoted HTML attribute value.
pub fn html_escape(s: &str) -> String {
    let mut res = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => res.push_str("&amp;"),
            '<' => res.push_str("&lt;"),
            '>' => res.push_str("&gt;"),
            '"' => res.push_str("&quot;"),
            '\'' => res.push_str("&#39;"),
            _ => res.push(c),
        }
    }
    res
}
