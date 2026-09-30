use regex::Regex;
use std::sync::LazyLock;

macro_rules! re {
    ($name:ident, $pat:expr) => {
        static $name: LazyLock<Regex> = LazyLock::new(|| Regex::new($pat).unwrap());
    };
}

fn ascii_digits(s: &str) -> String {
    s.chars()
        .map(|c| if ('０'..='９').contains(&c) { char::from_u32(c as u32 - '０' as u32 + '0' as u32).unwrap() } else { c })
        .collect()
}

fn to_int(s: &str) -> i64 {
    s.parse::<u128>().map(|v| v.min(i64::MAX as u128) as i64).unwrap_or(i64::MAX)
}

re!(WS, r"\s+");
re!(PCT, r"([0-9]{1,3}(?:\.[0-9]{1,3})?)\|?%");
re!(BRACKET, r"\[\|?([0-9]{1,3}\.[0-9]{1,3})");
re!(SPLIT, r"[\]/]");
re!(NUM, r"[0-9]+");
re!(FRAC, r"([0-9]+)/([0-9]+)");
re!(HPMP_END, r"(?i)(HP|MP)\W*$");

pub fn parse_exp(text: &str) -> Option<(i64, f64)> {
    let t = ascii_digits(&text.replace(',', "").replace('，', "").replace('．', "."));
    let t = WS.replace_all(&t, "|").to_string();
    if t.contains('%') || t.contains('[') {
        let m = PCT
            .captures_iter(&t)
            .find(|c| c[1].parse::<f64>().map_or(false, |v| v <= 100.0))
            .or_else(|| BRACKET.captures(&t))?;
        let pct: f64 = m[1].parse().ok()?;
        let start = m.get(0).unwrap().start();
        let seg = SPLIT.split(&t[..start]).last().unwrap_or("");
        let nums: Vec<&str> = NUM.find_iter(seg).map(|m| m.as_str()).collect();
        let best = nums.iter().fold(None::<&str>, |b, n| match b {
            Some(x) if x.len() >= n.len() => Some(x),
            _ => Some(n),
        })?;
        return Some((to_int(best), pct));
    }
    for c in FRAC.captures_iter(&t) {
        let s = c.get(0).unwrap().start();
        if HPMP_END.is_match(&t[..s]) {
            continue;
        }
        let (e, n) = (to_int(&c[1]), to_int(&c[2]));
        if 0 < n && e <= n {
            return Some((e, e as f64 * 100.0 / n as f64));
        }
    }
    None
}

re!(MESO_PLAIN, r"^[0-9]+$");
re!(MESO_GROUPS, r"^[0-9]{1,3}(?:[,.:;'’`|][0-9]{3})+$");

pub fn parse_meso(text: &str) -> Option<i64> {
    let t = ascii_digits(&text.replace('，', ",").replace('．', "."));
    let t: String = WS.replace_all(&t, "").to_string();
    let t = t.trim_matches(|c| ":;.,'’`|".contains(c));
    if MESO_PLAIN.is_match(t) {
        return Some(to_int(t));
    }
    if MESO_GROUPS.is_match(t) {
        return Some(to_int(&t.chars().filter(|c| c.is_ascii_digit()).collect::<String>()));
    }
    None
}

pub fn fix_digits(t: &str) -> String {
    t.trim()
        .chars()
        .map(|c| match c {
            '一' | '丨' | '|' | 'l' | 'I' => '1',
            'O' | 'o' => '0',
            c => c,
        })
        .collect()
}

re!(BAG, r"^[1-9][0-9]*$");

pub fn bag_count(t: &str) -> String {
    let t = ascii_digits(t);
    let t = t.trim_matches(|c| ".,:;。，、· ".contains(c)).replace(',', "");
    if BAG.is_match(&t) { t } else { String::new() }
}

pub fn parse_count(text: &str) -> Option<i64> {
    let t = ascii_digits(&text.replace(',', "").replace('，', ""));
    let cs: Vec<char> = t.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < cs.len() {
        if cs[i] == 'F' {
            let mut j = i + 1;
            while j < cs.len() && cs[j].is_ascii_digit() {
                j += 1;
            }
            let n = j - i - 1;
            if (1..=2).contains(&n) {
                out.push(' ');
                i = j;
                continue;
            }
        }
        out.push(cs[i]);
        i += 1;
    }
    NUM.find_iter(&out).last().map(|m| to_int(m.as_str()))
}

re!(INPUT, r"^[0-9]+(?:\.[0-9]+)?(?:[+*][0-9]+(?:\.[0-9]+)?)*$");

pub fn parse_input(text: &str) -> Result<Option<f64>, ()> {
    let t: String = text
        .replace([',', '，', '%', ' '], "")
        .replace(['x', 'X', '×'], "*")
        .replace('＋', "+");
    let t = ascii_digits(&t);
    if t.is_empty() {
        return Ok(None);
    }
    if !INPUT.is_match(&t) {
        return Err(());
    }
    let total: f64 = t.split('+').map(|term| term.split('*').map(|x| x.parse::<f64>().unwrap()).product::<f64>()).sum();
    Ok(Some(total))
}

pub fn comma(n: i64) -> String {
    let s = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

pub fn comma_signed(n: i64) -> String {
    if n >= 0 { format!("+{}", comma(n)) } else { comma(n) }
}

pub fn comma_f(v: f64) -> String {
    let r = v.round_ties_even();
    if r == 0.0 && v.is_sign_negative() && v != 0.0 {
        return "-0".into();
    }
    comma(r as i64)
}

pub fn short(n: Option<f64>) -> String {
    let Some(n) = n else { return "–".into() };
    if !n.is_finite() {
        return "–".into();
    }
    let a = n.abs();
    if a >= 1e8 {
        format!("{:.2}億", n / 1e8)
    } else if a >= 1e6 {
        format!("{:.0}萬", n / 1e4)
    } else if a >= 1e4 {
        format!("{:.1}萬", n / 1e4)
    } else {
        comma_f(n)
    }
}

pub fn dur(sec: Option<f64>) -> String {
    let Some(sec) = sec else { return "–".into() };
    if sec.is_nan() || sec < 0.0 || sec.is_infinite() {
        return "–".into();
    }
    let m = (sec / 60.0).floor() as i64;
    if m < 60 {
        return format!("{m}分");
    }
    let h = m / 60;
    if h < 48 {
        return format!("{h}時{:02}分", m % 60);
    }
    format!("{}天{}時", h / 24, h % 24)
}

pub fn hms(sec: f64) -> String {
    let s = sec.max(0.0) as i64;
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

pub fn pround(v: f64) -> i64 {
    v.round_ties_even() as i64
}

pub fn fmt_g(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e16 { format!("{}", v as i64) } else { format!("{v}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exp() {
        assert_eq!(parse_exp("47910210[29.24% ]"), Some((47910210, 29.24)));
        assert_eq!(parse_exp("HP[7897/7897] 101868263[65.58% ]"), Some((101868263, 65.58)));
        assert_eq!(parse_exp("12345/67890"), Some((12345, 12345.0 * 100.0 / 67890.0)));
        assert_eq!(parse_exp("HP 500/600"), None);
        assert_eq!(parse_exp("abc"), None);
        assert_eq!(parse_exp("25193920[15, 37%"), Some((25193920, 37.0)));
        assert_eq!(parse_exp("25193920[15,37%"), None);
    }

    #[test]
    fn counts() {
        assert_eq!(parse_count("F12 2907"), Some(2907));
        assert_eq!(parse_count("F123"), Some(123));
        assert_eq!(parse_count("1 620"), Some(620));
        assert_eq!(bag_count("3000."), "3000");
        assert_eq!(bag_count("0300"), "");
        assert_eq!(fix_digits(" l0O "), "100");
        assert_eq!(parse_meso("107,197,139"), Some(107197139));
        assert_eq!(parse_meso("111,12"), None);
        assert_eq!(parse_meso("104.305"), Some(104305));
        assert_eq!(parse_input("3000*2+1184"), Ok(Some(7184.0)));
        assert_eq!(parse_input("45.6%"), Ok(Some(45.6)));
        assert_eq!(parse_input("abc"), Err(()));
        assert_eq!(parse_input(""), Ok(None));
    }

    #[test]
    fn fmt() {
        assert_eq!(comma(1234567), "1,234,567");
        assert_eq!(comma(-1234), "-1,234");
        assert_eq!(comma_signed(13548), "+13,548");
        assert_eq!(short(Some(123456789.0)), "1.23億");
        assert_eq!(short(Some(4567890.0)), "457萬");
        assert_eq!(short(Some(78000.0)), "7.8萬");
        assert_eq!(dur(Some(3725.0)), "1時02分");
        assert_eq!(hms(3725.9), "01:02:05");
    }
}
