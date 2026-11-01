use pyo3::exceptions::PyIOError;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[pyclass(get_all)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnStats {
    pub name: String,
    pub inferred_type: String,
    pub total_count: usize,
    pub null_count: usize,
    pub unique_count: usize,
    pub null_percentage: f64,
    pub sample_values: Vec<String>,
}

#[pyclass(get_all)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAuditReport {
    pub file_path: String,
    pub file_format: String,
    pub row_count: usize,
    pub column_count: usize,
    pub duplicate_row_count: usize,
    pub columns: Vec<ColumnStats>,
}

#[pymethods]
impl FileAuditReport {
    pub fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| PyIOError::new_err(format!("Serialization error: {e}")))
    }
}

#[derive(Default)]
struct ColumnAccumulator {
    name: String,
    total_count: usize,
    null_count: usize,
    seen_values: HashSet<String>,
    samples: Vec<String>,
    all_int: bool,
    all_float: bool,
    all_bool: bool,
    has_non_null: bool,
}

impl ColumnAccumulator {
    fn new(name: String) -> Self {
        Self {
            name,
            total_count: 0,
            null_count: 0,
            seen_values: HashSet::new(),
            samples: Vec::with_capacity(5),
            all_int: true,
            all_float: true,
            all_bool: true,
            has_non_null: false,
        }
    }

    fn observe(&mut self, raw: &str) {
        self.total_count += 1;
        let trimmed = raw.trim();

        if trimmed.is_empty()
            || trimmed.eq_ignore_ascii_case("null")
            || trimmed.eq_ignore_ascii_case("na")
            || trimmed.eq_ignore_ascii_case("nan")
        {
            self.null_count += 1;
            return;
        }

        self.has_non_null = true;

        if self.all_int && trimmed.parse::<i64>().is_err() {
            self.all_int = false;
        }
        if self.all_float && trimmed.parse::<f64>().is_err() {
            self.all_float = false;
        }
        if self.all_bool
            && !matches!(
                trimmed.to_ascii_lowercase().as_str(),
                "true" | "false" | "0" | "1" | "yes" | "no"
            )
        {
            self.all_bool = false;
        }

        if self.seen_values.insert(trimmed.to_string()) && self.samples.len() < 5 {
            self.samples.push(trimmed.to_string());
        }
    }

    fn finalize(self) -> ColumnStats {
        let inferred_type = if !self.has_non_null {
            "Empty"
        } else if self.all_bool {
            "Boolean"
        } else if self.all_int {
            "Integer"
        } else if self.all_float {
            "Float"
        } else {
            "Text"
        }
        .to_string();

        let null_percentage = if self.total_count > 0 {
            ((self.null_count as f64 / self.total_count as f64) * 10000.0).round() / 100.0
        } else {
            0.0
        };

        ColumnStats {
            name: self.name,
            inferred_type,
            total_count: self.total_count,
            null_count: self.null_count,
            unique_count: self.seen_values.len(),
            null_percentage,
            sample_values: self.samples,
        }
    }
}

/// Audits a CSV file and computes schema types, null anomalies, and exact duplicate row counts.
#[pyfunction]
#[pyo3(signature = (path, delimiter=None))]
pub fn audit_csv_file(path: &str, delimiter: Option<u8>) -> PyResult<FileAuditReport> {
    let file_path = Path::new(path);
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter.unwrap_or(b','))
        .flexible(true)
        .from_path(file_path)
        .map_err(|e| PyIOError::new_err(format!("Failed to open CSV '{path}': {e}")))?;

    let headers = reader
        .headers()
        .map_err(|e| PyIOError::new_err(format!("Failed to read CSV headers: {e}")))?
        .clone();

    let mut accumulators: Vec<ColumnAccumulator> = headers
        .iter()
        .map(|h| ColumnAccumulator::new(h.to_string()))
        .collect();

    let mut row_count = 0usize;
    let mut duplicate_row_count = 0usize;
    let mut seen_rows: HashSet<Vec<String>> = HashSet::new();

    for result in reader.records() {
        let record = result.map_err(|e| PyIOError::new_err(format!("Malformed CSV row: {e}")))?;
        row_count += 1;

        let row_vec: Vec<String> = record.iter().map(ToString::to_string).collect();
        if !seen_rows.insert(row_vec) {
            duplicate_row_count += 1;
        }

        for (idx, acc) in accumulators.iter_mut().enumerate() {
            let val = record.get(idx).unwrap_or("");
            acc.observe(val);
        }
    }

    let columns: Vec<ColumnStats> = accumulators
        .into_iter()
        .map(ColumnAccumulator::finalize)
        .collect();

    Ok(FileAuditReport {
        file_path: path.to_string(),
        file_format: "csv".to_string(),
        row_count,
        column_count: columns.len(),
        duplicate_row_count,
        columns,
    })
}

/// Audits a newline-delimited JSON (JSONL) file.
#[pyfunction]
pub fn audit_jsonl_file(path: &str) -> PyResult<FileAuditReport> {
    let file = File::open(path)
        .map_err(|e| PyIOError::new_err(format!("Failed to open JSONL '{path}': {e}")))?;
    let reader = BufReader::new(file);

    let mut accumulators: Vec<ColumnAccumulator> = Vec::new();
    let mut column_index: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut row_count = 0usize;
    let mut duplicate_row_count = 0usize;
    let mut seen_lines: HashSet<String> = HashSet::new();

    for (line_num, line_res) in reader.lines().enumerate() {
        let line = line_res
            .map_err(|e| PyIOError::new_err(format!("Read error at line {}: {e}", line_num + 1)))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        row_count += 1;
        if !seen_lines.insert(trimmed.to_string()) {
            duplicate_row_count += 1;
        }

        let value: serde_json::Value = serde_json::from_str(trimmed).map_err(|e| {
            PyIOError::new_err(format!("Invalid JSON at line {}: {e}", line_num + 1))
        })?;

        if let Some(obj) = value.as_object() {
            for (k, v) in obj {
                let idx = if let Some(&existing_idx) = column_index.get(k) {
                    existing_idx
                } else {
                    let new_idx = accumulators.len();
                    column_index.insert(k.clone(), new_idx);
                    let mut new_acc = ColumnAccumulator::new(k.clone());
                    for _ in 0..(row_count - 1) {
                        new_acc.observe("");
                    }
                    accumulators.push(new_acc);
                    new_idx
                };

                let str_val = match v {
                    serde_json::Value::Null => String::new(),
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                accumulators[idx].observe(&str_val);
            }

            // Record missing keys in this JSON object as nulls
            for (k, &idx) in &column_index {
                if !obj.contains_key(k) {
                    accumulators[idx].observe("");
                }
            }
        }
    }

    let columns: Vec<ColumnStats> = accumulators
        .into_iter()
        .map(ColumnAccumulator::finalize)
        .collect();

    Ok(FileAuditReport {
        file_path: path.to_string(),
        file_format: "jsonl".to_string(),
        row_count,
        column_count: columns.len(),
        duplicate_row_count,
        columns,
    })
}

#[pymodule]
fn cull_core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ColumnStats>()?;
    m.add_class::<FileAuditReport>()?;
    m.add_function(wrap_pyfunction!(audit_csv_file, m)?)?;
    m.add_function(wrap_pyfunction!(audit_jsonl_file, m)?)?;
    Ok(())
}
