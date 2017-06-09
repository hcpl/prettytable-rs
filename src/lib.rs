#![warn(missing_docs,
        unused_extern_crates,
        unused_import_braces,
        unused_qualifications)]
//! A formatted and aligned table printer written in rust
extern crate unicode_width;
extern crate term;
extern crate atty;
#[cfg(feature = "csv")]
extern crate csv;
#[macro_use]
extern crate lazy_static;
extern crate encode_unicode;

use std::io::{self, Write, Error};
#[cfg(feature = "csv")]
use std::io::Read;
use std::fmt;
#[cfg(feature = "csv")]
use std::path::Path;
use std::iter::{FromIterator, IntoIterator};
use std::slice::{Iter, IterMut};
use std::ops::{Index, IndexMut};
use std::mem::transmute;

use term::{Terminal, stdout};

pub mod cell;
pub mod cell_content;
pub mod row;
pub mod format;
mod utils;

use row::Row;
use cell::Cell;
use cell_content::{CellContent, CellLines};
use format::{TableFormat, LinePosition, consts};
use utils::StringWriter;

/// An owned printable table
#[derive(Clone, Debug)]
pub struct Table<T: CellContent> {
    format: Box<TableFormat>,
    titles: Box<Option<Row<T>>>,
    rows: Vec<Row<T>>,
}

/// A borrowed immutable `Table` slice
/// A `TableSlice` is obtained by slicing a `Table` with the `Slice::slice` method.
///
/// # Examples
/// ```rust
/// # #[macro_use] extern crate prettytable;
/// use prettytable::{Table, Slice};
/// # fn main() {
/// let table = table![[1, 2, 3], [4, 5, 6], [7, 8, 9]];
/// let slice = table.slice(1..);
/// slice.printstd(); // Prints only rows 1 and 2
///
/// //Also supports other syntax :
/// table.slice(..);
/// table.slice(..2);
/// table.slice(1..3);
/// # }
/// ```
///
#[derive(Clone, Debug)]
pub struct TableSlice<'a, T: 'a + CellContent> {
    format: &'a TableFormat,
    titles: &'a Option<Row<T>>,
    rows: &'a [Row<T>],
}

impl<'a, T: CellContent> TableSlice<'a, T> {
    /// Compute and return the number of column
    pub fn get_column_num(&self) -> usize {
        let mut cnum = 0;
        for r in self.rows {
            let l = r.len();
            if l > cnum {
                cnum = l;
            }
        }
        cnum
    }

    /// Get the number of rows
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Check if the table slice is empty
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Get an immutable reference to a row
    pub fn get_row(&self, row: usize) -> Option<&Row<T>> {
        self.rows.get(row)
    }

    /// Get the width of the column at position `col_idx`.
    /// Return 0 if the column does not exists;
    fn get_column_width(&self, col_idx: usize) -> usize {
        let mut width = match *self.titles {
            Some(ref t) => t.get_cell_width(col_idx),
            None => 0,
        };
        for r in self.rows {
            let l = r.get_cell_width(col_idx);
            if l > width {
                width = l;
            }
        }
        width
    }

    /// Get the width of all columns, and return a slice
    /// with the result for each column
    fn get_all_column_width(&self) -> Vec<usize> {
        let colnum = self.get_column_num();
        let mut col_width = vec![0usize; colnum];
        for i in 0..colnum {
            col_width[i] = self.get_column_width(i);
        }
        col_width
    }

    /// Returns an iterator over the immutable cells of the column specified by `column`
    pub fn column_iter(&self, column: usize) -> ColumnIter<T> {
        ColumnIter(self.rows.iter(), column)
    }

    /// Returns an iterator over immutable rows
    pub fn row_iter(&self) -> Iter<Row<T>> {
        self.rows.iter()
    }

    /// Internal only
    fn __print<W, F>(&self, out: &mut W, f: F) -> Result<(), Error>
        where T: Default,
              W: Write + ?Sized,
              F: Fn(&Row<T>, &mut W, &TableFormat, &[usize]) -> Result<(), Error>
    {
        // Compute columns width
        let col_width = self.get_all_column_width();
        self.format
            .print_line_separator(out, &col_width, LinePosition::Top)?;
        if let Some(ref t) = *self.titles {
            f(t, out, self.format, &col_width)?;
            self.format
                .print_line_separator(out, &col_width, LinePosition::Title)?;
        }
        // Print rows
        let mut iter = self.rows.into_iter().peekable();
        while let Some(r) = iter.next() {
            f(r, out, self.format, &col_width)?;
            if iter.peek().is_some() {
                self.format
                    .print_line_separator(out, &col_width, LinePosition::Intern)?;
            }
        }
        self.format
            .print_line_separator(out, &col_width, LinePosition::Bottom)?;
        out.flush()
    }

    /// Print the table to `out`
    pub fn print<W: Write + ?Sized>(&self, out: &mut W) -> Result<(), Error>
        where T: Default
    {
        self.__print(out, Row::print)
    }

    /// Print the table to terminal `out`, applying styles when needed
    pub fn print_term<W: Terminal + ?Sized>(&self, out: &mut W) -> Result<(), Error>
        where T: Default
    {
        self.__print(out, Row::print_term)
    }

    /// Print the table to standard output. Colors won't be displayed unless
    /// stdout is a tty terminal, or `force_colorize` is set to `true`.
    /// In ANSI terminals, colors are displayed using ANSI escape characters. When for example the
    /// output is redirected to a file, or piped to another program, the output is considered
    /// as not beeing tty, and ANSI escape characters won't be displayed unless `force colorize`
    /// is set to `true`.
    /// # Panic
    /// Panic if writing to standard output fails
    pub fn print_tty(&self, force_colorize: bool) where T: Default {
        let r = match (stdout(), atty::is(atty::Stream::Stdout) || force_colorize) {
            (Some(mut o), true) => self.print_term(&mut *o),
            _ => self.print(&mut io::stdout()),
        };
        if let Err(e) = r {
            panic!("Cannot print table to standard output : {}", e);
        }
    }

    /// Print the table to standard output. Colors won't be displayed unless
    /// stdout is a tty terminal. This means that if stdout is redirected to a file, or piped
    /// to another program, no color will be displayed.
    /// To force colors rendering, use `print_tty()` method.
    /// Calling `printstd()` is equivalent to calling `print_tty(false)`
    /// # Panic
    /// Panic if writing to standard output fails
    pub fn printstd(&self) where T: Default {
        self.print_tty(false);
    }

    /// Write the table to the specified writer.
    #[cfg(feature = "csv")]
    pub fn to_csv<W: Write>(&self, w: W) -> csv::Result<csv::Writer<W>> {
        self.to_csv_writer(csv::Writer::from_writer(w))
    }

    /// Write the table to the specified writer.
    ///
    /// This allows for format customisation.
    #[cfg(feature = "csv")]
    pub fn to_csv_writer<W: Write>(&self,
                                   mut writer: csv::Writer<W>)
                                   -> csv::Result<csv::Writer<W>> {
        for title in self.titles {
            writer.write(title.iter().map(|c| c.get_content()))?;
        }
        for row in self.rows {
            writer.write(row.iter().map(|c| c.get_content()))?;
        }

        writer.flush()?;
        Ok(writer)
    }
}

impl<'a, T: CellContent> IntoIterator for &'a TableSlice<'a, T> {
    type Item = &'a Row<T>;
    type IntoIter = Iter<'a, Row<T>>;
    fn into_iter(self) -> Self::IntoIter {
        self.row_iter()
    }
}

impl<T: CellContent> Table<T> {
    /// Create an empty table
    pub fn new() -> Table<T> {
        Table::init(Vec::new())
    }

    /// Create a table initialized with `rows`
    pub fn init(rows: Vec<Row<T>>) -> Table<T> {
        Table {
            rows: rows,
            titles: Box::new(None),
            format: Box::new(*consts::FORMAT_DEFAULT),
        }
    }

    /// Change the table format. Eg : Separators
    pub fn set_format(&mut self, format: TableFormat) {
        *self.format = format;
    }

    /// Get a mutable reference to the internal format
    pub fn get_format(&mut self) -> &mut TableFormat {
        &mut self.format
    }

    /// Compute and return the number of column
    pub fn get_column_num(&self) -> usize {
        self.as_ref().get_column_num()
    }

    /// Get the number of rows
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Check if the table is empty
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Set the optional title lines
    pub fn set_titles(&mut self, titles: Row<T>) {
        *self.titles = Some(titles);
    }

    /// Unset the title line
    pub fn unset_titles(&mut self) {
        *self.titles = None;
    }

    /// Get a mutable reference to a row
    pub fn get_mut_row(&mut self, row: usize) -> Option<&mut Row<T>> {
        self.rows.get_mut(row)
    }

    /// Get an immutable reference to a row
    pub fn get_row(&self, row: usize) -> Option<&Row<T>> {
        self.rows.get(row)
    }

    /// Append a row in the table, transferring ownership of this row to the table
    /// and returning a mutable reference to the row
    pub fn add_row(&mut self, row: Row<T>) -> &mut Row<T> {
        self.rows.push(row);
        let l = self.rows.len() - 1;
        &mut self.rows[l]
    }

    /// Append an empty row in the table. Return a mutable reference to this new row.
    pub fn add_empty_row(&mut self) -> &mut Row<T> where T: Default {
        self.add_row(Row::default())
    }

    /// Insert `row` at the position `index`, and return a mutable reference to this row.
    /// If index is higher than current numbers of rows, `row` is appended at the end of the table
    pub fn insert_row(&mut self, index: usize, row: Row<T>) -> &mut Row<T> {
        if index < self.rows.len() {
            self.rows.insert(index, row);
            &mut self.rows[index]
        } else {
            self.add_row(row)
        }
    }

    /// Modify a single element in the table
    pub fn set_element(&mut self, element: &T, column: usize, row: usize) -> Result<(), &str>
            where T: Clone {
        let rowline = self.get_mut_row(row).ok_or("Cannot find row")?;
        // TODO: If a cell already exist, copy it's alignment parameter
        rowline.set_cell(Cell::new(&element), column)
    }

    /// Remove the row at position `index`. Silently skip if the row does not exist
    pub fn remove_row(&mut self, index: usize) {
        if index < self.rows.len() {
            self.rows.remove(index);
        }
    }

    /// Return an iterator over the immutable cells of the column specified by `column`
    pub fn column_iter(&self, column: usize) -> ColumnIter<T> {
        ColumnIter(self.rows.iter(), column)
    }

    /// Return an iterator over the mutable cells of the column specified by `column`
    pub fn column_iter_mut(&mut self, column: usize) -> ColumnIterMut<T> {
        ColumnIterMut(self.rows.iter_mut(), column)
    }

    /// Returns an iterator over immutable rows
    pub fn row_iter(&self) -> Iter<Row<T>> {
        self.rows.iter()
    }

    /// Returns an iterator over mutable rows
    pub fn row_iter_mut(&mut self) -> IterMut<Row<T>> {
        self.rows.iter_mut()
    }

    /// Print the table to `out`
    pub fn print<W: Write + ?Sized>(&self, out: &mut W) -> Result<(), Error> where T: Default {
        self.as_ref().print(out)
    }

    /// Print the table to terminal `out`, applying styles when needed
    pub fn print_term<W: Terminal + ?Sized>(&self, out: &mut W) -> Result<(), Error> where T: Default {
        self.as_ref().print_term(out)
    }

    /// Print the table to standard output. Colors won't be displayed unless
    /// stdout is a tty terminal, or `force_colorize` is set to `true`.
    /// In ANSI terminals, colors are displayed using ANSI escape characters. When for example the
    /// output is redirected to a file, or piped to another program, the output is considered
    /// as not beeing tty, and ANSI escape characters won't be displayed unless `force colorize`
    /// is set to `true`.
    /// # Panic
    /// Panic if writing to standard output fails
    pub fn print_tty(&self, force_colorize: bool) where T: Default {
        self.as_ref().print_tty(force_colorize);
    }

    /// Print the table to standard output. Colors won't be displayed unless
    /// stdout is a tty terminal. This means that if stdout is redirected to a file, or piped
    /// to another program, no color will be displayed.
    /// To force colors rendering, use `print_tty()` method.
    /// Calling `printstd()` is equivalent to calling `print_tty(false)`
    /// # Panic
    /// Panic if writing to standard output fails
    pub fn printstd(&self) where T: Default {
        self.as_ref().printstd();
    }

    /// Write the table to the specified writer.
    #[cfg(feature = "csv")]
    pub fn to_csv<W: Write>(&self, w: W) -> csv::Result<csv::Writer<W>> {
        self.as_ref().to_csv(w)
    }

    /// Write the table to the specified writer.
    ///
    /// This allows for format customisation.
    #[cfg(feature = "csv")]
    pub fn to_csv_writer<W: Write>(&self, writer: csv::Writer<W>) -> csv::Result<csv::Writer<W>> {
        self.as_ref().to_csv_writer(writer)
    }
}

impl Table<CellLines> {
    /// Create a table from a CSV string
    ///
    /// For more customisability use `from_csv()`
    #[cfg(feature = "csv")]
    pub fn from_csv_string(csv_s: &str) -> csv::Result<Table<CellLines>> {
        Ok(Table::from_csv(&mut csv::Reader::from_string(csv_s).has_headers(false)))
    }

    /// Create a table from a CSV file
    ///
    /// For more customisability use `from_csv()`
    #[cfg(feature = "csv")]
    pub fn from_csv_file<P: AsRef<Path>>(csv_p: P) -> csv::Result<Table<CellLines>> {
        Ok(Table::from_csv(&mut csv::Reader::from_file(csv_p)?.has_headers(false)))
    }

    /// Create a table from a CSV reader
    #[cfg(feature = "csv")]
    pub fn from_csv<R: Read>(reader: &mut csv::Reader<R>) -> Table<CellLines> {
        Table::init(reader
                        .records()
                        .map(|row| {
                                 Row::new(row.unwrap()
                                              .into_iter()
                                              .map(|cell| Cell::new(&CellLines::from(cell)))
                                              .collect())
                             })
                        .collect())
    }
}

impl<T: CellContent> Index<usize> for Table<T> {
    type Output = Row<T>;
    fn index(&self, idx: usize) -> &Self::Output {
        &self.rows[idx]
    }
}

impl<'a, T: CellContent> Index<usize> for TableSlice<'a, T> {
    type Output = Row<T>;
    fn index(&self, idx: usize) -> &Self::Output {
        &self.rows[idx]
    }
}

impl<T: CellContent> IndexMut<usize> for Table<T> {
    fn index_mut(&mut self, idx: usize) -> &mut Self::Output {
        &mut self.rows[idx]
    }
}

impl<T: CellContent + Default> fmt::Display for Table<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.as_ref().fmt(fmt)
    }
}

impl<'a, T: CellContent + Default> fmt::Display for TableSlice<'a, T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let mut writer = StringWriter::new();
        if self.print(&mut writer).is_err() {
            return Err(fmt::Error);
        }
        fmt.write_str(writer.as_string())
    }
}

impl<T, A> FromIterator<A> for Table<T>
    where T: CellContent + Clone,
          A: IntoIterator<Item = T>,
{
    fn from_iter<I>(iterator: I) -> Table<T>
        where I: IntoIterator<Item = A>
    {
        Table::init(iterator.into_iter().map(Row::from).collect())
    }
}

impl<T, I, A> From<I> for Table<T>
    where T: CellContent + Clone,
          A: IntoIterator<Item = T>,
          I: IntoIterator<Item = A>
{
    fn from(it: I) -> Table<T> {
        Table::from_iter(it)
    }
}

impl<'a, T: CellContent> IntoIterator for &'a Table<T> {
    type Item = &'a Row<T>;
    type IntoIter = Iter<'a, Row<T>>;
    fn into_iter(self) -> Self::IntoIter {
        self.as_ref().row_iter()
    }
}

impl<'a, T: CellContent> IntoIterator for &'a mut Table<T> {
    type Item = &'a mut Row<T>;
    type IntoIter = IterMut<'a, Row<T>>;
    fn into_iter(self) -> Self::IntoIter {
        self.row_iter_mut()
    }
}

/// Iterator over immutable cells in a column
pub struct ColumnIter<'a, T: 'a + CellContent>(Iter<'a, Row<T>>, usize);

impl<'a, T: CellContent> Iterator for ColumnIter<'a, T> {
    type Item = &'a Cell<T>;
    fn next(&mut self) -> Option<&'a Cell<T>> {
        self.0.next().and_then(|row| row.get_cell(self.1))
    }
}

/// Iterator over mutable cells in a column
pub struct ColumnIterMut<'a, T: 'a + CellContent>(IterMut<'a, Row<T>>, usize);

impl<'a, T: CellContent> Iterator for ColumnIterMut<'a, T> {
    type Item = &'a mut Cell<T>;
    fn next(&mut self) -> Option<&'a mut Cell<T>> {
        self.0.next().and_then(|row| row.get_mut_cell(self.1))
    }
}

impl<'a, T: CellContent> AsRef<TableSlice<'a, T>> for TableSlice<'a, T> {
    fn as_ref(&self) -> &TableSlice<'a, T> {
        self
    }
}

impl<'a, T: CellContent> AsRef<TableSlice<'a, T>> for Table<T> {
    fn as_ref(&self) -> &TableSlice<'a, T> {
        unsafe {
            // All this is a bit hacky. Let's try to find something else
            let s = &mut *((self as *const Table<T>) as *mut Table<T>);
            s.rows.shrink_to_fit();
            transmute(self)
        }
    }
}

/// Trait implemented by types which can be sliced
pub trait Slice<'a, E> {
    /// Type output after slicing
    type Output: 'a;
    /// Get a slice from self
    fn slice(&'a self, arg: E) -> Self::Output;
}

impl<'a, T, /*S,*/ E> Slice<'a, E> for /*S*/ TableSlice<'a, T>
    where T: CellContent,
         // S: AsRef<TableSlice<'a, T>>,
          [Row<T>]: Index<E, Output = [Row<T>]>
{
    type Output = TableSlice<'a, T>;
    fn slice(&'a self, arg: E) -> Self::Output {
        //let sl = self.as_ref();
        TableSlice {
            format: self.format,
            titles: self.titles,
            rows: self.rows.index(arg),
        }
    }
}

/// Create a table filled with some values
///
/// All the arguments used for elements must implement the `std::string::ToString` trait
/// # Syntax
/// ```text
/// table!([Element1_ row1, Element2_ row1, ...], [Element1_row2, ...], ...);
/// ```
///
/// # Example
/// ```
/// # #[macro_use] extern crate prettytable;
/// # fn main() {
/// // Create a table initialized with some rows :
/// let tab = table!(["Element1", "Element2", "Element3"],
/// 				 [1, 2, 3],
/// 				 ["A", "B", "C"]
/// 				 );
/// # drop(tab);
/// # }
/// ```
///
/// Some style can also be given in table creation
///
/// ```
/// # #[macro_use] extern crate prettytable;
/// # fn main() {
/// let tab = table!([FrByl->"Element1", Fgc->"Element2", "Element3"],
/// 				 [FrBy => 1, 2, 3],
/// 				 ["A", "B", "C"]
/// 				 );
/// # drop(tab);
/// # }
/// ```
///
/// For details about style specifier syntax, check doc for [`Cell::style_spec`](cell/struct.Cell.html#method.style_spec) method
#[macro_export]
macro_rules! table {
    ($([$($content:tt)*]), *) => (
        $crate::Table::init(vec![$(row![$($content)*]), *])
    );
}

/// Create a table with `table!` macro, print it to standard output, then return this table for future usage.
///
/// The syntax is the same that the one for the `table!` macro
#[macro_export]
macro_rules! ptable {
    ($($content:tt)*) => (
        {
            let tab = table!($($content)*);
            tab.printstd();
            tab
        }
    );
}

#[cfg(test)]
mod tests {
    use Table;
    use Slice;
    use row::Row;
    use cell::Cell;
    use format;
    use format::consts::{FORMAT_DEFAULT, FORMAT_NO_LINESEP, FORMAT_NO_COLSEP, FORMAT_CLEAN};

    #[test]
    fn table() {
        let mut table = Table::new();
        table.add_row(Row::new(vec![Cell::new("a"), Cell::new("bc"), Cell::new("def")]));
        table.add_row(Row::new(vec![Cell::new("def"), Cell::new("bc"), Cell::new("a")]));
        table.set_titles(Row::new(vec![Cell::new("t1"), Cell::new("t2"), Cell::new("t3")]));
        let out = "\
+-----+----+-----+
| t1  | t2 | t3  |
+=====+====+=====+
| a   | bc | def |
+-----+----+-----+
| def | bc | a   |
+-----+----+-----+
";
        assert_eq!(table.to_string().replace("\r\n", "\n"), out);
        table.unset_titles();
        let out = "\
+-----+----+-----+
| a   | bc | def |
+-----+----+-----+
| def | bc | a   |
+-----+----+-----+
";
        assert_eq!(table.to_string().replace("\r\n", "\n"), out);
    }

    #[test]
    fn index() {
        let mut table = Table::new();
        table.add_row(Row::new(vec![Cell::new("a"), Cell::new("bc"), Cell::new("def")]));
        table.add_row(Row::new(vec![Cell::new("def"), Cell::new("bc"), Cell::new("a")]));
        table.set_titles(Row::new(vec![Cell::new("t1"), Cell::new("t2"), Cell::new("t3")]));
        assert_eq!(table[1][1].get_content(), "bc");

        table[1][1] = Cell::new("newval");
        assert_eq!(table[1][1].get_content(), "newval");

        let out = "\
+-----+--------+-----+
| t1  | t2     | t3  |
+=====+========+=====+
| a   | bc     | def |
+-----+--------+-----+
| def | newval | a   |
+-----+--------+-----+
";
        assert_eq!(table.to_string().replace("\r\n", "\n"), out);
    }

    #[test]
    fn table_size() {
        let mut table = Table::new();
        assert!(table.is_empty());
        assert!(table.as_ref().is_empty());
        assert_eq!(table.len(), 0);
        assert_eq!(table.as_ref().len(), 0);
        assert_eq!(table.get_column_num(), 0);
        assert_eq!(table.as_ref().get_column_num(), 0);
        table.add_empty_row();
        assert!(!table.is_empty());
        assert!(!table.as_ref().is_empty());
        assert_eq!(table.len(), 1);
        assert_eq!(table.as_ref().len(), 1);
        assert_eq!(table.get_column_num(), 0);
        assert_eq!(table.as_ref().get_column_num(), 0);
        table[0].add_cell(Cell::default());
        assert_eq!(table.get_column_num(), 1);
        assert_eq!(table.as_ref().get_column_num(), 1);
    }

    #[test]
    fn get_row() {
        let mut table = Table::new();
        table.add_row(Row::new(vec![Cell::new("a"), Cell::new("bc"), Cell::new("def")]));
        table.add_row(Row::new(vec![Cell::new("def"), Cell::new("bc"), Cell::new("a")]));
        assert!(table.get_row(12).is_none());
        assert!(table.get_row(1).is_some());
        assert_eq!(table.get_row(1).unwrap()[0].get_content(), "def");
        assert!(table.get_mut_row(12).is_none());
        assert!(table.get_mut_row(1).is_some());
        table.get_mut_row(1).unwrap().add_cell(Cell::new("z"));
        assert_eq!(table.get_row(1).unwrap()[3].get_content(), "z");
    }

    #[test]
    fn add_empty_row() {
        let mut table = Table::new();
        assert_eq!(table.len(), 0);
        table.add_empty_row();
        assert_eq!(table.len(), 1);
        assert_eq!(table[0].len(), 0);
    }

    #[test]
    fn remove_row() {
        let mut table = Table::new();
        table.add_row(Row::new(vec![Cell::new("a"), Cell::new("bc"), Cell::new("def")]));
        table.add_row(Row::new(vec![Cell::new("def"), Cell::new("bc"), Cell::new("a")]));
        table.remove_row(12);
        assert_eq!(table.len(), 2);
        table.remove_row(0);
        assert_eq!(table.len(), 1);
        assert_eq!(table[0][0].get_content(), "def");
    }

    #[test]
    fn insert_row() {
        let mut table = Table::new();
        table.add_row(Row::new(vec![Cell::new("a"), Cell::new("bc"), Cell::new("def")]));
        table.add_row(Row::new(vec![Cell::new("def"), Cell::new("bc"), Cell::new("a")]));
        table.insert_row(12, Row::new(vec![Cell::new("1"), Cell::new("2"), Cell::new("3")]));
        assert_eq!(table.len(), 3);
        assert_eq!(table[2][1].get_content(), "2");
        table.insert_row(1, Row::new(vec![Cell::new("3"), Cell::new("4"), Cell::new("5")]));
        assert_eq!(table.len(), 4);
        assert_eq!(table[1][1].get_content(), "4");
        assert_eq!(table[2][1].get_content(), "bc");
    }

    #[test]
    fn set_element() {
        let mut table = Table::new();
        table.add_row(Row::new(vec![Cell::new("a"), Cell::new("bc"), Cell::new("def")]));
        table.add_row(Row::new(vec![Cell::new("def"), Cell::new("bc"), Cell::new("a")]));
        assert!(table.set_element("foo", 12, 12).is_err());
        assert!(table.set_element("foo", 1, 1).is_ok());
        assert_eq!(table[1][1].get_content(), "foo");
    }

    #[test]
    fn no_linesep() {
        let mut table = Table::new();
        table.set_format(*FORMAT_NO_LINESEP);
        table.add_row(Row::new(vec![Cell::new("a"), Cell::new("bc"), Cell::new("def")]));
        table.add_row(Row::new(vec![Cell::new("def"), Cell::new("bc"), Cell::new("a")]));
        table.set_titles(Row::new(vec![Cell::new("t1"), Cell::new("t2"), Cell::new("t3")]));
        assert_eq!(table[1][1].get_content(), "bc");

        table[1][1] = Cell::new("newval");
        assert_eq!(table[1][1].get_content(), "newval");

        let out = "\
+-----+--------+-----+
| t1  | t2     | t3  |
| a   | bc     | def |
| def | newval | a   |
+-----+--------+-----+
";
        assert_eq!(table.to_string().replace("\r\n", "\n"), out);
    }

    #[test]
    fn no_colsep() {
        let mut table = Table::new();
        table.set_format(*FORMAT_NO_COLSEP);
        table.add_row(Row::new(vec![Cell::new("a"), Cell::new("bc"), Cell::new("def")]));
        table.add_row(Row::new(vec![Cell::new("def"), Cell::new("bc"), Cell::new("a")]));
        table.set_titles(Row::new(vec![Cell::new("t1"), Cell::new("t2"), Cell::new("t3")]));
        assert_eq!(table[1][1].get_content(), "bc");

        table[1][1] = Cell::new("newval");
        assert_eq!(table[1][1].get_content(), "newval");

        let out = "\
------------------
 t1   t2      t3 \n\
==================
 a    bc      def \n\
------------------
 def  newval  a \n\
------------------
";
        println!("{}", out);
        println!("____");
        println!("{}", table.to_string().replace("\r\n", "\n"));
        assert_eq!(table.to_string().replace("\r\n", "\n"), out);
    }

    #[test]
    fn clean() {
        let mut table = Table::new();
        table.set_format(*FORMAT_CLEAN);
        table.add_row(Row::new(vec![Cell::new("a"), Cell::new("bc"), Cell::new("def")]));
        table.add_row(Row::new(vec![Cell::new("def"), Cell::new("bc"), Cell::new("a")]));
        table.set_titles(Row::new(vec![Cell::new("t1"), Cell::new("t2"), Cell::new("t3")]));
        assert_eq!(table[1][1].get_content(), "bc");

        table[1][1] = Cell::new("newval");
        assert_eq!(table[1][1].get_content(), "newval");

        let out = "\
\u{0020}t1   t2      t3 \n\
\u{0020}a    bc      def \n\
\u{0020}def  newval  a \n\
";
        println!("{}", out);
        println!("____");
        println!("{}", table.to_string().replace("\r\n", "\n"));
        assert_eq!(out, table.to_string().replace("\r\n", "\n"));
    }

    #[test]
    fn padding() {
        let mut table = Table::new();
        let mut format = *FORMAT_DEFAULT;
        format.padding(2, 2);
        table.set_format(format);
        table.add_row(Row::new(vec![Cell::new("a"), Cell::new("bc"), Cell::new("def")]));
        table.add_row(Row::new(vec![Cell::new("def"), Cell::new("bc"), Cell::new("a")]));
        table.set_titles(Row::new(vec![Cell::new("t1"), Cell::new("t2"), Cell::new("t3")]));
        assert_eq!(table[1][1].get_content(), "bc");

        table[1][1] = Cell::new("newval");
        assert_eq!(table[1][1].get_content(), "newval");

        let out = "\
+-------+----------+-------+
|  t1   |  t2      |  t3   |
+=======+==========+=======+
|  a    |  bc      |  def  |
+-------+----------+-------+
|  def  |  newval  |  a    |
+-------+----------+-------+
";
        println!("{}", out);
        println!("____");
        println!("{}", table.to_string().replace("\r\n", "\n"));
        assert_eq!(out, table.to_string().replace("\r\n", "\n"));
    }

    #[test]
    fn indent() {
        let mut table = Table::new();
        table.add_row(Row::new(vec![Cell::new("a"), Cell::new("bc"), Cell::new("def")]));
        table.add_row(Row::new(vec![Cell::new("def"), Cell::new("bc"), Cell::new("a")]));
        table.set_titles(Row::new(vec![Cell::new("t1"), Cell::new("t2"), Cell::new("t3")]));
        table.get_format().indent(8);
        let out = "        +-----+----+-----+
        | t1  | t2 | t3  |
        +=====+====+=====+
        | a   | bc | def |
        +-----+----+-----+
        | def | bc | a   |
        +-----+----+-----+
";
        assert_eq!(table.to_string().replace("\r\n", "\n"), out);
    }

    #[test]
    fn slices() {
        let mut table = Table::new();
        table.set_titles(Row::new(vec![Cell::new("t1"), Cell::new("t2"), Cell::new("t3")]));
        table.add_row(Row::new(vec![Cell::new("0"), Cell::new("0"), Cell::new("0")]));
        table.add_row(Row::new(vec![Cell::new("1"), Cell::new("1"), Cell::new("1")]));
        table.add_row(Row::new(vec![Cell::new("2"), Cell::new("2"), Cell::new("2")]));
        table.add_row(Row::new(vec![Cell::new("3"), Cell::new("3"), Cell::new("3")]));
        table.add_row(Row::new(vec![Cell::new("4"), Cell::new("4"), Cell::new("4")]));
        table.add_row(Row::new(vec![Cell::new("5"), Cell::new("5"), Cell::new("5")]));
        let out = "\
+----+----+----+
| t1 | t2 | t3 |
+====+====+====+
| 1  | 1  | 1  |
+----+----+----+
| 2  | 2  | 2  |
+----+----+----+
| 3  | 3  | 3  |
+----+----+----+
";
        let slice = table.slice(..);
        let slice = slice.slice(1..);
        let slice = slice.slice(..3);
        assert_eq!(out, slice.to_string().replace("\r\n", "\n"));
        assert_eq!(out, table.slice(1..4).to_string().replace("\r\n", "\n"));
    }

    #[test]
    fn test_unicode_separators() {
        let mut table = Table::new();
        table.set_format(format::FormatBuilder::new()
                             .column_separator('│')
                             .borders('│')
                             .separators(&[format::LinePosition::Top],
                                         format::LineSeparator::new('─',
                                                                    '┬',
                                                                    '┌',
                                                                    '┐'))
                             .separators(&[format::LinePosition::Intern],
                                         format::LineSeparator::new('─',
                                                                    '┼',
                                                                    '├',
                                                                    '┤'))
                             .separators(&[format::LinePosition::Bottom],
                                         format::LineSeparator::new('─',
                                                                    '┴',
                                                                    '└',
                                                                    '┘'))
                             .padding(1, 1)
                             .build());
        table.add_row(Row::new(vec![Cell::new("1"), Cell::new("1"), Cell::new("1")]));
        table.add_row(Row::new(vec![Cell::new("2"), Cell::new("2"), Cell::new("2")]));
        table.set_titles(Row::new(vec![Cell::new("t1"), Cell::new("t2"), Cell::new("t3")]));
        let out = "\
┌────┬────┬────┐
│ t1 │ t2 │ t3 │
├────┼────┼────┤
│ 1  │ 1  │ 1  │
├────┼────┼────┤
│ 2  │ 2  │ 2  │
└────┴────┴────┘
";
        println!("{}", out);
        println!("____");
        println!("{}", table.to_string().replace("\r\n", "\n"));
        assert_eq!(out, table.to_string().replace("\r\n", "\n"));
    }

    #[cfg(feature = "csv")]
    mod csv {
        use Table;
        use row::Row;
        use cell::Cell;

        static CSV_S: &'static str = "ABC,DEFG,HIJKLMN\n\
                                  foobar,bar,foo\n\
                                  foobar2,bar2,foo2\n";

        fn test_table() -> Table {
            let mut table = Table::new();
            table
                .add_row(Row::new(vec![Cell::new("ABC"), Cell::new("DEFG"), Cell::new("HIJKLMN")]));
            table.add_row(Row::new(vec![Cell::new("foobar"), Cell::new("bar"), Cell::new("foo")]));
            table.add_row(Row::new(vec![Cell::new("foobar2"),
                                        Cell::new("bar2"),
                                        Cell::new("foo2")]));
            table
        }

        #[test]
        fn from() {
            assert_eq!(test_table().to_string().replace("\r\n", "\n"),
                       Table::from_csv_string(CSV_S)
                           .unwrap()
                           .to_string()
                           .replace("\r\n", "\n"));
        }

        #[test]
        fn to() {
            assert_eq!(test_table().to_csv(Vec::new()).unwrap().as_string(), CSV_S);
        }

        #[test]
        fn trans() {
            assert_eq!(Table::from_csv_string(test_table()
                                                  .to_csv(Vec::new())
                                                  .unwrap()
                                                  .as_string())
                               .unwrap()
                               .to_string()
                               .replace("\r\n", "\n"),
                       test_table().to_string().replace("\r\n", "\n"));
        }
    }
}
