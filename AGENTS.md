# AGENTS.md

Guidance for AI agents working on this codebase.

## Project overview

A single-binary Rust CLI that converts CSV ↔ AEAT Modelo 720 fixed-width files (500 bytes/line, ISO-8859-15 encoded), and can extract portfolio positions from broker PDF statements. No sub-crates, no async, no web framework.

## Architecture

- `src/main.rs` — CLI entry point (clap derive) and wiring only.
- `src/record.rs` — Fixed-width record primitives: `Reg`, `write_num`, `write_str`, `write_decimal`, `read_field`, `read_decimal`, `split_decimal`.
- `src/model.rs` — `CsvRecord` (serde-deserializable struct with required + optional fields), `Default` impl, and `validate()` for input validation.
- `src/builder.rs` — `build_summary` and `build_detail`: construct 500-byte records from `CsvRecord` data.
- `src/codec.rs` — `generate` / `reverse` (filesystem wrappers) and `generate_to_writer` / `reverse_from_bytes` (testable core functions decoupled from I/O).
- `src/extract.rs` — PDF position extraction: auto-detects broker source (Indexa/Cecabank or Revolut), parses text via `pdftotext`, outputs CSV rows. Uses `Decimal` for monetary values.
- `tests/acceptance.rs` — integration tests that invoke the compiled binary as a subprocess.
- `sample.csv` — reference input for manual testing.

Key types and conventions:
- `Reg = [u8; 500]` — a single fixed-width record (summary or detail).
- `CsvRecord` — serde-deserializable struct with required + optional fields (`#[serde(default)]`). Has `Default` impl for easy test construction.
- Field positions are 1-indexed in comments and function calls (`write_str`, `write_num`, `read_field`) to match the BOE specification directly.
- Text fields are space-padded right. Numeric fields are zero-padded left.
- Negative amounts use a sign field (`N`) in the byte immediately before the numeric field.
- `write_decimal` / `read_decimal` encapsulate the sign+int+frac pattern to avoid duplication.
- Input validation via `validate()` checks NIF, ISIN, country_code, dates, acquisition_type, decimals, and percentage range.

## Build and test

```bash
cargo build --release
cargo test                # runs unit tests (record, model, builder, codec, extract) and acceptance tests
```

All tests must pass before any change is considered complete. The acceptance tests compile the binary and run it as a subprocess, so `cargo test` covers the full CLI flow.

## Development workflow

### 1. Understand the spec first

Field positions and sizes come from [BOE-A-2013-954](https://www.boe.es/buscar/act.php?id=BOE-A-2013-954). When adding or modifying fields, always reference the spec positions (1-indexed). The [burocratin](https://github.com/vaijira/burocratin/blob/main/src/reports/aeat_720.rs) implementation is a useful cross-reference.

### 2. Write tests before or alongside changes

- Unit tests for field encoding/decoding go in `record.rs` tests.
- Unit tests for record building go in `builder.rs` tests.
- Unit tests for generate/reverse logic go in `codec.rs` tests (using in-memory buffers, no filesystem).
- Unit tests for PDF parsing go in `extract.rs` tests (using text fixtures, no real PDFs).
- Validation tests go in `model.rs` tests.
- Acceptance tests in `tests/acceptance.rs` test the full CLI as a subprocess.
- When adding a new CSV field, add unit tests in `builder.rs` for its position and ensure the roundtrip test in `codec.rs` covers it.

### 3. Module responsibilities

| Module | Responsibility |
|---|---|
| `record.rs` | Low-level fixed-width I/O primitives |
| `model.rs` | Data types, defaults, validation |
| `builder.rs` | Construct 500-byte records from CsvRecord |
| `codec.rs` | CSV ↔ 720 conversion (core logic + filesystem wrappers) |
| `extract.rs` | PDF → CSV extraction |
| `main.rs` | CLI definition and wiring only |

### 4. Adding a new PDF source

To add a new broker parser in `src/extract.rs`:
1. Add a variant to the `Source` enum and detection logic in `detect()`.
2. Write a `parse_<broker>(text, ...)` function returning `Result<Vec<Position>>`.
3. Wire it into the `extract()` function's match.
4. Add unit tests with anonymized text fixtures matching the PDF's `pdftotext -layout` output.

### 5. CSV field changes

To add a new optional CSV column:
1. Add the field to `CsvRecord` in `model.rs` with `#[serde(default)]`.
2. Handle it in `build_detail` in `builder.rs` (and `build_summary` if it affects the summary).
3. Handle it in `reverse_from_bytes` in `codec.rs` so the roundtrip works.
4. Add the column to `CSV_COLUMNS` in `codec.rs`.
5. Add validation in `validate()` if needed.
6. Add unit tests in `builder.rs` for the field position and in `codec.rs` for roundtrip.
7. Update `README.md` (optional columns table).

### 6. Encoding

The output file is ISO-8859-15. The `write_str` function in `record.rs` handles encoding via `encoding_rs`. When reading back (reverse), bytes are decoded from ISO-8859-15. Don't use raw UTF-8 string operations on the 500-byte records.

### 7. Decimal handling

Monetary values and quantities use `rust_decimal` throughout (both in `model.rs` and `extract.rs`). The `split_decimal` function separates integer and fractional parts for zero-padded output. The `write_decimal` / `read_decimal` helpers encapsulate the full sign+int+frac pattern. Be careful with the fractional part — `set_scale(0)` is used to get the raw digits (e.g., `.50` → `50`, not `5`).

## Common pitfalls

- Field positions are 1-indexed in the code to match the BOE spec, but Rust slices are 0-indexed. The helpers in `record.rs` handle this offset internally.
- `company_name` is uppercased on write (`to_uppercase()`), so the reversed CSV will have uppercase names even if the original didn't.
- The `phone` field is written as a zero-padded number, so leading zeros are lost on reverse (stripped with `trim_start_matches('0')`).
- All CSV rows must share the same `nif`, `name`, `phone`, and `year`. The tool reads these from the first row only.
- Input validation runs before generation — invalid data produces descriptive error messages.
