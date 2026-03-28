use anyhow::{bail, Result};
use std::io::Write;
use std::fs;

use crate::builder::*;
use crate::model::{CsvRecord, validate};
use crate::record::*;

pub fn generate_to_writer(writer: &mut impl Write, records: &[CsvRecord]) -> Result<()> {
    if records.is_empty() { bail!("CSV has no records"); }
    for r in records { validate(r)?; }
    let nif = &records[0].nif;
    let name = &records[0].name;
    let contact_name = &records[0].contact_name;
    let phone = &records[0].phone;
    let year = records[0].year;
    let summary = build_summary(nif, name, contact_name, phone, year, records);
    writer.write_all(&summary)?;
    writer.write_all(b"\n")?;
    for r in records {
        writer.write_all(&build_detail(nif, name, year, r)?)?;
        writer.write_all(b"\n")?;
    }
    Ok(())
}

pub fn reverse_from_bytes(data: &[u8]) -> Result<(String, String, String, Vec<CsvRecord>)> {
    let lines: Vec<&[u8]> = data.split(|&b| b == b'\n').filter(|l| l.len() == REG_SIZE).collect();
    if lines.is_empty() { bail!("No valid 500-byte records found"); }
    let summary = lines.iter().find(|l| l[0] == b'1').expect("no summary record");
    let nif = read_field(summary, 9, 17);
    let name = read_field(summary, 18, 57);
    let contact_name = read_field(summary, 68, 107);
    let phone = read_field(summary, 59, 67).trim_start_matches('0').to_string();

    let mut records = Vec::new();
    for line in &lines {
        if line[0] != b'2' { continue; }
        records.push(CsvRecord {
            nif: nif.clone(),
            name: name.clone(),
            phone: phone.clone(),
            year: read_field(line, 5, 8).parse().unwrap_or(0),
            company_name: read_field(line, 190, 230),
            isin: read_field(line, 132, 143),
            country_code: read_field(line, 129, 130),
            first_acquisition_date: read_field(line, 415, 422),
            acquisition_type: read_field(line, 423, 423),
            value_in_euro: read_decimal(line, 432, 433, 444, 445, 446),
            quantity: {
                let qi = read_field(line, 463, 472).trim_start_matches('0').to_string();
                let qf = read_field(line, 473, 474);
                format!("{}.{}", if qi.is_empty() { "0" } else { &qi }, qf)
            },
            percentage: {
                let pi = read_field(line, 476, 478).trim_start_matches('0').to_string();
                let pf = read_field(line, 479, 480);
                format!("{}.{}", if pi.is_empty() { "0" } else { &pi }, pf)
            },
            contact_name: contact_name.clone(),
            asset_type: read_field(line, 102, 102),
            asset_subtype: read_field(line, 103, 103),
            stock_id_type: read_field(line, 131, 131),
            account_id_type: read_field(line, 144, 144),
            account_id: read_field(line, 145, 155),
            account_code: read_field(line, 156, 189),
            entity_address: read_field_raw(line, 251, 412).trim_end().to_string(),
            entity_country_code: read_field(line, 413, 414),
            stock_representation: read_field(line, 462, 462),
            valuation_value: read_decimal(line, 447, 448, 459, 460, 461),
        });
    }
    Ok((nif, contact_name, phone, records))
}

pub fn generate(csv_path: &str, output: Option<&str>) -> Result<()> {
    let mut rdr = csv::Reader::from_path(csv_path)?;
    let records: Vec<CsvRecord> = rdr.deserialize().collect::<Result<_, _>>()?;
    let output_path = output.map(String::from).unwrap_or_else(|| {
        if records.is_empty() { "output.720".into() } else { format!("{}.720", records[0].nif) }
    });
    let mut out = Vec::with_capacity(REG_SIZE * (records.len() + 1) + records.len() + 1);
    generate_to_writer(&mut out, &records)?;
    fs::write(&output_path, &out)?;
    println!("Written {} ({} detail records)", output_path, records.len());
    Ok(())
}

const CSV_COLUMNS: [&str; 23] = [
    "nif","name","phone","year","company_name","isin","country_code",
    "first_acquisition_date","acquisition_type","value_in_euro","quantity","percentage",
    "contact_name","asset_type","asset_subtype","stock_id_type",
    "account_id_type","account_id","account_code",
    "entity_address","entity_country_code","stock_representation","valuation_value",
];

pub fn reverse(input_720: &str, output: Option<&str>) -> Result<()> {
    let data = fs::read(input_720)?;
    let (_nif, _contact_name, _phone, records) = reverse_from_bytes(&data)?;
    let output_path = output.map(String::from).unwrap_or_else(|| input_720.replace(".720", ".csv"));
    let mut wtr = csv::Writer::from_writer(Vec::new());
    wtr.write_record(CSV_COLUMNS)?;
    for r in &records {
        wtr.write_record([
            &r.nif, &r.name, &r.phone, &r.year.to_string(),
            &r.company_name, &r.isin, &r.country_code,
            &r.first_acquisition_date, &r.acquisition_type, &r.value_in_euro,
            &r.quantity, &r.percentage, &r.contact_name,
            &r.asset_type, &r.asset_subtype, &r.stock_id_type,
            &r.account_id_type, &r.account_id, &r.account_code,
            &r.entity_address, &r.entity_country_code, &r.stock_representation,
            &r.valuation_value,
        ])?;
    }
    wtr.flush()?;
    fs::write(&output_path, wtr.into_inner()?)?;
    println!("Written {} ({} records)", output_path, records.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_records() -> Vec<CsvRecord> {
        vec![
            CsvRecord {
                nif: "12345678A".into(),
                name: "GARCIA LOPEZ JUAN".into(),
                phone: "612345678".into(),
                year: 2024,
                company_name: "Apple Inc".into(),
                isin: "US0378331005".into(),
                country_code: "US".into(),
                first_acquisition_date: "20230115".into(),
                acquisition_type: "A".into(),
                value_in_euro: "15234.56".into(),
                quantity: "100.00".into(),
                percentage: "100.00".into(),
                ..Default::default()
            },
            CsvRecord {
                nif: "12345678A".into(),
                name: "GARCIA LOPEZ JUAN".into(),
                phone: "612345678".into(),
                year: 2024,
                company_name: "Microsoft Corp".into(),
                isin: "US5949181045".into(),
                country_code: "US".into(),
                first_acquisition_date: "20230301".into(),
                acquisition_type: "A".into(),
                value_in_euro: "8750.25".into(),
                quantity: "50.50".into(),
                percentage: "100.00".into(),
                ..Default::default()
            },
        ]
    }

    #[test]
    fn test_generate_to_writer() {
        let records = sample_records();
        let mut buf = Vec::new();
        generate_to_writer(&mut buf, &records).unwrap();
        let lines: Vec<&[u8]> = buf.split(|&b| b == b'\n').filter(|l| l.len() == REG_SIZE).collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0][0], b'1');
        assert_eq!(lines[1][0], b'2');
        assert_eq!(lines[2][0], b'2');
    }

    #[test]
    fn test_generate_empty_records() {
        let mut buf = Vec::new();
        assert!(generate_to_writer(&mut buf, &[]).is_err());
    }

    #[test]
    fn test_reverse_from_bytes() {
        let records = sample_records();
        let mut buf = Vec::new();
        generate_to_writer(&mut buf, &records).unwrap();
        let (_nif, _cn, _phone, reversed) = reverse_from_bytes(&buf).unwrap();
        assert_eq!(reversed.len(), 2);
        assert_eq!(reversed[0].value_in_euro, "15234.56");
        assert_eq!(reversed[1].value_in_euro, "8750.25");
        assert_eq!(reversed[0].isin, "US0378331005");
        assert_eq!(reversed[1].quantity, "50.50");
    }

    #[test]
    fn test_roundtrip_in_memory() {
        let records = sample_records();
        let mut buf = Vec::new();
        generate_to_writer(&mut buf, &records).unwrap();
        let (_, _, _, reversed) = reverse_from_bytes(&buf).unwrap();
        assert_eq!(reversed[0].company_name, "APPLE INC");
        assert_eq!(reversed[1].company_name, "MICROSOFT CORP");
        assert_eq!(reversed[0].first_acquisition_date, "20230115");
        assert_eq!(reversed[1].first_acquisition_date, "20230301");
        assert_eq!(reversed[0].percentage, "100.00");
    }
}
