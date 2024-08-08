use std::fmt::{Display, Formatter};
use num_format::Grouping;
use crate::lang::errors::{byte_unit_error, CrushError};
use crate::util::byte_unit::ByteUnit::{Binary, Decimal, Raw};
use crate::util::integer_formater::format_integer;

#[derive(Copy, Clone)]
pub enum ByteUnit {
    Binary,
    Decimal,
    Raw,
}

impl TryFrom<&str> for ByteUnit {
    type Error = CrushError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "binary" => Ok(Binary),
            "decimal" => Ok(Decimal),
            "raw" => Ok(Raw),
            _ => byte_unit_error(s),
        }
    }
}

impl Display for ByteUnit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Binary => f.write_str("binary"),
            Decimal => f.write_str("decimal"),
            Raw => f.write_str("raw"),
        }
    }
}

impl ByteUnit {
    pub fn units() -> &'static [ByteUnit] {
        &[Binary, Decimal, Raw]
    }

    pub fn format(&self, size: i128, grouping: Grouping) -> String {
        match self {
            Decimal => format_size(size, 1, 1000, &["B", "kB", "MB", "GB", "TB", "PB"]),
            Binary => format_size(size, 1, 1024, &["B", "kiB", "MiB", "GiB", "TiB", "PiB"]),
            Raw => format_integer(size, grouping),
        }
    }
}

fn format_size(numerator: i128, denominator: i128, multiplier: i128, prefixes: &[&str]) -> String {
    if numerator / denominator > multiplier && prefixes.len() > 1 {
        format_size(numerator, denominator * multiplier, multiplier, &prefixes[1..])
    } else {
        if denominator == 1 {
            format!("{} {}", numerator, prefixes[0])
        } else {
            let sz = (numerator as f64) / (denominator as f64);
            let dec = 4 - (sz as usize).to_string().len();
            format!("{:.*} {}", dec, sz, prefixes[0])
        }
    }
}
