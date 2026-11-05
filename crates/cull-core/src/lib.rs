use pyo3::exceptions::PyIOError;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
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
        serde_json::to_string(self)
            .map_err(|e| PyIOError::new_err(format!("Serialization error: {e}")))
    }
}

#[pyclass(get_all)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RowPreview {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub offset: usize,
    pub limit: usize,
    pub total_rows: usize,
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

    fn is_null_token(trimmed: &str) -> bool {
        trimmed.is_empty()
            || trimmed.eq_ignore_ascii_case("null")
            || trimmed.eq_ignore_ascii_case("na")
            || trimmed.eq_ignore_ascii_case("nan")
    }

    fn observe(&mut self, raw: &str) {
        self.total_count += 1;
        let trimmed = raw.trim();

        if Self::is_null_token(trimmed) {
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
    let mut column_index: HashMap<String, usize> = HashMap::new();
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

/// Streams a paginated slice of rows from a CSV or JSONL file for the workbench data grid.
#[pyfunction]
#[pyo3(signature = (path, offset=0, limit=100))]
pub fn preview_rows(path: &str, offset: usize, limit: usize) -> PyResult<RowPreview> {
    if path.ends_with(".jsonl") || path.ends_with(".ndjson") {
        let file = File::open(path)
            .map_err(|e| PyIOError::new_err(format!("Failed to open JSONL '{path}': {e}")))?;
        let reader = BufReader::new(file);

        let mut headers: Vec<String> = Vec::new();
        let mut header_map: HashMap<String, usize> = HashMap::new();
        let mut raw_objs: Vec<serde_json::Map<String, serde_json::Value>> = Vec::new();
        let mut total_rows = 0usize;

        for line_res in reader.lines() {
            let line = line_res.map_err(|e| PyIOError::new_err(format!("Read error: {e}")))?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(serde_json::Value::Object(obj)) = serde_json::from_str(trimmed) {
                for k in obj.keys() {
                    if !header_map.contains_key(k) {
                        header_map.insert(k.clone(), headers.len());
                        headers.push(k.clone());
                    }
                }
                if total_rows >= offset && raw_objs.len() < limit {
                    raw_objs.push(obj);
                }
                total_rows += 1;
            }
        }

        let rows: Vec<Vec<String>> = raw_objs
            .into_iter()
            .map(|obj| {
                headers
                    .iter()
                    .map(|h| match obj.get(h) {
                        None | Some(serde_json::Value::Null) => String::new(),
                        Some(serde_json::Value::String(s)) => s.clone(),
                        Some(other) => other.to_string(),
                    })
                    .collect()
            })
            .collect();

        Ok(RowPreview {
            headers,
            rows,
            offset,
            limit,
            total_rows,
        })
    } else {
        let mut reader = csv::ReaderBuilder::new()
            .flexible(true)
            .from_path(path)
            .map_err(|e| PyIOError::new_err(format!("Failed to open CSV '{path}': {e}")))?;

        let headers: Vec<String> = reader
            .headers()
            .map_err(|e| PyIOError::new_err(format!("Failed to read headers: {e}")))?
            .iter()
            .map(ToString::to_string)
            .collect();

        let mut rows: Vec<Vec<String>> = Vec::new();
        let mut total_rows = 0usize;

        for record_res in reader.records() {
            let record = record_res.map_err(|e| PyIOError::new_err(format!("Row error: {e}")))?;
            if total_rows >= offset && rows.len() < limit {
                rows.push(record.iter().map(ToString::to_string).collect());
            }
            total_rows += 1;
        }

        Ok(RowPreview {
            headers,
            rows,
            offset,
            limit,
            total_rows,
        })
    }
}

/// Applies column dropping, renaming, exact deduplication, and non-null row filtering,
/// writing the cleaned output to `output_path` and returning the retained row count.
#[pyfunction]
#[pyo3(signature = (input_path, output_path, drop_columns=Vec::new(), rename_map=HashMap::new(), deduplicate=false, drop_null_columns=Vec::new()))]
pub fn clean_and_export_csv(
    input_path: &str,
    output_path: &str,
    drop_columns: Vec<String>,
    rename_map: HashMap<String, String>,
    deduplicate: bool,
    drop_null_columns: Vec<String>,
) -> PyResult<usize> {
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_path(input_path)
        .map_err(|e| PyIOError::new_err(format!("Failed to open input CSV: {e}")))?;

    let mut writer = csv::Writer::from_path(output_path)
        .map_err(|e| PyIOError::new_err(format!("Failed to create output CSV: {e}")))?;

    let headers = reader
        .headers()
        .map_err(|e| PyIOError::new_err(format!("Failed to read CSV headers: {e}")))?
        .clone();

    let drop_set: HashSet<String> = drop_columns.into_iter().collect();
    let non_null_set: HashSet<String> = drop_null_columns.into_iter().collect();

    let mut kept_indices: Vec<usize> = Vec::new();
    let mut output_headers: Vec<String> = Vec::new();
    let mut required_indices: Vec<usize> = Vec::new();

    for (idx, h) in headers.iter().enumerate() {
        if non_null_set.contains(h) {
            required_indices.push(idx);
        }
        if !drop_set.contains(h) {
            kept_indices.push(idx);
            let final_name = rename_map.get(h).cloned().unwrap_or_else(|| h.to_string());
            output_headers.push(final_name);
        }
    }

    writer
        .write_record(&output_headers)
        .map_err(|e| PyIOError::new_err(format!("Failed to write CSV headers: {e}")))?;

    let mut seen_rows: HashSet<Vec<String>> = HashSet::new();
    let mut written_rows = 0usize;

    for result in reader.records() {
        let record = result.map_err(|e| PyIOError::new_err(format!("Malformed CSV row: {e}")))?;

        let has_null_in_required = required_indices.iter().any(|&req_idx| {
            let val = record.get(req_idx).unwrap_or("").trim();
            ColumnAccumulator::is_null_token(val)
        });
        if has_null_in_required {
            continue;
        }

        let projected_row: Vec<String> = kept_indices
            .iter()
            .map(|&idx| record.get(idx).unwrap_or("").trim().to_string())
            .collect();

        if deduplicate && !seen_rows.insert(projected_row.clone()) {
            continue;
        }

        writer
            .write_record(&projected_row)
            .map_err(|e| PyIOError::new_err(format!("Failed to write row: {e}")))?;
        written_rows += 1;
    }

    writer
        .flush()
        .map_err(|e| PyIOError::new_err(format!("Failed to flush output CSV: {e}")))?;

    Ok(written_rows)
}

#[pymodule]
fn cull_core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ColumnStats>()?;
    m.add_class::<FileAuditReport>()?;
    m.add_class::<RowPreview>()?;
    m.add_function(wrap_pyfunction!(audit_csv_file, m)?)?;
    m.add_function(wrap_pyfunction!(audit_jsonl_file, m)?)?;
    m.add_function(wrap_pyfunction!(preview_rows, m)?)?;
    m.add_function(wrap_pyfunction!(clean_and_export_csv, m)?)?;
    Ok(())
}
