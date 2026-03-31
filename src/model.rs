use anyhow::{bail, Result};
use rust_decimal::Decimal;
use serde::Deserialize;
use std::str::FromStr;

#[derive(Debug, Deserialize)]
pub struct CsvRecord {
    pub nif: String,
    pub name: String,
    pub phone: String,
    pub year: usize,
    pub company_name: String,
    pub isin: String,
    pub country_code: String,
    pub first_acquisition_date: String,
    pub acquisition_type: String,
    pub value_in_euro: String,
    pub quantity: String,
    pub percentage: String,
    #[serde(default)]
    pub contact_name: String,
    #[serde(default)]
    pub asset_type: String,
    #[serde(default)]
    pub asset_subtype: String,
    #[serde(default)]
    pub stock_id_type: String,
    #[serde(default)]
    pub account_id_type: String,
    #[serde(default)]
    pub account_id: String,
    #[serde(default)]
    pub account_code: String,
    #[serde(default)]
    pub entity_address: String,
    #[serde(default)]
    pub entity_country_code: String,
    #[serde(default)]
    pub stock_representation: String,
    #[serde(default)]
    pub valuation_value: String,
}

impl Default for CsvRecord {
    fn default() -> Self {
        Self {
            nif: "00000000A".into(),
            name: "TEST USER".into(),
            phone: "600000000".into(),
            year: 2024,
            company_name: "ACME".into(),
            isin: "US1234567890".into(),
            country_code: "US".into(),
            first_acquisition_date: "20230101".into(),
            acquisition_type: "A".into(),
            value_in_euro: "0.00".into(),
            quantity: "0.00".into(),
            percentage: "100.00".into(),
            contact_name: String::new(),
            asset_type: String::new(),
            asset_subtype: String::new(),
            stock_id_type: String::new(),
            account_id_type: String::new(),
            account_id: String::new(),
            account_code: String::new(),
            entity_address: String::new(),
            entity_country_code: String::new(),
            stock_representation: String::new(),
            valuation_value: String::new(),
        }
    }
}

pub fn validate(r: &CsvRecord) -> Result<()> {
    // NIF: 8 digits + 1 uppercase letter
    if r.nif.len() != 9
        || !r.nif[..8].chars().all(|c| c.is_ascii_digit())
        || !r.nif[8..].chars().all(|c| c.is_ascii_uppercase())
    {
        bail!("Invalid NIF '{}': expected 8 digits + 1 letter", r.nif);
    }

    // country_code: exactly 2 uppercase letters
    if r.country_code.len() != 2 || !r.country_code.chars().all(|c| c.is_ascii_uppercase()) {
        bail!("Invalid country_code '{}': expected 2 uppercase letters", r.country_code);
    }
    // first_acquisition_date: "0" (empty) or 8 digits, valid YYYYMMDD
    if r.first_acquisition_date != "0" {
        if r.first_acquisition_date.len() != 8 || !r.first_acquisition_date.chars().all(|c| c.is_ascii_digit()) {
            bail!("Invalid date '{}': expected YYYYMMDD", r.first_acquisition_date);
        }
        let y: u32 = r.first_acquisition_date[..4].parse().unwrap_or(0);
        let m: u32 = r.first_acquisition_date[4..6].parse().unwrap_or(0);
        let d: u32 = r.first_acquisition_date[6..8].parse().unwrap_or(0);
        if y < 1900 || m < 1 || m > 12 || d < 1 || d > 31 {
            bail!("Invalid date '{}': out of range", r.first_acquisition_date);
        }
    }
    // acquisition_type
    if !matches!(r.acquisition_type.as_str(), "A" | "M" | "C") {
        bail!("Invalid acquisition_type '{}': expected A, M, or C", r.acquisition_type);
    }
    // decimals
    Decimal::from_str(&r.value_in_euro)
        .map_err(|_| anyhow::anyhow!("Invalid value_in_euro '{}'", r.value_in_euro))?;
    Decimal::from_str(&r.quantity)
        .map_err(|_| anyhow::anyhow!("Invalid quantity '{}'", r.quantity))?;
    let pct = Decimal::from_str(&r.percentage)
        .map_err(|_| anyhow::anyhow!("Invalid percentage '{}'", r.percentage))?;
    if pct < Decimal::ZERO || pct > Decimal::from(100) {
        bail!("Invalid percentage '{}': must be 0-100", r.percentage);
    }
    // asset_type (optional)
    if !r.asset_type.is_empty() && !matches!(r.asset_type.as_str(), "C" | "V" | "I" | "S" | "B") {
        bail!("Invalid asset_type '{}': expected C, V, I, S, or B", r.asset_type);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_record() -> CsvRecord {
        CsvRecord {
            nif: "12345678A".into(),
            isin: "US0378331005".into(),
            country_code: "US".into(),
            first_acquisition_date: "20230115".into(),
            acquisition_type: "A".into(),
            value_in_euro: "1000.50".into(),
            quantity: "10.00".into(),
            percentage: "100.00".into(),
            ..Default::default()
        }
    }

    #[test]
    fn test_validate_valid_record() {
        assert!(validate(&valid_record()).is_ok());
    }

    #[test]
    fn test_validate_invalid_nif() {
        let mut r = valid_record();
        r.nif = "BADNIF".into();
        assert!(validate(&r).unwrap_err().to_string().contains("Invalid NIF"));
    }

    #[test]
    fn test_validate_invalid_country_code() {
        let mut r = valid_record();
        r.country_code = "usa".into();
        assert!(validate(&r).unwrap_err().to_string().contains("Invalid country_code"));
    }

    #[test]
    fn test_validate_invalid_date() {
        let mut r = valid_record();
        r.first_acquisition_date = "20231301".into(); // month 13
        assert!(validate(&r).unwrap_err().to_string().contains("Invalid date"));
    }

    #[test]
    fn test_validate_invalid_acquisition_type() {
        let mut r = valid_record();
        r.acquisition_type = "X".into();
        assert!(validate(&r).unwrap_err().to_string().contains("Invalid acquisition_type"));
    }

    #[test]
    fn test_validate_invalid_value() {
        let mut r = valid_record();
        r.value_in_euro = "not_a_number".into();
        assert!(validate(&r).unwrap_err().to_string().contains("Invalid value_in_euro"));
    }

    #[test]
    fn test_validate_percentage_out_of_range() {
        let mut r = valid_record();
        r.percentage = "150.00".into();
        assert!(validate(&r).unwrap_err().to_string().contains("Invalid percentage"));
    }

    #[test]
    fn test_validate_invalid_asset_type() {
        let mut r = valid_record();
        r.asset_type = "X".into();
        assert!(validate(&r).unwrap_err().to_string().contains("Invalid asset_type"));
    }

    #[test]
    fn test_validate_empty_asset_type_ok() {
        let r = valid_record(); // asset_type is empty by default
        assert!(validate(&r).is_ok());
    }
}

