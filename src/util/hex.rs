use crate::lang::errors::{CrushResult, data_error};

pub fn to_hex(v: u8) -> String {
    let arr = vec![
        "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "a", "b", "c", "d", "e", "f",
    ];
    format!("{}{}", arr[(v >> 4) as usize], arr[(v & 15) as usize])
}

fn val(c: u8) -> CrushResult<u8> {
    match c {
        b'A'..=b'F' => Ok(c - b'A' + 10),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'0'..=b'9' => Ok(c - b'0'),
        _ => data_error(format!("Invalid character in hew sttring character {}", c)),
    }
}

pub fn from_hex(v: &str) -> CrushResult<Vec<u8>> {
    if v.len() % 2 != 0 {
        return data_error("Hex value with odd number of characters");
    }

    v.as_bytes()
        .chunks(2)
        .map(|pair| { Ok(val(pair[0])? << 4 | val(pair[1])?)
    }).collect()
}
