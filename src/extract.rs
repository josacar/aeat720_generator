use anyhow::{bail, Result};
use regex::Regex;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::process::Command;
use std::str::FromStr;

/// A position extracted from a broker PDF.
#[derive(Debug, Clone)]
pub struct Position {
    pub company_name: String,
    pub isin: String,
    pub country_code: String,
    pub first_acquisition_date: String,
    pub acquisition_type: String,
    pub value_in_euro: Decimal,
    pub quantity: Decimal,
    pub asset_type: String,
    pub asset_subtype: String,
    pub stock_id_type: String,
    pub entity_address: String,
    pub entity_country_code: String,
    pub stock_representation: String,
}

pub struct ExtractArgs {
    pub nif: String,
    pub name: String,
    pub phone: String,
    pub year: usize,
    pub percentage: f64,
    pub contact_name: String,
    pub eur_usd_rate: Option<f64>,
}

// ---------------------------------------------------------------------------
// PDF text extraction
// ---------------------------------------------------------------------------

pub fn pdf_to_text(path: &str) -> Result<String> {
    let out = Command::new("pdftotext")
        .args(["-layout", path, "-"])
        .output()?;
    if !out.status.success() {
        bail!("pdftotext failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

// ---------------------------------------------------------------------------
// Source detection
// ---------------------------------------------------------------------------

pub(crate) enum Source {
    Indexa,
    Revolut,
}

pub(crate) fn detect(text: &str) -> Option<Source> {
    if text.contains("Indexa Capital") || text.contains("Cecabank") {
        Some(Source::Indexa)
    } else if text.contains("Revolut Securities") {
        Some(Source::Revolut)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub(crate) fn country_from_isin(isin: &str) -> &str {
    if isin.len() >= 2 && isin[..2].chars().all(|c| c.is_ascii_alphabetic()) {
        &isin[..2]
    } else {
        "US"
    }
}

/// Parse European decimal format: "3.253,89" -> Decimal
pub(crate) fn parse_eur(s: &str) -> Result<Decimal> {
    let normalized = s.replace('.', "").replace(',', ".");
    Decimal::from_str(&normalized).map_err(|e| anyhow::anyhow!("Failed to parse '{}': {}", s, e))
}

// ---------------------------------------------------------------------------
// Indexa / Cecabank parser
// ---------------------------------------------------------------------------

pub(crate) fn parse_indexa(text: &str) -> Result<Vec<Position>> {
    let mut positions = Vec::new();

    let parts: Vec<&str> = text.split("Información impuesto de patrimonio").collect();
    let section = match parts.last() {
        Some(s) if parts.len() >= 3 => s,
        _ => return Ok(positions),
    };

    let first_dates = indexa_first_dates(text);

    let re = Regex::new(
        r"([A-Z]{2}\w{10})\s+([\d,.]+)\s+([\d,.]+)€\s+\d{2}/\d{2}/\d{4}\s+([\d,.]+)€"
    ).unwrap();

    for caps in re.captures_iter(section) {
        let isin = caps[1].to_string();
        let quantity = parse_eur(&caps[2])?;
        let valuation = parse_eur(&caps[4])?;

        let match_start = caps.get(0).unwrap().start();
        let line_start = section[..match_start].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let name = section[line_start..match_start].trim();
        let name = if name.is_empty() { "UNKNOWN FUND" } else { name };

        let cc = country_from_isin(&isin).to_string();
        let first_date = first_dates.get(isin.as_str()).cloned().unwrap_or_else(|| "0".into());

        positions.push(Position {
            company_name: name.to_string(),
            isin,
            country_code: cc,
            first_acquisition_date: first_date,
            acquisition_type: "A".into(),
            value_in_euro: valuation,
            quantity,
            asset_type: "I".into(),
            asset_subtype: "0".into(),
            stock_id_type: "1".into(),
            entity_address: String::new(),
            entity_country_code: "ES".into(),
            stock_representation: "B".into(),
        });
    }
    Ok(positions)
}

pub(crate) fn indexa_first_dates(text: &str) -> HashMap<String, String> {
    let mut dates: HashMap<String, String> = HashMap::new();
    let parts: Vec<&str> = text.split("Ganancias y pérdidas patrimoniales").collect();
    let tx = match parts.last() {
        Some(s) if parts.len() >= 2 => s,
        _ => return dates,
    };

    let re = Regex::new(r"([A-Z]{2}\w{10})\s+Compras en\s+(\d{2})/(\d{2})/(\d{4})").unwrap();
    for caps in re.captures_iter(tx) {
        let isin = caps[1].to_string();
        let yyyymmdd = format!("{}{}{}", &caps[4], &caps[3], &caps[2]);
        let entry = dates.entry(isin).or_insert_with(|| yyyymmdd.clone());
        if yyyymmdd < *entry {
            *entry = yyyymmdd;
        }
    }
    dates
}

// ---------------------------------------------------------------------------
// Revolut parser
// ---------------------------------------------------------------------------

const REVOLUT_ADDRESS: &str =
    "KONSTITUCIJOS AVE. 21B                                                                      VILNA                                                       08130";

pub(crate) fn parse_revolut(text: &str, eur_usd_rate: Option<f64>) -> Result<Vec<Position>> {
    let first_dates = revolut_first_dates(text);
    let mut positions = Vec::new();
    for currency in &["EUR", "USD"] {
        positions.extend(parse_revolut_portfolio(text, currency, &first_dates, eur_usd_rate)?);
    }
    Ok(positions)
}

pub(crate) fn revolut_first_dates(text: &str) -> HashMap<String, String> {
    let re = Regex::new(
        r"(\d{2}\s+\w+\s+\d{4})\s+[\d:]+\s+GMT\s+(\w+)\s+Trade\s+-\s+(?:Market|Limit)\s+([\d.]+)\s+.*?(Buy|Sell)"
    ).unwrap();

    let mut buys: HashMap<String, Vec<(String, f64)>> = HashMap::new();
    let mut sells: HashMap<String, Vec<String>> = HashMap::new();

    for caps in re.captures_iter(text) {
        let date = parse_revolut_date(&caps[1]);
        let symbol = caps[2].to_string();
        let qty: f64 = caps[3].parse().unwrap_or(0.0);
        if &caps[4] == "Buy" {
            buys.entry(symbol).or_default().push((date, qty));
        } else {
            sells.entry(symbol).or_default().push(date);
        }
    }

    let mut dates = HashMap::new();
    for (symbol, mut buy_list) in buys {
        buy_list.sort_by(|a, b| a.0.cmp(&b.0));
        let mut sell_list = sells.remove(&symbol).unwrap_or_default();
        sell_list.sort();
        let date = if let Some(last_sell) = sell_list.last() {
            buy_list.iter()
                .find(|(d, _)| d > last_sell)
                .map(|(d, _)| d.clone())
                .unwrap_or_else(|| buy_list[0].0.clone())
        } else {
            buy_list[0].0.clone()
        };
        dates.insert(symbol, date);
    }
    dates
}

pub(crate) fn parse_revolut_date(s: &str) -> String {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() < 3 { return "0".into(); }
    let month = match parts[1] {
        "Jan" => "01", "Feb" => "02", "Mar" => "03", "Apr" => "04",
        "May" => "05", "Jun" => "06", "Jul" => "07", "Aug" => "08",
        "Sep" => "09", "Oct" => "10", "Nov" => "11", "Dec" => "12",
        _ => "01",
    };
    format!("{}{}{:02}", parts[2], month, parts[0].parse::<u32>().unwrap_or(1))
}

fn parse_revolut_portfolio(
    text: &str, currency: &str,
    first_dates: &HashMap<String, String>,
    eur_usd_rate: Option<f64>,
) -> Result<Vec<Position>> {
    let mut positions = Vec::new();

    let header = format!("{} Portfolio breakdown\n", currency);
    let trailer = format!("{} Transactions", currency);
    let start = match text.find(&header) {
        Some(i) => i + header.len(),
        None => return Ok(positions),
    };
    let end = text[start..].find(&trailer).map(|i| start + i).unwrap_or(text.len());
    let section = &text[start..end];

    let price_re = r"€|US\$";
    let pattern = format!(
        r"(?m)^(\w+)\s+(.+?)\s{{2,}}([A-Z]{{2}}\w{{8,10}})\s+([\d.]+)\s+(?:{p})([\d,.]+)\s+(?:{p})([\d,.]+)",
        p = price_re
    );
    let re = Regex::new(&pattern).unwrap();

    for caps in re.captures_iter(section) {
        let symbol = caps[1].to_string();
        let isin = caps[3].to_string();
        let quantity = Decimal::from_str(&caps[4].replace(',', ""))
            .map_err(|e| anyhow::anyhow!("Bad quantity '{}': {}", &caps[4], e))?;
        let value = Decimal::from_str(&caps[6].replace(',', ""))
            .map_err(|e| anyhow::anyhow!("Bad value '{}': {}", &caps[6], e))?;

        let value_eur = if currency == "USD" {
            match eur_usd_rate {
                Some(rate) => {
                    let rate_dec = Decimal::from_str(&format!("{}", rate))?;
                    (value / rate_dec).round_dp(2)
                }
                None => {
                    eprintln!("WARNING: USD position {} needs --eur-usd-rate, skipping", symbol);
                    continue;
                }
            }
        } else {
            value
        };

        let is_fund = currency == "EUR";
        let cc = country_from_isin(&isin).to_string();
        let first_date = first_dates.get(&symbol).cloned().unwrap_or_else(|| "0".into());

        positions.push(Position {
            company_name: "REVOLUT SECURITIES EUROPE UAB".into(),
            isin,
            country_code: cc,
            first_acquisition_date: first_date,
            acquisition_type: "A".into(),
            value_in_euro: value_eur,
            quantity,
            asset_type: if is_fund { "I" } else { "V" }.into(),
            asset_subtype: if is_fund { "0" } else { "1" }.into(),
            stock_id_type: "2".into(),
            entity_address: REVOLUT_ADDRESS.into(),
            entity_country_code: "LT".into(),
            stock_representation: "B".into(),
        });
    }
    Ok(positions)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn extract(pdf_path: &str, args: &ExtractArgs, output: Option<&str>, dry_run: bool) -> Result<()> {
    let text = pdf_to_text(pdf_path)?;

    let (source, positions) = match detect(&text) {
        Some(Source::Indexa) => ("indexa", parse_indexa(&text)?),
        Some(Source::Revolut) => ("revolut", parse_revolut(&text, args.eur_usd_rate)?),
        None => bail!("Could not detect PDF source type"),
    };

    if positions.is_empty() {
        bail!("No positions extracted from {}", source);
    }

    let rows: Vec<Vec<String>> = positions.iter().map(|p| position_to_row(p, args)).collect();

    if dry_run || output.is_none() {
        let mut wtr = csv::Writer::from_writer(std::io::stdout());
        if dry_run {
            wtr.write_record(CSV_HEADER)?;
        }
        for row in &rows {
            wtr.write_record(row)?;
        }
        wtr.flush()?;
        eprintln!("{} positions from {}", rows.len(), source);
    } else {
        let path = output.unwrap();
        let exists = std::path::Path::new(path).exists()
            && std::fs::metadata(path).map(|m| m.len() > 0).unwrap_or(false);
        let file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
        let mut wtr = csv::Writer::from_writer(file);
        if !exists {
            wtr.write_record(CSV_HEADER)?;
        }
        for row in &rows {
            wtr.write_record(row)?;
        }
        wtr.flush()?;
        eprintln!("{} positions appended to {}", rows.len(), path);
    }
    Ok(())
}

const CSV_HEADER: [&str; 23] = [
    "nif", "name", "phone", "year", "company_name", "isin", "country_code",
    "first_acquisition_date", "acquisition_type", "value_in_euro", "quantity",
    "percentage", "contact_name", "asset_type", "asset_subtype", "stock_id_type",
    "account_id_type", "account_id", "account_code", "entity_address",
    "entity_country_code", "stock_representation", "valuation_value",
];

fn position_to_row(p: &Position, args: &ExtractArgs) -> Vec<String> {
    vec![
        args.nif.clone(), args.name.clone(), args.phone.clone(),
        args.year.to_string(), p.company_name.clone(), p.isin.clone(),
        p.country_code.clone(), p.first_acquisition_date.clone(),
        p.acquisition_type.clone(), format!("{}", p.value_in_euro),
        format!("{}", p.quantity), format!("{}", args.percentage),
        args.contact_name.clone(), p.asset_type.clone(), p.asset_subtype.clone(),
        p.stock_id_type.clone(), String::new(), String::new(), String::new(),
        p.entity_address.clone(), p.entity_country_code.clone(),
        p.stock_representation.clone(), "0".into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- detect ---

    #[test]
    fn test_detect_indexa() {
        assert!(matches!(detect("blah Indexa Capital blah"), Some(Source::Indexa)));
        assert!(matches!(detect("blah Cecabank blah"), Some(Source::Indexa)));
    }

    #[test]
    fn test_detect_revolut() {
        assert!(matches!(detect("blah Revolut Securities blah"), Some(Source::Revolut)));
    }

    #[test]
    fn test_detect_unknown() {
        assert!(detect("random text").is_none());
    }

    // --- helpers ---

    #[test]
    fn test_parse_eur() {
        assert_eq!(parse_eur("3.253,89").unwrap(), Decimal::from_str("3253.89").unwrap());
        assert_eq!(parse_eur("100,00").unwrap(), Decimal::from_str("100.00").unwrap());
        assert_eq!(parse_eur("0,50").unwrap(), Decimal::from_str("0.50").unwrap());
    }

    #[test]
    fn test_parse_eur_invalid() {
        assert!(parse_eur("not_a_number").is_err());
    }

    #[test]
    fn test_country_from_isin_valid() {
        assert_eq!(country_from_isin("US0378331005"), "US");
        assert_eq!(country_from_isin("IE00AA00AA00"), "IE");
    }

    #[test]
    fn test_country_from_isin_invalid() {
        assert_eq!(country_from_isin("12"), "US"); // fallback
    }

    #[test]
    fn test_parse_revolut_date() {
        assert_eq!(parse_revolut_date("24 Nov 2025"), "20251124");
        assert_eq!(parse_revolut_date("01 Jan 2024"), "20240101");
        assert_eq!(parse_revolut_date("15 Dec 2025"), "20251215");
    }

    // --- Indexa fixtures ---

    fn indexa_fixture() -> String {
        // Mimics real Cecabank/Indexa fiscal report structure (anonymized)
        // parse_indexa splits on "Información impuesto de patrimonio" and needs >= 3 parts
        // indexa_first_dates splits on "Ganancias y pérdidas patrimoniales" and needs >= 2 parts
        // ISIN regex: [A-Z]{2}\w{10} = exactly 12 chars
        r#"
Datos fiscales Cecabank 2025

Indexa Capital

Tabla de contenidos
     1. Productos e intervinientes
     4. Información impuesto de patrimonio

3. Ganancias y pérdidas patrimoniales

FUND ALPHA     IE00AAAAA001      Compras en   15/03/2024
FUND ALPHA     IE00AAAAA001      Compras en   20/06/2023
FUND BETA      IE00BBBBB002      Compras en   10/01/2025

4. Información impuesto de patrimonio

Cuentas de valores:

FUND ALPHA          IE00AAAAA001                3,53            213,49€        31/12/2025          753,62€
FUND BETA           IE00BBBBB002                6,36            511,62€        31/12/2025        3.253,89€
"#.to_string()
    }

    #[test]
    fn test_parse_indexa() {
        let text = indexa_fixture();
        let positions = parse_indexa(&text).unwrap();
        assert_eq!(positions.len(), 2);

        assert_eq!(positions[0].company_name, "FUND ALPHA");
        assert_eq!(positions[0].isin, "IE00AAAAA001");
        assert_eq!(positions[0].quantity, Decimal::from_str("3.53").unwrap());
        assert_eq!(positions[0].value_in_euro, Decimal::from_str("753.62").unwrap());
        assert_eq!(positions[0].first_acquisition_date, "20230620"); // earliest
        assert_eq!(positions[0].asset_type, "I");
        assert_eq!(positions[0].entity_country_code, "ES");

        assert_eq!(positions[1].company_name, "FUND BETA");
        assert_eq!(positions[1].value_in_euro, Decimal::from_str("3253.89").unwrap());
    }

    #[test]
    fn test_parse_indexa_no_patrimonio_section() {
        let text = "Some random text without the required sections";
        let positions = parse_indexa(text).unwrap();
        assert!(positions.is_empty());
    }

    #[test]
    fn test_indexa_first_dates_picks_earliest() {
        let text = r#"
3. Ganancias y pérdidas patrimoniales

FUND X     IE00XXXXXX01      Compras en   15/06/2024
FUND X     IE00XXXXXX01      Compras en   10/03/2023
"#;
        let dates = indexa_first_dates(text);
        assert_eq!(dates.get("IE00XXXXXX01").unwrap(), "20230310");
    }

    // --- Revolut fixtures ---

    fn revolut_eur_fixture() -> String {
        r#"
Revolut Securities Europe UAB

EUR Portfolio breakdown
Symbol       Company                                                                                     ISIN                Quantity         Price        Value          % of Portfolio

ABCD         Test Bond Fund ETF                                                                          IE00AA00AA01        1.50000000       €100.00      €150.00               50.00%

EFGH         Test Equity Fund ETF                                                                        IE00BB00BB02        5.00000000       €20.00       €100.00               33.33%

Positions Value                                                                                                                                            €250.00              97.43%

EUR Transactions
Date                                                    Symbol         Type                                 Quantity          Price        Side   Value       Fees         Commission

24 Nov 2025 13:00:08 GMT                                ABCD           Trade - Market                       0.50000000        €100.00      Buy    €50         €0                    €0

24 Nov 2025 13:01:02 GMT                                EFGH           Trade - Market                       2.00000000        €20.00       Buy    €40         €0                    €0

USD Portfolio breakdown
Symbol                        Company                               ISIN              Quantity                            Price                Value                                                         % of Portfolio

Positions Value                                                                                                                                US$0                                                                          0%

USD Transactions
Date              Symbol                      Type               Quantity                          Price               Side            Value                 Fees                                              Commission
"#.to_string()
    }

    fn revolut_usd_fixture() -> String {
        r#"
Revolut Securities Europe UAB

EUR Portfolio breakdown
Symbol       Company                                                                                     ISIN                Quantity         Price        Value          % of Portfolio

Positions Value                                                                                                                                            €0

EUR Transactions

USD Portfolio breakdown
Symbol                        Company                               ISIN              Quantity                            Price                Value                                                         % of Portfolio

AAPL         Apple Inc                                                                                    US0378331005        10              US$150.00    US$1500.00                                                   100%

Positions Value                                                                                                                                US$1500.00

USD Transactions
Date              Symbol                      Type               Quantity                          Price               Side            Value                 Fees                                              Commission

15 Mar 2025 10:00:00 GMT                      AAPL               Trade - Market                    10                  US$150.00       Buy     US$1500.00    US$0                                              US$0
"#.to_string()
    }

    #[test]
    fn test_parse_revolut_eur() {
        let text = revolut_eur_fixture();
        let positions = parse_revolut(&text, None).unwrap();
        assert_eq!(positions.len(), 2);

        assert_eq!(positions[0].company_name, "REVOLUT SECURITIES EUROPE UAB");
        assert_eq!(positions[0].isin, "IE00AA00AA01");
        assert_eq!(positions[0].quantity, Decimal::from_str("1.50000000").unwrap());
        assert_eq!(positions[0].value_in_euro, Decimal::from_str("150.00").unwrap());
        assert_eq!(positions[0].asset_type, "I"); // EUR = fund
        assert_eq!(positions[0].entity_country_code, "LT");
        assert_eq!(positions[0].first_acquisition_date, "20251124");

        assert_eq!(positions[1].isin, "IE00BB00BB02");
        assert_eq!(positions[1].value_in_euro, Decimal::from_str("100.00").unwrap());
    }

    #[test]
    fn test_parse_revolut_usd_with_rate() {
        let text = revolut_usd_fixture();
        let positions = parse_revolut(&text, Some(1.5)).unwrap();
        // Only USD position (EUR section is empty)
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].isin, "US0378331005");
        assert_eq!(positions[0].asset_type, "V"); // USD = stock
        // 1500 / 1.5 = 1000.00
        assert_eq!(positions[0].value_in_euro, Decimal::from_str("1000.00").unwrap());
    }

    #[test]
    fn test_parse_revolut_usd_without_rate_skips() {
        let text = revolut_usd_fixture();
        let positions = parse_revolut(&text, None).unwrap();
        // USD positions skipped without rate
        assert!(positions.is_empty());
    }

    #[test]
    fn test_revolut_first_dates_buy_after_sell() {
        let text = r#"
10 Jan 2025 10:00:00 GMT                      AAPL               Trade - Market                    5                   US$150.00       Buy     US$750        US$0                                              US$0

15 Feb 2025 10:00:00 GMT                      AAPL               Trade - Market                    5                   US$160.00       Sell    US$800        US$0                                              US$0

20 Mar 2025 10:00:00 GMT                      AAPL               Trade - Market                    3                   US$170.00       Buy     US$510        US$0                                              US$0
"#;
        let dates = revolut_first_dates(text);
        // Should pick the buy after the last sell
        assert_eq!(dates.get("AAPL").unwrap(), "20250320");
    }

    #[test]
    fn test_revolut_first_dates_no_sells() {
        let text = r#"
10 Jan 2025 10:00:00 GMT                      AAPL               Trade - Market                    5                   US$150.00       Buy     US$750        US$0                                              US$0

20 Mar 2025 10:00:00 GMT                      AAPL               Trade - Market                    3                   US$170.00       Buy     US$510        US$0                                              US$0
"#;
        let dates = revolut_first_dates(text);
        assert_eq!(dates.get("AAPL").unwrap(), "20250110"); // earliest buy
    }
}
