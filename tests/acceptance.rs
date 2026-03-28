use std::fs;
use std::process::Command;

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_aeat720_generator"))
}

fn tmp_dir() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("aeat720_test_{}_{}", std::process::id(), id));
    let _ = fs::create_dir_all(&dir);
    dir
}

const SAMPLE_CSV: &str = "\
nif,name,phone,year,company_name,isin,country_code,first_acquisition_date,acquisition_type,value_in_euro,quantity,percentage
12345678A,GARCIA LOPEZ JUAN,612345678,2024,Apple Inc,US0378331005,US,20230115,A,15234.56,100.00,100.00
12345678A,GARCIA LOPEZ JUAN,612345678,2024,Microsoft Corp,US5949181045,US,20230301,A,8750.25,50.50,100.00
";

// --- Acceptance: no args prints usage ---

#[test]
fn test_no_args_shows_usage() {
    let out = binary().output().expect("failed to run");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Usage"), "expected Usage in stdout: {}", stdout);
}

// --- Acceptance: generate .720 from CSV ---

#[test]
fn test_generate_720_from_csv() {
    let dir = tmp_dir();
    let csv_path = dir.join("input.csv");
    let out_path = dir.join("output.720");
    fs::write(&csv_path, SAMPLE_CSV).unwrap();

    let result = binary()
        .current_dir(&dir)
        .arg(csv_path.to_str().unwrap())
        .arg(out_path.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(result.status.success(), "stderr: {}", String::from_utf8_lossy(&result.stderr));
    assert!(out_path.exists());

    let data = fs::read(&out_path).unwrap();
    let lines: Vec<&[u8]> = data.split(|&b| b == b'\n').filter(|l| l.len() == 500).collect();

    // 1 summary + 2 detail records
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0][0], b'1');
    assert_eq!(lines[1][0], b'2');
    assert_eq!(lines[2][0], b'2');
    for line in &lines {
        assert_eq!(line.len(), 500);
    }

    fs::remove_dir_all(&dir).ok();
}

// --- Acceptance: reverse .720 back to CSV ---

#[test]
fn test_reverse_720_to_csv() {
    let dir = tmp_dir();
    let csv_path = dir.join("input.csv");
    let f720_path = dir.join("test.720");
    let out_csv = dir.join("reversed.csv");
    fs::write(&csv_path, SAMPLE_CSV).unwrap();

    binary()
        .current_dir(&dir)
        .arg(csv_path.to_str().unwrap())
        .arg(f720_path.to_str().unwrap())
        .output()
        .expect("failed to run");

    let result = binary()
        .current_dir(&dir)
        .arg("--reverse")
        .arg(f720_path.to_str().unwrap())
        .arg(out_csv.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(result.status.success(), "stderr: {}", String::from_utf8_lossy(&result.stderr));
    assert!(out_csv.exists());

    let content = fs::read_to_string(&out_csv).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 3); // header + 2 rows
    assert!(lines[0].starts_with("nif,"));
    assert!(lines[1].contains("12345678A"));
    assert!(lines[1].contains("APPLE INC"));
    assert!(lines[2].contains("MICROSOFT CORP"));

    fs::remove_dir_all(&dir).ok();
}

// --- Acceptance: roundtrip preserves data ---

#[test]
fn test_roundtrip_preserves_values() {
    let dir = tmp_dir();
    let csv_path = dir.join("input.csv");
    let f720_path = dir.join("rt.720");
    let out_csv = dir.join("rt.csv");
    fs::write(&csv_path, SAMPLE_CSV).unwrap();

    binary()
        .current_dir(&dir)
        .arg(csv_path.to_str().unwrap())
        .arg(f720_path.to_str().unwrap())
        .output()
        .expect("failed to run");

    binary()
        .current_dir(&dir)
        .arg("--reverse")
        .arg(f720_path.to_str().unwrap())
        .arg(out_csv.to_str().unwrap())
        .output()
        .expect("failed to run");

    let content = fs::read_to_string(&out_csv).unwrap();
    assert!(content.contains("15234.56"));
    assert!(content.contains("8750.25"));
    assert!(content.contains("US0378331005"));
    assert!(content.contains("US5949181045"));
    assert!(content.contains("20230115"));
    assert!(content.contains("20230301"));
    assert!(content.contains("100.00"));
    assert!(content.contains("50.50"));

    fs::remove_dir_all(&dir).ok();
}

// --- Acceptance: default output filename is NIF.720 ---

#[test]
fn test_default_output_filename() {
    let dir = tmp_dir();
    let csv_path = dir.join("input.csv");
    fs::write(&csv_path, SAMPLE_CSV).unwrap();

    let result = binary()
        .current_dir(&dir)
        .arg(csv_path.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(result.status.success(), "stderr: {}", String::from_utf8_lossy(&result.stderr));

    let expected = dir.join("12345678A.720");
    assert!(expected.exists(), "Expected default output 12345678A.720");

    fs::remove_dir_all(&dir).ok();
}

// --- Acceptance: --reverse with missing file fails ---

#[test]
fn test_reverse_missing_file_fails() {
    let result = binary()
        .arg("--reverse")
        .arg("/tmp/nonexistent_aeat720.720")
        .output()
        .expect("failed to run");
    assert!(!result.status.success());
}

// --- Acceptance: --reverse without input file fails ---

#[test]
fn test_reverse_no_input_fails() {
    let result = binary()
        .arg("--reverse")
        .output()
        .expect("failed to run");
    assert!(!result.status.success());
}

// --- Acceptance: .720 field positions are correct ---

#[test]
fn test_720_field_positions() {
    let dir = tmp_dir();
    let csv_path = dir.join("input.csv");
    let f720_path = dir.join("pos.720");
    fs::write(&csv_path, SAMPLE_CSV).unwrap();

    let result = binary()
        .current_dir(&dir)
        .arg(csv_path.to_str().unwrap())
        .arg(f720_path.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(result.status.success(), "generate failed: {}", String::from_utf8_lossy(&result.stderr));
    assert!(f720_path.exists(), "720 file not found at {:?}", f720_path);

    let data = fs::read(&f720_path).unwrap();
    let lines: Vec<&[u8]> = data.split(|&b| b == b'\n').filter(|l| l.len() == 500).collect();

    // Summary line
    let s = lines[0];
    let field = |b: usize, e: usize| String::from_utf8_lossy(&s[b - 1..e]).to_string();
    assert_eq!(field(1, 1), "1");
    assert_eq!(field(2, 4), "720");
    assert_eq!(field(5, 8), "2024");
    assert_eq!(field(58, 58), "T");

    // Detail line
    let d = lines[1];
    let dfield = |b: usize, e: usize| String::from_utf8_lossy(&d[b - 1..e]).trim().to_string();
    assert_eq!(dfield(1, 1), "2");
    assert_eq!(dfield(2, 4), "720");
    assert_eq!(dfield(76, 76), "1");    // owner
    assert_eq!(dfield(102, 102), "V");  // stocks
    assert_eq!(dfield(103, 103), "1");  // subtype
    assert_eq!(dfield(131, 131), "1");  // ISIN type
    assert_eq!(dfield(423, 423), "A");  // acquisition type
    assert_eq!(dfield(462, 462), "");  // stock representation (empty when not set)

    fs::remove_dir_all(&dir).ok();
}
