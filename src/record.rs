use encoding_rs::ISO_8859_15;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

pub const REG_SIZE: usize = 500;
pub type Reg = [u8; REG_SIZE];

pub fn write_num(buf: &mut Reg, begin: usize, end: usize, val: usize) {
    let size = end - begin + 1;
    let s = format!("{val:0size$}");
    buf[begin - 1..end].copy_from_slice(s.as_bytes());
}

pub fn write_str(buf: &mut Reg, begin: usize, end: usize, val: &str) {
    let size = end - begin + 1;
    let (encoded, _, _) = ISO_8859_15.encode(val);
    let slice = &mut buf[begin - 1..end];
    let len = encoded.len().min(size);
    slice[..len].copy_from_slice(&encoded[..len]);
    for b in &mut slice[len..] {
        *b = b' ';
    }
}

pub fn split_decimal(d: Decimal) -> (bool, usize, usize) {
    let neg = d.is_sign_negative();
    let int_part = d.trunc().abs().to_usize().unwrap_or(0);
    let mut frac = d.fract().abs();
    let _ = frac.set_scale(0);
    let frac_part = frac.to_usize().unwrap_or(0);
    (neg, int_part, frac_part)
}

pub fn write_decimal(buf: &mut Reg, sign_pos: usize, int_begin: usize, int_end: usize, frac_begin: usize, frac_end: usize, value: Decimal) {
    let (neg, int_part, frac_part) = split_decimal(value);
    if neg { write_str(buf, sign_pos, sign_pos, "N"); }
    write_num(buf, int_begin, int_end, int_part);
    write_num(buf, frac_begin, frac_end, frac_part);
}

pub fn read_field(line: &[u8], begin: usize, end: usize) -> String {
    String::from_utf8_lossy(&line[begin - 1..end]).trim().to_string()
}

pub fn read_field_raw(line: &[u8], begin: usize, end: usize) -> String {
    String::from_utf8_lossy(&line[begin - 1..end]).to_string()
}

pub fn read_decimal(line: &[u8], sign_pos: usize, int_begin: usize, int_end: usize, frac_begin: usize, frac_end: usize) -> String {
    let sign = if read_field(line, sign_pos, sign_pos) == "N" { "-" } else { "" };
    let int_part = read_field(line, int_begin, int_end);
    let frac_part = read_field(line, frac_begin, frac_end);
    let raw = format!("{}{}.{}", sign, int_part.trim_start_matches('0'), frac_part);
    if raw.starts_with('.') || raw.starts_with("-.") || raw == "." {
        raw.replacen('.', "0.", 1)
    } else {
        raw
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_write_num_zero_padded() {
        let mut buf: Reg = [b' '; REG_SIZE];
        write_num(&mut buf, 2, 4, 720);
        assert_eq!(&buf[1..4], b"720");
    }

    #[test]
    fn test_write_num_pads_leading_zeros() {
        let mut buf: Reg = [b' '; REG_SIZE];
        write_num(&mut buf, 5, 8, 24);
        assert_eq!(&buf[4..8], b"0024");
    }

    #[test]
    fn test_write_num_single_digit() {
        let mut buf: Reg = [b' '; REG_SIZE];
        write_num(&mut buf, 1, 1, 2);
        assert_eq!(buf[0], b'2');
    }

    #[test]
    fn test_write_str_pads_with_spaces() {
        let mut buf: Reg = [b'X'; REG_SIZE];
        write_str(&mut buf, 1, 5, "AB");
        assert_eq!(&buf[0..5], b"AB   ");
    }

    #[test]
    fn test_write_str_truncates_long_value() {
        let mut buf: Reg = [b' '; REG_SIZE];
        write_str(&mut buf, 1, 3, "ABCDEF");
        assert_eq!(&buf[0..3], b"ABC");
    }

    #[test]
    fn test_write_str_empty() {
        let mut buf: Reg = [b'X'; REG_SIZE];
        write_str(&mut buf, 1, 3, "");
        assert_eq!(&buf[0..3], b"   ");
    }

    #[test]
    fn test_write_str_iso8859_encoding() {
        let mut buf: Reg = [b' '; REG_SIZE];
        write_str(&mut buf, 1, 1, "Ñ");
        assert_eq!(buf[0], 209);
    }

    #[test]
    fn test_split_decimal_positive() {
        let d = Decimal::from_str("1234.56").unwrap();
        let (neg, int_part, frac) = split_decimal(d);
        assert!(!neg);
        assert_eq!(int_part, 1234);
        assert_eq!(frac, 56);
    }

    #[test]
    fn test_split_decimal_negative() {
        let d = Decimal::from_str("-99.01").unwrap();
        let (neg, int_part, frac) = split_decimal(d);
        assert!(neg);
        assert_eq!(int_part, 99);
        assert_eq!(frac, 1);
    }

    #[test]
    fn test_split_decimal_zero() {
        let (neg, int_part, frac) = split_decimal(Decimal::ZERO);
        assert!(!neg);
        assert_eq!(int_part, 0);
        assert_eq!(frac, 0);
    }

    #[test]
    fn test_read_field_trims_spaces() {
        let mut buf: Reg = [b' '; REG_SIZE];
        buf[0..3].copy_from_slice(b"AB ");
        assert_eq!(read_field(&buf, 1, 3), "AB");
    }

    #[test]
    fn test_read_field_all_spaces() {
        let buf: Reg = [b' '; REG_SIZE];
        assert_eq!(read_field(&buf, 1, 5), "");
    }

    #[test]
    fn test_write_decimal_positive() {
        let mut buf: Reg = [b' '; REG_SIZE];
        write_decimal(&mut buf, 1, 2, 13, 14, 15, Decimal::from_str("5000.99").unwrap());
        assert_eq!(read_field(&buf, 1, 1), "");
        assert_eq!(read_field(&buf, 2, 13), "000000005000");
        assert_eq!(read_field(&buf, 14, 15), "99");
    }

    #[test]
    fn test_write_decimal_negative() {
        let mut buf: Reg = [b' '; REG_SIZE];
        write_decimal(&mut buf, 1, 2, 13, 14, 15, Decimal::from_str("-1234.56").unwrap());
        assert_eq!(read_field(&buf, 1, 1), "N");
        assert_eq!(read_field(&buf, 2, 13), "000000001234");
        assert_eq!(read_field(&buf, 14, 15), "56");
    }

    #[test]
    fn test_read_decimal_positive() {
        let mut buf: Reg = [b' '; REG_SIZE];
        write_decimal(&mut buf, 1, 2, 13, 14, 15, Decimal::from_str("5000.99").unwrap());
        assert_eq!(read_decimal(&buf, 1, 2, 13, 14, 15), "5000.99");
    }

    #[test]
    fn test_read_decimal_negative() {
        let mut buf: Reg = [b' '; REG_SIZE];
        write_decimal(&mut buf, 1, 2, 13, 14, 15, Decimal::from_str("-1234.56").unwrap());
        assert_eq!(read_decimal(&buf, 1, 2, 13, 14, 15), "-1234.56");
    }

    #[test]
    fn test_read_decimal_zero() {
        let mut buf: Reg = [b' '; REG_SIZE];
        write_decimal(&mut buf, 1, 2, 13, 14, 15, Decimal::ZERO);
        assert_eq!(read_decimal(&buf, 1, 2, 13, 14, 15), "0.00");
    }
}
