//! Minimal RFC-4180 helpers. Our own writer never embeds raw newlines in a field (all text is
//! sanitised on the way in), but we still quote and escape correctly for hand-edited files.

use std::{
    fs,
    io::{self, Write},
    path::Path,
};

/// Quotes a field when it contains a separator, a quote or a line break.
pub fn escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// Splits one CSV line, honouring quoted fields and doubled quotes.
pub fn split_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut chars = line.trim_end_matches('\r').chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '"' if quoted && chars.peek() == Some(&'"') => {
                current.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => {
                fields.push(current.trim().to_string());
                current.clear();
            }
            other => current.push(other),
        }
    }

    fields.push(current.trim().to_string());
    fields
}

/// Reads a file's data rows, skipping the header line. Blank lines are ignored.
pub fn read_rows(path: &Path) -> io::Result<Vec<Vec<String>>> {
    let text = fs::read_to_string(path)?;
    Ok(text
        .lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .map(split_line)
        .collect())
}

/// Atomically replaces a file: write a sibling `.tmp`, then rename over the target.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension().and_then(|ext| ext.to_str()).unwrap_or("csv")
    ));

    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }

    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            // Windows cannot rename over an existing file in all cases.
            let _ = fs::remove_file(path);
            fs::rename(&tmp, path).map_err(|_| error)
        }
    }
}

/// Writes a header plus rows to a file atomically.
pub fn write_rows(path: &Path, header: &str, rows: &[Vec<String>]) -> io::Result<()> {
    let mut text = header.to_string() + "\n";
    for row in rows {
        text.push_str(
            &row.iter()
                .map(|value| escape(value))
                .collect::<Vec<_>>()
                .join(","),
        );
        text.push('\n');
    }
    atomic_write(path, text.as_bytes())
}

/// Creates `path` with `header` if it does not exist (or is empty).
pub fn ensure_file(path: &Path, header: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if !path.exists() || fs::metadata(path)?.len() == 0 {
        atomic_write(path, format!("{header}\n").as_bytes())?;
    }
    Ok(())
}
