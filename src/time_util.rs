use chrono::Duration;

pub fn duration_format(d: &Duration) -> String {
    const MICROS_IN_SECOND: i128 = 1_000_000_000;
    const MICROS_IN_MINUTE: i128 = MICROS_IN_SECOND * 60;
    const MICROS_IN_HOUR: i128 = MICROS_IN_MINUTE * 60;
    const MICROS_IN_DAY: i128 = MICROS_IN_HOUR * 24;
    const MICROS_IN_YEAR: i128 = MICROS_IN_DAY * 365;
    let mut remaining_nanos = d.num_nanoseconds()
        .map(|v| v as i128)
        .unwrap_or(
        d.num_microseconds()
            .map(|v| v as i128 * 1000)
            .unwrap_or(
                d.num_milliseconds() as i128 * 1000_000
            )
    );

    let mut res = "".to_string();

    let years = remaining_nanos / MICROS_IN_YEAR;
    if years != 0 {
        remaining_nanos -= years * MICROS_IN_YEAR;
        res.push_str(format!("{}y", years).as_str());
    }

    let days = remaining_nanos / MICROS_IN_DAY;
    if days != 0 || !res.is_empty() {
        remaining_nanos -= days * MICROS_IN_DAY;
        res.push_str(format!("{}d", days).as_str());
    }

    let hours = remaining_nanos / MICROS_IN_HOUR;
    if hours != 0 || !res.is_empty() {
        remaining_nanos -= hours * MICROS_IN_HOUR;
        res.push_str(format!("{}:", hours).as_str());
    }

    let minutes = remaining_nanos / MICROS_IN_MINUTE;
    if minutes != 0 || !res.is_empty() {
        remaining_nanos -= minutes * MICROS_IN_MINUTE;
        if res.is_empty() {
            res.push_str(format!("{}:", minutes).as_str());
        } else {
            res.push_str(format!("{:02}:", minutes).as_str());
        }
    }

    let seconds = remaining_nanos / MICROS_IN_SECOND;
    remaining_nanos -= seconds * MICROS_IN_SECOND;
    if res.is_empty() {
        res.push_str(format!("{}", seconds).as_str());
    } else {
        res.push_str(format!("{:02}", seconds).as_str());
    }

    if res.len() < 4 {
        if remaining_nanos != 0 {
            res.push_str(format!(".{:09}", remaining_nanos).trim_end_matches('0'))
        }
    }
    res
}
