// SPDX-License-Identifier: Apache-2.0

//! Lightweight table printer for consistent formatting.

/// A simple table printer for rendering rows with consistent column alignment.
#[allow(dead_code)]
pub struct TablePrinter {
    /// Column widths for alignment.
    column_widths: Vec<usize>,
    /// Rows of data.
    rows: Vec<Vec<String>>,
}

impl TablePrinter {
    /// Create a new table printer with the given column count.
    #[allow(dead_code)]
    pub fn new(column_count: usize) -> Self {
        Self {
            column_widths: vec![0; column_count],
            rows: Vec::new(),
        }
    }

    /// Add a row to the table, updating column widths as needed.
    #[allow(dead_code)]
    pub fn add_row(&mut self, cells: &[&str]) {
        let cells: Vec<String> = cells.iter().map(std::string::ToString::to_string).collect();
        for (i, cell) in cells.iter().enumerate() {
            if i < self.column_widths.len() {
                self.column_widths[i] = self.column_widths[i].max(cell.len());
            }
        }
        self.rows.push(cells);
    }

    /// Render the table as a formatted string.
    #[allow(dead_code)]
    pub fn render(&self) -> String {
        use std::fmt::Write;
        let mut output = String::new();

        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                if i < self.column_widths.len() {
                    let width = self.column_widths[i];
                    let _ = write!(output, "{cell:<width$}");
                    if i < row.len() - 1 {
                        output.push_str("  ");
                    }
                } else {
                    output.push_str(cell);
                }
            }
            output.push('\n');
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_printer_basic() {
        let mut table = TablePrinter::new(2);
        table.add_row(&["Name", "Age"]);
        table.add_row(&["Alice", "30"]);
        table.add_row(&["Bob", "25"]);

        let output = table.render();
        assert!(output.contains("Name"));
        assert!(output.contains("Alice"));
        assert!(output.contains("Bob"));
    }

    #[test]
    fn test_table_printer_alignment() {
        let mut table = TablePrinter::new(2);
        table.add_row(&["A", "B"]);
        table.add_row(&["LongName", "X"]);

        let output = table.render();
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with('A'));
        assert!(lines[1].starts_with("LongName"));
    }
}
