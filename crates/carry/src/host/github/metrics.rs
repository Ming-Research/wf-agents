//! What a measurement's run measured, as the CSV file it uploads holds it: a header line naming the columns, then one
//! row for each thing measured. The columns its declaration names as metrics each hold a number in every row; every
//! other column says what a row measured. The engine checks that much and gives the file as it is: it works out nothing
//! of its own from it, and a file that is not so is said with why, never given in part.
//!
//! A value holds no comma, or is quoted as CSV quotes one, a quote inside it doubled; no value spans two lines.

use flow::agents::Measurement;

use super::api::File;

/// What a measurement's run measured, as its file holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Table {
    /// The file's lines, without the blank lines at its end.
    pub(super) text: String,
    /// How many rows it holds, its header not counted.
    pub(super) rows: usize,
}

/// The file the measurement's declaration names, among the files of the artifact, read and checked; or why it cannot
/// be: the artifact does not hold it, it is not UTF-8, it has no header line, its header names a column twice or lacks
/// a metric, a row has not as many values as the header names, or a metric of a row is not a number.
pub(super) fn read(files: &[File], measurement: &Measurement) -> Result<Table, String> {
    let file = &measurement.file;
    let Some((_, bytes)) = files.iter().find(|(path, _)| path == file) else {
        let held: Vec<&str> = files.iter().map(|(path, _)| path.as_str()).collect();
        return Err(format!("it holds no file {file}, but {}", held.join(", ")));
    };
    let text = std::str::from_utf8(bytes).map_err(|_| format!("{file} is not UTF-8"))?;
    let text = text.trim_end();
    let mut lines = text.lines().enumerate();
    let header = match lines.next() {
        Some((_, line)) if !line.trim().is_empty() => values(line).map_err(|why| format!("{file}, line 1: {why}"))?,
        _ => return Err(format!("{file} is empty: it has no header line naming its columns")),
    };
    for (at, column) in header.iter().enumerate() {
        if header[..at].contains(column) {
            return Err(format!("{file} names the column {column} twice"));
        }
    }
    let mut metrics = Vec::new();
    for metric in measurement.metrics.keys() {
        match header.iter().position(|column| column == metric) {
            Some(at) => metrics.push((metric, at)),
            None => {
                return Err(format!(
                    "{file} has no column {metric}, which the measurement's declaration names as a metric"
                ));
            }
        }
    }
    let mut rows = 0;
    for (at, line) in lines {
        let number = at + 1;
        let row = values(line).map_err(|why| format!("{file}, line {number}: {why}"))?;
        if row.len() != header.len() {
            return Err(format!(
                "{file}, line {number}: {} values, and its header names {} columns",
                row.len(),
                header.len()
            ));
        }
        for (metric, column) in &metrics {
            let value = &row[*column];
            if !value.trim().parse::<f64>().is_ok_and(f64::is_finite) {
                return Err(format!(
                    "{file}, line {number}: {metric} is {value:?}, which is no number"
                ));
            }
        }
        rows += 1;
    }
    Ok(Table {
        text: text.to_string(),
        rows,
    })
}

/// The values of a line of CSV, each unquoted; or why the line is not one.
fn values(line: &str) -> Result<Vec<String>, String> {
    let mut values = Vec::new();
    let mut chars = line.chars().peekable();
    loop {
        let mut value = String::new();
        if chars.peek() == Some(&'"') {
            chars.next();
            loop {
                match chars.next() {
                    Some('"') if chars.peek() == Some(&'"') => {
                        chars.next();
                        value.push('"');
                    }
                    Some('"') => break,
                    Some(c) => value.push(c),
                    None => return Err("a quoted value has no closing quote".to_string()),
                }
            }
            match chars.next() {
                None => {
                    values.push(value);
                    return Ok(values);
                }
                Some(',') => {}
                Some(c) => {
                    return Err(format!(
                        "{c:?} follows a quoted value, where a comma or the line's end goes"
                    ));
                }
            }
        } else {
            loop {
                match chars.next() {
                    None => {
                        values.push(value);
                        return Ok(values);
                    }
                    Some(',') => break,
                    Some('"') => return Err("a quote inside a value that is not quoted".to_string()),
                    Some(c) => value.push(c),
                }
            }
        }
        values.push(value);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    /// A measurement whose run writes `compare.csv`, the requests a second and the 99th percentile its metrics.
    fn compare() -> Measurement {
        Measurement {
            about: "the commit's image against where its change began".to_string(),
            workflow: "redis-bench.yml".to_string(),
            runner: "14900k".to_string(),
            too_long: "3h".to_string(),
            inputs: BTreeMap::from([("call".to_string(), "{call}".to_string())]),
            open: Vec::new(),
            artifact: "measurement".to_string(),
            file: "compare.csv".to_string(),
            metrics: BTreeMap::from([
                ("rps".to_string(), "requests/s".to_string()),
                ("p99_ms".to_string(), "ms".to_string()),
            ]),
        }
    }

    fn files(text: &str) -> Vec<File> {
        vec![
            ("host.txt".to_string(), b"kernel".to_vec()),
            ("compare.csv".to_string(), text.as_bytes().to_vec()),
        ]
    }

    // A file whose header names every metric and whose rows each hold a number in them is given as it is, its rows
    // counted, the blank lines at its end left off; a quoted value may hold a comma. Under a millisecond alone; the one
    // test of a file read whole.
    #[test]
    fn a_file_with_every_metric_a_number_in_each_row_is_given_as_it_is() {
        let text = "line,cpus,test,rps,p99_ms\nbase,2,\"set, 16\",151000.5,0.31\nhead,2,\"set, 16\",160200,0.29\n\n";
        let table = read(&files(text), &compare()).expect("read");
        assert_eq!(table.rows, 2);
        assert_eq!(table.text, text.trim_end());
        let header = read(&files("line,rps,p99_ms\n"), &compare()).expect("read");
        assert_eq!(header.rows, 0, "a file of a header alone holds no row, and says so");
    }

    // A file that is not as the declaration says is refused, saying where and why, and none of it is given: none in
    // the artifact, empty, a metric missing from its header, a column named twice, a row of another width, a metric
    // that is no number or is empty, and a quote left open. Under a millisecond alone; the one test of what refuses a
    // file.
    #[test]
    fn a_file_that_is_not_as_the_declaration_says_is_refused_saying_where_and_why() {
        let refused = |text: &str| read(&files(text), &compare()).expect_err(text);
        assert_eq!(
            read(&[("host.txt".to_string(), Vec::new())], &compare()).expect_err("no file"),
            "it holds no file compare.csv, but host.txt"
        );
        assert!(refused("").contains("compare.csv is empty"));
        assert!(refused("line,rps\nbase,1\n").contains("has no column p99_ms"));
        assert!(refused("rps,p99_ms,rps\n1,2,3\n").contains("names the column rps twice"));
        assert!(refused("line,rps,p99_ms\nbase,1\n").contains("line 2: 2 values, and its header names 3 columns"));
        assert!(refused("line,rps,p99_ms\nbase,1,fast\n").contains("line 2: p99_ms is \"fast\", which is no number"));
        assert!(refused("line,rps,p99_ms\nbase,,0.3\n").contains("line 2: rps is \"\", which is no number"));
        assert!(refused("line,rps,p99_ms\nbase,1,NaN\n").contains("which is no number"));
        assert!(refused("line,rps,p99_ms\n\"base,1,2\n").contains("line 2: a quoted value has no closing quote"));
        assert!(
            read(&[("compare.csv".to_string(), vec![0xff])], &compare())
                .expect_err("bytes")
                .contains("not UTF-8")
        );
    }
}
