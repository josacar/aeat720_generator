# AEAT 720 Generator

A command-line tool to convert CSV files into Spanish AEAT Modelo 720 fixed-width format files, and vice versa. It can also extract portfolio positions directly from broker PDF statements.

Modelo 720 is the Spanish tax declaration for assets and rights held abroad. The file format follows the specification published in [BOE-A-2013-954](https://www.boe.es/buscar/act.php?id=BOE-A-2013-954). Format reference: [burocratin](https://github.com/vaijira/burocratin/blob/main/src/reports/aeat_720.rs).

## Building

```bash
cargo build --release
```

## Usage

```
aeat720_generator [OPTIONS] [INPUT] [OUTPUT] [COMMAND]
```

### CSV to 720

```bash
aeat720_generator input.csv              # outputs {NIF}.720
aeat720_generator input.csv output.720   # custom output path
```

### 720 to CSV

```bash
aeat720_generator --reverse input.720              # outputs input.csv
aeat720_generator --reverse input.720 output.csv   # custom output path
```

### Extract positions from broker PDFs

Extract portfolio positions from a broker PDF statement and output them as CSV rows compatible with this tool. Requires `pdftotext` (from `poppler-utils`).

Supported sources:
- **Cecabank / Indexa Capital** — fiscal reports with patrimonio section (funds)
- **Revolut Securities** — trading account statements (EUR funds and USD stocks)

```bash
aeat720_generator extract <PDF> --nif NIF --name NAME --phone PHONE \
    --year YEAR --percentage PCT [OPTIONS]
```

Options:

| Flag | Description |
|---|---|
| `--contact-name NAME` | Contact person name (defaults to `--name`) |
| `--eur-usd-rate RATE` | EUR/USD rate at year end, required for USD positions |
| `-o, --output CSV` | CSV file to append to (prints to stdout if omitted) |
| `--dry-run` | Print rows with header without writing to file |

Examples:

```bash
# Indexa/Cecabank fund report
aeat720_generator extract informe_cecabank.pdf \
    --nif 12345678A --name "GARCIA LOPEZ JUAN" --phone 612345678 \
    --year 2025 --percentage 100 -o 12345678A.csv

# Revolut EUR ETFs
aeat720_generator extract revolut_eur.pdf \
    --nif 12345678A --name "GARCIA LOPEZ JUAN" --phone 612345678 \
    --year 2025 --percentage 100 -o 12345678A.csv

# Revolut USD stocks (requires exchange rate)
aeat720_generator extract revolut_usd.pdf \
    --nif 12345678A --name "GARCIA LOPEZ JUAN" --phone 612345678 \
    --year 2025 --percentage 100 --eur-usd-rate 1.1732 -o 12345678A.csv

# Preview without writing
aeat720_generator extract statement.pdf \
    --nif 12345678A --name "GARCIA LOPEZ JUAN" --phone 612345678 \
    --year 2025 --percentage 100 --dry-run
```

The source type is auto-detected from the PDF content. Multiple extractions can be appended to the same CSV file (the header is only written once).

## CSV Format

### Required columns

| Column | Type | Description |
|---|---|---|
| `nif` | string | Spanish tax ID (e.g. `12345678A`) |
| `name` | string | Full name, surname first (e.g. `GARCIA LOPEZ JUAN`) |
| `phone` | string | Contact phone number |
| `year` | integer | Tax year |
| `company_name` | string | Name of the foreign entity |
| `isin` | string | ISIN code (e.g. `US0378331005`) |
| `country_code` | string | 2-letter ISO country code |
| `first_acquisition_date` | string | Date in `YYYYMMDD` format |
| `acquisition_type` | string | `A` = initial, `M` = already existed, `C` = disposed |
| `value_in_euro` | decimal | Acquisition value in euros (e.g. `15234.56`) |
| `quantity` | decimal | Number of shares (e.g. `100.00`) |
| `percentage` | decimal | Ownership percentage (e.g. `100.00`) |

### Optional columns

These columns have sensible defaults and can be omitted from the CSV:

| Column | Type | Default | Description |
|---|---|---|---|
| `contact_name` | string | same as `name` | Contact person name in the summary record |
| `asset_type` | string | `V` | Asset type code. See [asset_type values](#asset_type-values) |
| `asset_subtype` | string | `1` | Asset subtype code. See [asset_subtype values](#asset_subtype-values) |
| `stock_id_type` | string | `1` | Stock identification type. See [stock_id_type values](#stock_id_type-values) |
| `account_id_type` | string | *(empty)* | Account identification type. See [account_id_type values](#account_id_type-values) |
| `account_id` | string | *(empty)* | Account identifier (BIC code) |
| `account_code` | string | *(empty)* | Account code (IBAN or other account number) |
| `entity_address` | string | *(empty)* | Address of the foreign entity |
| `entity_country_code` | string | derived from `isin` | Country code of the entity (first 2 chars of ISIN if omitted) |
| `stock_representation` | string | *(empty)* | Stock representation type. See [stock_representation values](#stock_representation-values) |
| `valuation_value` | decimal | `0.00` | Valuation value at year end |

#### `asset_type` values

| Value | Description |
|---|---|
| `C` | Bank accounts abroad |
| `V` | Securities and rights abroad (stocks, bonds, etc.) |
| `I` | Shares/participations in collective investment institutions abroad |
| `S` | Life/disability insurance and annuities with foreign insurers |
| `B` | Real estate and real estate rights abroad |

#### `asset_subtype` values

Depends on `asset_type`:

For `C` (bank accounts):

| Value | Description |
|---|---|
| `1` | Current account |
| `2` | Savings account |
| `3` | Term deposit |
| `4` | Credit account |
| `5` | Other accounts |

For `V` (securities):

| Value | Description |
|---|---|
| `1` | Equity participations in any type of entity |
| `2` | Debt securities (capital ceded to third parties) |
| `3` | Securities contributed for management/administration to trusts or similar |

For `I` (collective investment): set to `0` (no subtype).

For `S` (insurance/annuities):

| Value | Description |
|---|---|
| `1` | Life or disability insurance with foreign insurer |
| `2` | Temporary or lifetime annuities from foreign entity |

For `B` (real estate):

| Value | Description |
|---|---|
| `1` | Ownership of the property |
| `2` | Rights of use or enjoyment |
| `3` | Bare ownership |
| `4` | Timeshare or similar |
| `5` | Other real estate rights |

#### `stock_id_type` values

Only used when `asset_type` is `V` or `I`:

| Value | Description |
|---|---|
| `1` | ISIN code (12 characters) |
| `2` | Foreign securities without ISIN |

#### `account_id_type` values

Only used when `asset_type` is `C`:

| Value | Description |
|---|---|
| `I` | IBAN |
| `O` | Other identification |

#### `stock_representation` values

Only used when `asset_type` is `V` or `I`:

| Value | Description |
|---|---|
| `A` | Book-entry (anotaciones en cuenta) |
| `B` | Not book-entry (physical certificates) |

#### `acquisition_type` values

| Value | Description |
|---|---|
| `A` | First declaration or newly acquired in the tax year |
| `M` | Previously declared (re-declared due to value increase > €20,000) |
| `C` | Disposed/extinguished during the tax year |

### Example CSV

```csv
nif,name,phone,year,company_name,isin,country_code,first_acquisition_date,acquisition_type,value_in_euro,quantity,percentage
12345678A,GARCIA LOPEZ JUAN,612345678,2024,Apple Inc,US0378331005,US,20230115,A,15234.56,100.00,100.00
12345678A,GARCIA LOPEZ JUAN,612345678,2024,Microsoft Corp,US5949181045,US,20230301,A,8750.25,50.00,100.00
```

All rows must share the same `nif`, `name`, `phone`, and `year`. Each row becomes one detail record in the output file.

## Output File Format

The `.720` file is a fixed-width text file encoded in ISO-8859-15. Each line is exactly 500 bytes.

- **Line 1** — Summary register (type `1`): declarant info and totals across all records.
- **Lines 2+** — Detail registers (type `2`): one per asset held abroad.

Numeric fields are zero-padded. Text fields are space-padded. Negative values are indicated by an `N` sign character in the preceding sign field.

## Output Filename

When no output path is given, the tool derives the filename from the NIF:

- CSV → 720: `{NIF}.720` (e.g. `12345678A.720`)
- 720 → CSV: replaces `.720` with `.csv` (e.g. `12345678A.csv`)
