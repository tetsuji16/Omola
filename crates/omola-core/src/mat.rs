//! Reader for OpenModelica simulation results in MATLAB level-4 format
//! (`Aclass = "Atrajectory"`, `"1.1"`, `""`, `"binTrans"` or `"binNormal"`).
//!
//! The file contains six matrices in this order: `Aclass`, `name`,
//! `description`, `dataInfo`, `data_1` (parameters / constants) and `data_2`
//! (time-varying values). This module uses only the Rust standard library and
//! never invokes OMC. Dymola DSRES files use a related but different layout and
//! are intentionally rejected unless they are a measured, supported case.

use std::{fmt, fs, path::Path};

/// One variable stored in a result file.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultVariable {
    pub name: String,
    pub description: String,
    /// `true` when the value is stored in `data_1` (constant over time).
    pub is_parameter: bool,
}

/// A fully decoded trajectory result.
#[derive(Debug, Clone, PartialEq)]
pub struct TrajectoryResult {
    pub variables: Vec<ResultVariable>,
    /// Time samples (taken from the abscissa variable, usually `time`).
    pub time: Vec<f64>,
    /// `values[i]` holds one sample per entry in `time` for `variables[i]`.
    pub values: Vec<Vec<f64>>,
}

impl TrajectoryResult {
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.variables.iter().position(|v| v.name == name)
    }

    pub fn series(&self, name: &str) -> Option<&[f64]> {
        self.index_of(name).map(|i| self.values[i].as_slice())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatError(pub String);

impl fmt::Display for MatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid OpenModelica MAT result: {}", self.0)
    }
}
impl std::error::Error for MatError {}

fn err<T>(message: impl Into<String>) -> Result<T, MatError> {
    Err(MatError(message.into()))
}

/// Read and decode a result file from disk.
pub fn read_trajectory_file(path: &Path) -> Result<TrajectoryResult, MatError> {
    let bytes =
        fs::read(path).map_err(|e| MatError(format!("could not read {}: {e}", path.display())))?;
    read_trajectory(&bytes)
}

/// Decode a result file already loaded into memory.
pub fn read_trajectory(bytes: &[u8]) -> Result<TrajectoryResult, MatError> {
    let mut cursor = Cursor { bytes, pos: 0 };
    let aclass = cursor.matrix("Aclass")?;
    let class_rows = aclass.text_rows(false)?;
    if class_rows.first().map(String::as_str) != Some("Atrajectory") {
        return err("Aclass[0] is not `Atrajectory`");
    }
    let transposed = match class_rows.get(3).map(String::as_str) {
        Some("binTrans") => true,
        Some("binNormal") | None => false,
        Some(other) => return err(format!("unsupported Aclass storage `{other}`")),
    };
    let names = cursor.matrix("name")?.text_rows(transposed)?;
    let descriptions = cursor.matrix("description")?.text_rows(transposed)?;
    let info = cursor.matrix("dataInfo")?;
    let data_1 = cursor.matrix("data_1")?;
    let data_2 = cursor.matrix("data_2")?;

    let count = names.len();
    if descriptions.len() != count {
        return err("name and description counts differ");
    }
    // dataInfo is 4 x n (binTrans: rows=4, cols=n) or n x 4 (binNormal).
    let info_at = |var: usize, field: usize| -> Result<i64, MatError> {
        let index = if transposed {
            var * info.rows + field
        } else {
            field * info.rows + var
        };
        info.values
            .get(index)
            .map(|v| *v as i64)
            .ok_or_else(|| MatError("dataInfo is too small".into()))
    };
    // Number of variables stored per data set and number of samples.
    let (d1_vars, d1_samples) = if transposed {
        (data_1.rows, data_1.cols)
    } else {
        (data_1.cols, data_1.rows)
    };
    let (d2_vars, d2_samples) = if transposed {
        (data_2.rows, data_2.cols)
    } else {
        (data_2.cols, data_2.rows)
    };
    let sample = |matrix: &Matrix, vars: usize, samples: usize, column: usize, k: usize| -> f64 {
        let index = if transposed {
            k * vars + column
        } else {
            column * samples + k
        };
        matrix.values[index]
    };

    let mut variables = Vec::with_capacity(count);
    let mut values = Vec::with_capacity(count);
    for i in 0..count {
        let dataset = info_at(i, 0)?;
        let raw_index = info_at(i, 1)?;
        let column = raw_index.unsigned_abs() as usize;
        if column == 0 {
            return err(format!("variable `{}` has data index 0", names[i]));
        }
        let column = column - 1;
        let sign = if raw_index < 0 { -1.0 } else { 1.0 };
        // dataset 0 is the abscissa (time); OpenModelica stores it in data_2 column 1.
        let (matrix, vars, samples, is_parameter) = match dataset {
            1 => (&data_1, d1_vars, d1_samples, true),
            0 | 2 => (&data_2, d2_vars, d2_samples, false),
            other => {
                return err(format!(
                    "variable `{}` uses unknown data set {other}",
                    names[i]
                ))
            }
        };
        if column >= vars {
            return err(format!(
                "variable `{}` points outside its data matrix",
                names[i]
            ));
        }
        let series: Vec<f64> = if is_parameter {
            // Parameters store their value at start (and end); expand to the time grid.
            let value = sign * sample(matrix, vars, samples, column, 0);
            vec![value; d2_samples]
        } else {
            (0..samples)
                .map(|k| sign * sample(matrix, vars, samples, column, k))
                .collect()
        };
        variables.push(ResultVariable {
            name: names[i].clone(),
            description: descriptions[i].clone(),
            is_parameter,
        });
        values.push(series);
    }
    let time = match variables.iter().position(|v| v.name == "time") {
        Some(i) => values[i].clone(),
        None if d2_vars > 0 => (0..d2_samples)
            .map(|k| sample(&data_2, d2_vars, d2_samples, 0, k))
            .collect(),
        None => return err("result has no time-varying data"),
    };
    Ok(TrajectoryResult {
        variables,
        time,
        values,
    })
}

/// Encode a result in OpenModelica's `binTrans` layout (little endian, f64 data).
///
/// `result.variables` must contain `time`; it is written as the abscissa. Parameters go to
/// `data_1` (their first value), every other variable to `data_2`.
pub fn write_trajectory(result: &TrajectoryResult) -> Result<Vec<u8>, MatError> {
    let time_index = result
        .index_of("time")
        .ok_or_else(|| MatError("result has no `time` variable".into()))?;
    if result.values.len() != result.variables.len()
        || result.values.iter().any(|v| v.len() != result.time.len())
    {
        return err("every variable needs one value per time sample");
    }
    let mut info = Vec::with_capacity(result.variables.len());
    let mut params = Vec::new();
    let mut signals = vec![time_index];
    for (i, var) in result.variables.iter().enumerate() {
        if i == time_index {
            info.push([0, 1, 0, -1]);
        } else if var.is_parameter {
            params.push(i);
            info.push([1, params.len() as i32, 0, 0]);
        } else {
            signals.push(i);
            info.push([2, signals.len() as i32, 0, -1]);
        }
    }
    let mut out = Vec::new();
    let class_rows = ["Atrajectory", "1.1", "", "binTrans"];
    write_header(&mut out, 51, 4, 11, "Aclass");
    for j in 0..11 {
        for row in class_rows {
            out.push(*row.as_bytes().get(j).unwrap_or(&b' '));
        }
    }
    let names: Vec<&str> = result.variables.iter().map(|v| v.name.as_str()).collect();
    let descriptions: Vec<&str> = result
        .variables
        .iter()
        .map(|v| v.description.as_str())
        .collect();
    write_text_columns(&mut out, "name", &names);
    write_text_columns(&mut out, "description", &descriptions);
    write_header(&mut out, 20, 4, info.len(), "dataInfo");
    for column in &info {
        for value in column {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    write_header(&mut out, 0, params.len(), 2, "data_1");
    for _ in 0..2 {
        for &p in &params {
            let value = result.values[p].first().copied().unwrap_or(0.0);
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    write_header(&mut out, 0, signals.len(), result.time.len(), "data_2");
    for (k, _) in result.time.iter().enumerate() {
        for &s in &signals {
            out.extend_from_slice(&result.values[s][k].to_le_bytes());
        }
    }
    Ok(out)
}

fn write_header(out: &mut Vec<u8>, type_word: i32, rows: usize, cols: usize, name: &str) {
    for value in [
        type_word,
        rows as i32,
        cols as i32,
        0,
        name.len() as i32 + 1,
    ] {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out.extend_from_slice(name.as_bytes());
    out.push(0);
}

fn write_text_columns(out: &mut Vec<u8>, name: &str, strings: &[&str]) {
    let len = strings.iter().map(|s| s.len()).max().unwrap_or(0).max(1);
    write_header(out, 51, len, strings.len(), name);
    for s in strings {
        let mut bytes = s.as_bytes().to_vec();
        bytes.resize(len, 0);
        out.extend_from_slice(&bytes);
    }
}

struct Matrix {
    rows: usize,
    cols: usize,
    is_text: bool,
    values: Vec<f64>,
}

impl Matrix {
    /// Decode a text matrix into trimmed strings. With `transposed`, each column is one string.
    fn text_rows(&self, transposed: bool) -> Result<Vec<String>, MatError> {
        if !self.is_text && self.values.iter().any(|v| *v < 0.0 || *v > 255.0) {
            return err("expected a text matrix");
        }
        let (count, length) = if transposed {
            (self.cols, self.rows)
        } else {
            (self.rows, self.cols)
        };
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            let mut text = String::with_capacity(length);
            for j in 0..length {
                // Column-major storage: element (r, c) is at c * rows + r.
                let index = if transposed {
                    i * self.rows + j
                } else {
                    j * self.rows + i
                };
                let code = self.values[index] as u32;
                if code != 0 {
                    text.push(char::from_u32(code).unwrap_or('?'));
                }
            }
            out.push(text.trim_end().to_owned());
        }
        Ok(out)
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Cursor<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], MatError> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| MatError("unexpected end of file".into()))?;
        let slice = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn i32(&mut self, big_endian: bool) -> Result<i32, MatError> {
        let b: [u8; 4] = self.take(4)?.try_into().expect("4 bytes");
        Ok(if big_endian {
            i32::from_be_bytes(b)
        } else {
            i32::from_le_bytes(b)
        })
    }

    fn matrix(&mut self, expected: &str) -> Result<Matrix, MatError> {
        // Peek the type word in little endian; a big-endian file has M = 1.
        let raw = self.i32(false)?;
        let (type_word, big_endian) = if (0..=4052).contains(&raw) {
            (raw, false)
        } else {
            (raw.swap_bytes(), true)
        };
        let m = type_word / 1000;
        let o = (type_word / 100) % 10;
        let p = (type_word / 10) % 10;
        let t = type_word % 10;
        if o != 0 || !(m == 0 || m == 1) || t > 1 {
            return err(format!(
                "matrix `{expected}` has unsupported type {type_word}"
            ));
        }
        let rows = self.i32(big_endian)?;
        let cols = self.i32(big_endian)?;
        let imag = self.i32(big_endian)?;
        let name_len = self.i32(big_endian)?;
        if rows < 0 || cols < 0 || name_len <= 0 || imag != 0 {
            return err(format!("matrix `{expected}` has an invalid header"));
        }
        let name_bytes = self.take(name_len as usize)?;
        let name: String = name_bytes
            .iter()
            .take_while(|b| **b != 0)
            .map(|b| *b as char)
            .collect();
        if name != expected {
            return err(format!("expected matrix `{expected}`, found `{name}`"));
        }
        let (rows, cols) = (rows as usize, cols as usize);
        let count = rows
            .checked_mul(cols)
            .ok_or_else(|| MatError("matrix is too large".into()))?;
        let width = match p {
            0 => 8,
            1 | 2 => 4,
            3 | 4 => 2,
            5 => 1,
            _ => return err(format!("matrix `{expected}` has unknown precision {p}")),
        };
        let data = self.take(
            count
                .checked_mul(width)
                .ok_or_else(|| MatError("matrix is too large".into()))?,
        )?;
        let values = data
            .chunks_exact(width)
            .map(|c| decode(c, p, m == 1))
            .collect();
        Ok(Matrix {
            rows,
            cols,
            is_text: t == 1,
            values,
        })
    }
}

fn decode(c: &[u8], precision: i32, big_endian: bool) -> f64 {
    macro_rules! num {
        ($t:ty, $n:expr) => {{
            let b: [u8; $n] = c.try_into().expect("chunk width");
            if big_endian {
                <$t>::from_be_bytes(b) as f64
            } else {
                <$t>::from_le_bytes(b) as f64
            }
        }};
    }
    match precision {
        0 => num!(f64, 8),
        1 => num!(f32, 4),
        2 => num!(i32, 4),
        3 => num!(i16, 2),
        4 => num!(u16, 2),
        _ => c[0] as f64,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn header(out: &mut Vec<u8>, type_word: i32, rows: usize, cols: usize, name: &str) {
        for v in [
            type_word,
            rows as i32,
            cols as i32,
            0,
            name.len() as i32 + 1,
        ] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(name.as_bytes());
        out.push(0);
    }

    /// Text matrix stored binTrans-style: each string is one column.
    fn text(out: &mut Vec<u8>, name: &str, strings: &[&str]) {
        let len = strings.iter().map(|s| s.len()).max().unwrap_or(0).max(1);
        header(out, 51, len, strings.len(), name);
        for s in strings {
            let mut bytes = s.as_bytes().to_vec();
            bytes.resize(len, 0);
            out.extend_from_slice(&bytes);
        }
    }

    /// Aclass is always stored row-wise (4 rows), like OpenModelica does.
    fn aclass(out: &mut Vec<u8>) {
        let rows = ["Atrajectory", "1.1", "", "binTrans"];
        let len = 11;
        header(out, 51, rows.len(), len, "Aclass");
        for j in 0..len {
            for r in rows {
                out.push(*r.as_bytes().get(j).unwrap_or(&b' '));
            }
        }
    }

    /// Build a small binTrans result: time, x (state), k (parameter), y = -x (alias).
    pub(crate) fn sample_file() -> Vec<u8> {
        let mut out = Vec::new();
        aclass(&mut out);
        text(&mut out, "name", &["time", "x", "k", "y"]);
        text(
            &mut out,
            "description",
            &["Simulation time [s]", "state", "gain", ""],
        );
        header(&mut out, 20, 4, 4, "dataInfo");
        for col in [
            [0i32, 1, 0, -1],
            [2, 2, 0, -1],
            [1, 1, 0, 0],
            [2, -2, 0, -1],
        ] {
            for v in col {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        // data_1: 1 variable x 2 samples (start, stop).
        header(&mut out, 0, 1, 2, "data_1");
        for v in [2.5f64, 2.5] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        // data_2: 2 variables (time, x) x 3 samples.
        header(&mut out, 0, 2, 3, "data_2");
        for (t, x) in [(0.0f64, 1.0f64), (0.5, 0.6), (1.0, 0.3)] {
            out.extend_from_slice(&t.to_le_bytes());
            out.extend_from_slice(&x.to_le_bytes());
        }
        out
    }

    #[test]
    fn decodes_bintrans_result_with_parameters_and_aliases() {
        let result = read_trajectory(&sample_file()).expect("valid file");
        let names: Vec<_> = result.variables.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, ["time", "x", "k", "y"]);
        assert_eq!(result.time, [0.0, 0.5, 1.0]);
        assert_eq!(result.series("x").unwrap(), [1.0, 0.6, 0.3]);
        assert_eq!(result.series("y").unwrap(), [-1.0, -0.6, -0.3]);
        assert_eq!(result.series("k").unwrap(), [2.5, 2.5, 2.5]);
        assert!(result.variables[2].is_parameter);
        assert_eq!(result.variables[1].description, "state");
    }

    #[test]
    fn writer_round_trips_through_reader() {
        let original = read_trajectory(&sample_file()).unwrap();
        let bytes = write_trajectory(&original).unwrap();
        assert_eq!(read_trajectory(&bytes).unwrap(), original);
    }

    #[test]
    fn rejects_truncated_and_foreign_files() {
        let bytes = sample_file();
        assert!(read_trajectory(&bytes[..bytes.len() - 4]).is_err());
        assert!(read_trajectory(b"not a mat file").is_err());
    }
}
