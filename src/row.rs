//! This module contains definition of table rows stuff
use std::io::{Write, Error};
use std::iter::FromIterator;
use std::slice::{Iter, IterMut};
use std::ops::{Index, IndexMut};

use term::Terminal;

use super::utils::NEWLINE;
use super::cell::Cell;
use super::cell_content::CellContent;
use super::format::{TableFormat, ColumnPosition};

/// Represent a table row made of cells
#[derive(Clone, Debug)]
pub struct Row<T: CellContent> {
    cells: Vec<Cell<T>>,
}

impl<T: CellContent> Row<T> {
    /// Create a new `Row` backed with `cells` vector
    pub fn new(cells: Vec<Cell<T>>) -> Row<T> {
        Row { cells: cells }
    }

    /// Create an row of length `size`, with empty strings stored
    pub fn empty() -> Row<T> {
        Row::new(Vec::new())
    }

    /// Get the number of cells in this row
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Check if the row is empty (has no cell)
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Get the height of this row
    pub fn get_height(&self) -> usize {
        let mut height = 1; // Minimum height must be 1 to print empty rows
        for cell in &self.cells {
            let h = cell.get_height();
            if h > height {
                height = h;
            }
        }
        height
    }

    /// Get the minimum width required by the cell in the column `column`.
    /// Return 0 if the cell does not exist in this row
    pub fn get_cell_width(&self, column: usize) -> usize {
        self.cells
            .get(column)
            .map(|cell| cell.get_width())
            .unwrap_or(0)
    }

    /// Get the cell at index `idx`
    pub fn get_cell(&self, idx: usize) -> Option<&Cell<T>> {
        self.cells.get(idx)
    }

    /// Get the mutable cell at index `idx`
    pub fn get_mut_cell(&mut self, idx: usize) -> Option<&mut Cell<T>> {
        self.cells.get_mut(idx)
    }

    /// Set the `cell` in the row at the given `column`
    pub fn set_cell(&mut self, cell: Cell<T>, column: usize) -> Result<(), &str> {
        if column >= self.len() {
            return Err("Cannot find cell");
        }
        self.cells[column] = cell;
        Ok(())
    }

    /// Append a `cell` at the end of the row
    pub fn add_cell(&mut self, cell: Cell<T>) {
        self.cells.push(cell);
    }

    /// Insert `cell` at position `index`. If `index` is higher than the row length,
    /// the cell will be appended at the end
    pub fn insert_cell(&mut self, index: usize, cell: Cell<T>) {
        if index < self.cells.len() {
            self.cells.insert(index, cell);
        } else {
            self.add_cell(cell);
        }
    }

    /// Remove the cell at position `index`. Silently skip if this cell does not exist
    pub fn remove_cell(&mut self, index: usize) {
        if index < self.cells.len() {
            self.cells.remove(index);
        }
    }

    /// Returns an immutable iterator over cells
    pub fn iter(&self) -> Iter<Cell<T>> {
        self.cells.iter()
    }

    /// Returns an mutable iterator over cells
    pub fn iter_mut(&mut self) -> IterMut<Cell<T>> {
        self.cells.iter_mut()
    }

    /// Internal only
    fn __print<W: Write + ?Sized, F>(&self,
                                     out: &mut W,
                                     format: &TableFormat,
                                     col_width: &[usize],
                                     f: F)
                                     -> Result<(), Error>
        where T: Default,
              F: Fn(&Cell<T>, &mut W, usize, usize, bool) -> Result<(), Error>
    {
        for i in 0..self.get_height() {
            //TODO: Wrap this into dedicated function one day
            out.write_all(&vec![b' '; format.get_indent()])?;
            format.print_column_separator(out, ColumnPosition::Left)?;
            let (lp, rp) = format.get_padding();
            for j in 0..col_width.len() {
                out.write_all(&vec![b' '; lp])?;
                let skip_r_fill = (j == col_width.len() - 1) &&
                                  format.get_column_separator(ColumnPosition::Right).is_none();
                match self.get_cell(j) {
                    Some(c) => f(c, out, i, col_width[j], skip_r_fill)?,
                    None => f(&Cell::default(), out, i, col_width[j], skip_r_fill)?,
                };
                out.write_all(&vec![b' '; rp])?;
                if j < col_width.len() - 1 {
                    format.print_column_separator(out, ColumnPosition::Intern)?;
                }
            }
            format.print_column_separator(out, ColumnPosition::Right)?;
            out.write_all(NEWLINE)?;
        }
        Ok(())
    }

    /// Print the row to `out`, with `separator` as column separator, and `col_width`
    /// specifying the width of each columns
    pub fn print<W: Write + ?Sized>(&self,
                                    out: &mut W,
                                    format: &TableFormat,
                                    col_width: &[usize])
                                    -> Result<(), Error>
        where T: Default
    {
        self.__print(out, format, col_width, Cell::print)
    }

    /// Print the row to terminal `out`, with `separator` as column separator, and `col_width`
    /// specifying the width of each columns. Apply style when needed
    pub fn print_term<W: Terminal + ?Sized>(&self,
                                            out: &mut W,
                                            format: &TableFormat,
                                            col_width: &[usize])
                                            -> Result<(), Error>
        where T: Default
    {
        self.__print(out, format, col_width, Cell::print_term)
    }
}

impl<T: CellContent> Default for Row<T> {
    fn default() -> Row<T> {
        Row::empty()
    }
}

impl<T: CellContent> Index<usize> for Row<T> {
    type Output = Cell<T>;
    fn index(&self, idx: usize) -> &Self::Output {
        &self.cells[idx]
    }
}

impl<T: CellContent> IndexMut<usize> for Row<T> {
    fn index_mut(&mut self, idx: usize) -> &mut Self::Output {
        &mut self.cells[idx]
    }
}

impl<T: CellContent> FromIterator<T> for Row<T> {
    fn from_iter<I>(iterator: I) -> Row<T>
        where I: IntoIterator<Item = T>
    {
        Row::new(iterator.into_iter().map(Cell::from).collect())
    }
}

impl<T, I> From<I> for Row<T>
    where T: CellContent,
          I: IntoIterator<Item = T>
{
    fn from(it: I) -> Row<T> {
        Row::from_iter(it)
    }
}

impl<'a, T: CellContent> IntoIterator for &'a Row<T> {
    type Item = &'a Cell<T>;
    type IntoIter = Iter<'a, Cell<T>>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T: CellContent> IntoIterator for &'a mut Row<T> {
    type Item = &'a mut Cell<T>;
    type IntoIter = IterMut<'a, Cell<T>>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

/// This macro simplifies `Row` creation
///
/// The syntax support style spec
/// # Example
/// ```
/// # #[macro_use] extern crate prettytable;
/// # fn main() {
/// // Create a normal row
/// let row1 = row!["Element 1", "Element 2", "Element 3"];
/// // Create a row with all cells formatted with red foreground color, yellow background color
/// // bold, italic, align in the center of the cell
/// let row2 = row![FrBybic => "Element 1", "Element 2", "Element 3"];
/// // Create a row with first cell in blue, second one in red, and last one with default style
/// let row3 = row![Fb->"blue", Fr->"red", "normal"];
/// // Do something with rows
/// # drop(row1);
/// # drop(row2);
/// # drop(row3);
/// # }
/// ```
///
/// For details about style specifier syntax, check doc for [`Cell::style_spec`](cell/struct.Cell.html#method.style_spec) method
#[macro_export]
macro_rules! row {
    (($($out:tt)*); $value:expr) => (vec![$($out)* cell!($value)]);
    (($($out:tt)*); $value:expr, $($n:tt)*) => (row!(($($out)* cell!($value),); $($n)*));
    (($($out:tt)*); $style:ident -> $value:expr) => (vec![$($out)* cell!($style -> $value)]);
    (($($out:tt)*); $style:ident -> $value:expr, $($n: tt)*) => (row!(($($out)* cell!($style -> $value),); $($n)*));

    ($($content:expr), *) => ($crate::row::Row::new(vec![$(cell!($content)), *]));
    ($style:ident => $($content:expr), *) => ($crate::row::Row::new(vec![$(cell!($style -> $content)), *]));
    ($($content:tt)*) => ($crate::row::Row::new(row!((); $($content)*)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use cell::Cell;

    #[test]
    fn row_default_empty() {
        let row1 = Row::default();
        assert_eq!(row1.len(), 0);
        assert!(row1.is_empty());
    }

    #[test]
    fn get_add_set_cell() {
        let mut row = Row::from(vec!["foo", "bar", "foobar"]);
        assert_eq!(row.len(), 3);
        assert!(row.get_mut_cell(12).is_none());
        let c1 = row.get_mut_cell(0).unwrap().clone();
        assert_eq!(c1.get_content(), "foo");

        let c1 = Cell::from(&"baz");
        assert!(row.set_cell(c1.clone(), 1000).is_err());
        assert!(row.set_cell(c1.clone(), 0).is_ok());
        assert_eq!(row.get_cell(0).unwrap().get_content(), "baz");

        row.add_cell(c1.clone());
        assert_eq!(row.len(), 4);
        assert_eq!(row.get_cell(3).unwrap().get_content(), "baz");
    }

    #[test]
    fn insert_cell() {
        let mut row = Row::from(vec!["foo", "bar", "foobar"]);
        assert_eq!(row.len(), 3);
        let cell = Cell::new("baz");
        row.insert_cell(1000, cell.clone());
        assert_eq!(row.len(), 4);
        assert_eq!(row.get_cell(3).unwrap().get_content(), "baz");
        row.insert_cell(1, cell.clone());
        assert_eq!(row.len(), 5);
        assert_eq!(row.get_cell(1).unwrap().get_content(), "baz");
    }

    #[test]
    fn remove_cell() {
        let mut row = Row::from(vec!["foo", "bar", "foobar"]);
        assert_eq!(row.len(), 3);
        row.remove_cell(1000);
        assert_eq!(row.len(), 3);
        row.remove_cell(1);
        assert_eq!(row.len(), 2);
        assert_eq!(row.get_cell(0).unwrap().get_content(), "foo");
        assert_eq!(row.get_cell(1).unwrap().get_content(), "foobar");
    }
}
