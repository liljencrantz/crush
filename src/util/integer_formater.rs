use num_format::Grouping;

pub fn format_integer(i: i128, grouping: Grouping) -> String {
    match grouping {
        Grouping::Standard => {
            let whole = i.to_string();
            let mut rest = whole.as_str();
            let mut res = String::new();
            if i < 0 {
                res.push('-');
                rest = &rest[1..];
            }
            loop {
                if rest.len() <= 3 {
                    break;
                }
                let split = ((rest.len() - 1) % 3) + 1;
                res.push_str(&rest[0..split]);
                res.push('_');
                rest = &rest[split..];
            }
            res.push_str(rest);
            res
        }
        Grouping::Indian => {
            let whole = i.to_string();
            let mut rest = whole.as_str();
            let mut res = String::new();
            if i < 0 {
                res.push('-');
                rest = &rest[1..];
            }
            loop {
                if rest.len() <= 3 {
                    break;
                }
                let split = 1 + rest.len() % 2;
                res.push_str(&rest[0..split]);
                res.push('_');
                rest = &rest[split..];
            }
            res.push_str(rest);
            res
        }
        Grouping::Posix => i.to_string(),
    }
}