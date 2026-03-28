use anyhow::Result;
use rust_decimal::Decimal;
use std::str::FromStr;

use crate::model::CsvRecord;
use crate::record::*;

pub fn build_summary(nif: &str, name: &str, contact_name: &str, phone: &str, year: usize, records: &[CsvRecord]) -> Reg {
    let mut f: Reg = [b' '; REG_SIZE];
    write_num(&mut f, 1, 1, 1);
    write_num(&mut f, 2, 4, 720);
    write_num(&mut f, 5, 8, year);
    write_str(&mut f, 9, 17, nif);
    write_str(&mut f, 18, 57, name);
    write_str(&mut f, 58, 58, "T");
    write_num(&mut f, 59, 67, phone.parse().unwrap_or(0));
    let cn = if contact_name.is_empty() { name } else { contact_name };
    write_str(&mut f, 68, 107, cn);
    write_num(&mut f, 108, 110, 720);
    write_num(&mut f, 111, 120, 0);
    write_num(&mut f, 123, 135, 0);
    write_num(&mut f, 136, 144, records.len());
    let mut total = Decimal::ZERO;
    for r in records {
        total += Decimal::from_str(&r.value_in_euro).unwrap_or(Decimal::ZERO);
    }
    write_decimal(&mut f, 145, 146, 160, 161, 162, total);
    write_num(&mut f, 164, 178, 0);
    write_num(&mut f, 179, 180, 0);
    f
}

pub fn build_detail(nif: &str, name: &str, year: usize, r: &CsvRecord) -> Result<Reg> {
    let mut f: Reg = [b' '; REG_SIZE];
    write_num(&mut f, 1, 1, 2);
    write_num(&mut f, 2, 4, 720);
    write_num(&mut f, 5, 8, year);
    write_str(&mut f, 9, 17, nif);
    write_str(&mut f, 18, 26, nif);
    write_str(&mut f, 36, 75, name);
    write_num(&mut f, 76, 76, 1);

    let asset_type = if r.asset_type.is_empty() { "V" } else { &r.asset_type };
    write_str(&mut f, 102, 102, asset_type);
    let asset_subtype: usize = r.asset_subtype.parse().unwrap_or(1);
    write_num(&mut f, 103, 103, asset_subtype);

    write_str(&mut f, 129, 130, &r.country_code);
    let stock_id_type: usize = r.stock_id_type.parse().unwrap_or(1);
    write_num(&mut f, 131, 131, stock_id_type);
    write_str(&mut f, 132, 143, &r.isin);

    if !r.account_id_type.is_empty() { write_str(&mut f, 144, 144, &r.account_id_type); }
    if !r.account_id.is_empty() { write_str(&mut f, 145, 155, &r.account_id); }
    if !r.account_code.is_empty() { write_str(&mut f, 156, 189, &r.account_code); }

    write_str(&mut f, 190, 230, &r.company_name.to_uppercase());

    if !r.entity_address.is_empty() { write_str(&mut f, 251, 412, &r.entity_address); }

    let ec = if r.entity_country_code.is_empty() {
        if r.isin.len() >= 2 { r.isin[0..2].to_string() } else { String::new() }
    } else {
        r.entity_country_code.clone()
    };
    if !ec.is_empty() { write_str(&mut f, 413, 414, &ec); }

    write_num(&mut f, 415, 422, r.first_acquisition_date.parse().unwrap_or(0));
    write_str(&mut f, 423, 423, if r.acquisition_type.is_empty() { "A" } else { &r.acquisition_type });
    write_num(&mut f, 424, 431, 0);

    write_decimal(&mut f, 432, 433, 444, 445, 446, Decimal::from_str(&r.value_in_euro)?);

    let vval = if !r.valuation_value.is_empty() {
        Decimal::from_str(&r.valuation_value)?
    } else {
        Decimal::ZERO
    };
    write_decimal(&mut f, 447, 448, 459, 460, 461, vval);

    if !r.stock_representation.is_empty() { write_str(&mut f, 462, 462, &r.stock_representation); }

    let qty = Decimal::from_str(&r.quantity)?;
    let (_, qi, qf) = split_decimal(qty);
    write_num(&mut f, 463, 472, qi);
    write_num(&mut f, 473, 474, qf);

    let pct = Decimal::from_str(&r.percentage)?;
    let (_, pi, pf) = split_decimal(pct);
    write_num(&mut f, 476, 478, pi);
    write_num(&mut f, 479, 480, pf);
    Ok(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_record(value: &str) -> CsvRecord {
        CsvRecord {
            nif: "12345678A".into(),
            value_in_euro: value.into(),
            quantity: "10.00".into(),
            ..Default::default()
        }
    }

    #[test]
    fn test_summary_register_type() {
        let records = vec![make_record("1000.50")];
        let s = build_summary("12345678A", "TEST USER", "", "600000000", 2024, &records);
        assert_eq!(s[0], b'1');
        assert_eq!(&s[1..4], b"720");
    }

    #[test]
    fn test_summary_year_and_nif() {
        let records = vec![make_record("500.00")];
        let s = build_summary("12345678A", "TEST USER", "", "600000000", 2024, &records);
        assert_eq!(&s[4..8], b"2024");
        assert_eq!(read_field(&s, 9, 17), "12345678A");
    }

    #[test]
    fn test_summary_record_count() {
        let records = vec![make_record("100.00"), make_record("200.00")];
        let s = build_summary("12345678A", "TEST", "", "600000000", 2024, &records);
        assert_eq!(read_field(&s, 136, 144), "000000002");
    }

    #[test]
    fn test_summary_total_acquisition() {
        let records = vec![make_record("1000.50"), make_record("2000.25")];
        let s = build_summary("12345678A", "TEST", "", "600000000", 2024, &records);
        assert_eq!(read_field(&s, 146, 160), "000000000003000");
        assert_eq!(read_field(&s, 161, 162), "75");
    }

    #[test]
    fn test_summary_size() {
        let records = vec![make_record("100.00")];
        let s = build_summary("12345678A", "TEST", "", "600000000", 2024, &records);
        assert_eq!(s.len(), REG_SIZE);
    }

    #[test]
    fn test_detail_register_type() {
        let r = make_record("5000.99");
        let d = build_detail("12345678A", "TEST USER", 2024, &r).unwrap();
        assert_eq!(d[0], b'2');
        assert_eq!(&d[1..4], b"720");
    }

    #[test]
    fn test_detail_fields() {
        let r = make_record("5000.99");
        let d = build_detail("12345678A", "TEST USER", 2024, &r).unwrap();
        assert_eq!(read_field(&d, 129, 130), "US");
        assert_eq!(read_field(&d, 132, 143), "US1234567890");
        assert_eq!(read_field(&d, 190, 230), "ACME");
        assert_eq!(read_field(&d, 415, 422), "20230101");
        assert_eq!(read_field(&d, 423, 423), "A");
    }

    #[test]
    fn test_detail_value() {
        let r = make_record("5000.99");
        let d = build_detail("12345678A", "TEST USER", 2024, &r).unwrap();
        assert_eq!(read_field(&d, 433, 444), "000000005000");
        assert_eq!(read_field(&d, 445, 446), "99");
        assert_eq!(read_field(&d, 432, 432), "");
    }

    #[test]
    fn test_detail_negative_value() {
        let r = make_record("-1234.56");
        let d = build_detail("12345678A", "TEST USER", 2024, &r).unwrap();
        assert_eq!(read_field(&d, 432, 432), "N");
        assert_eq!(read_field(&d, 433, 444), "000000001234");
        assert_eq!(read_field(&d, 445, 446), "56");
    }

    #[test]
    fn test_detail_quantity_and_percentage() {
        let r = make_record("100.00");
        let d = build_detail("12345678A", "TEST USER", 2024, &r).unwrap();
        assert_eq!(read_field(&d, 463, 472), "0000000010");
        assert_eq!(read_field(&d, 473, 474), "00");
        assert_eq!(read_field(&d, 476, 478), "100");
        assert_eq!(read_field(&d, 479, 480), "00");
    }

    #[test]
    fn test_detail_size() {
        let r = make_record("100.00");
        let d = build_detail("12345678A", "TEST USER", 2024, &r).unwrap();
        assert_eq!(d.len(), REG_SIZE);
    }

    #[test]
    fn test_detail_stock_type_v() {
        let r = make_record("100.00");
        let d = build_detail("12345678A", "TEST USER", 2024, &r).unwrap();
        assert_eq!(read_field(&d, 102, 102), "V");
        assert_eq!(read_field(&d, 462, 462), "");
    }

    #[test]
    fn test_detail_entity_country_from_isin() {
        let r = make_record("100.00");
        let d = build_detail("12345678A", "TEST USER", 2024, &r).unwrap();
        assert_eq!(read_field(&d, 413, 414), "US");
    }
}
