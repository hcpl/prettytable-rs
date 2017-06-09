//! This module contains definition of table/row cells stuff

use std::io::{Write, Error};
use std::string::ToString;
use unicode_width::UnicodeWidthStr;
use term::{Attr, Terminal, color};
use super::cell_content::CellContent;
use super::format::Alignment;
use super::utils::print_align;

/// Represent a table cell containing a string.
///
/// Once created, a cell's content cannot be modified.
/// The cell would have to be replaced by another one
#[derive(Clone, Debug)]
pub struct Cell<T: CellContent> {
    content: T,
    align: Alignment,
    style: Vec<Attr>,
}

impl<T: CellContent> Cell<T> {
    /// Create a new `Cell` initialized with content from `string`.
    /// Text alignment in cell is configurable with the `align` argument
    pub fn new_align(content: &T, align: Alignment) -> Cell<T>
            where T: Clone {
        Cell {
            content: content.clone(),
            align: align,
            style: Vec::new(),
        }
    }

    /// Create a new `Cell` initialized with content from `string`.
    /// By default, content is align to `LEFT`
    pub fn new(content: &T) -> Cell<T>
            where T: Clone {
        Cell::new_align(content, Alignment::LEFT)
    }

    /// Set text alignment in the cell
    pub fn align(&mut self, align: Alignment) {
        self.align = align;
    }

    /// Add a style attribute to the cell
    pub fn style(&mut self, attr: Attr) {
        self.style.push(attr);
    }

    /// Add a style attribute to the cell. Can be chained
    pub fn with_style(mut self, attr: Attr) -> Cell<T> {
        self.style(attr);
        self
    }

    /// Remove all style attributes and reset alignment to default (LEFT)
    pub fn reset_style(&mut self) {
        self.style.clear();
        self.align(Alignment::LEFT);
    }

    /// Set the cell's style by applying the given specifier string
    ///
    /// # Style spec syntax
    ///
    /// The syntax for the style specifier looks like this :
    /// **FrBybl** which means **F**oreground **r**ed **B**ackground **y**ellow **b**old **l**eft
    ///
    /// ### List of supported specifiers :
    ///
    /// * **F** : **F**oreground (must be followed by a color specifier)
    /// * **B** : **B**ackground (must be followed by a color specifier)
    /// * **b** : **b**old
    /// * **i** : **i**talic
    /// * **u** : **u**nderline
    /// * **c** : Align **c**enter
    /// * **l** : Align **l**eft
    /// * **r** : Align **r**ight
    /// * **d** : **d**efault style
    ///
    /// ### List of color specifiers :
    ///
    /// * **r** : Red
    /// * **b** : Blue
    /// * **g** : Green
    /// * **y** : Yellow
    /// * **c** : Cyan
    /// * **m** : Magenta
    /// * **w** : White
    /// * **d** : Black
    ///
    /// And capital letters are for **bright** colors.
    /// Eg :
    ///
    /// * **R** : Bright Red
    /// * **B** : Bright Blue
    /// * ... and so on ...
    pub fn style_spec(mut self, spec: &str) -> Cell<T> {
        self.reset_style();
        let mut foreground = false;
        let mut background = false;
        for c in spec.chars() {
            if foreground || background {
                let color = match c {
                    'r' => color::RED,
                    'R' => color::BRIGHT_RED,
                    'b' => color::BLUE,
                    'B' => color::BRIGHT_BLUE,
                    'g' => color::GREEN,
                    'G' => color::BRIGHT_GREEN,
                    'y' => color::YELLOW,
                    'Y' => color::BRIGHT_YELLOW,
                    'c' => color::CYAN,
                    'C' => color::BRIGHT_CYAN,
                    'm' => color::MAGENTA,
                    'M' => color::BRIGHT_MAGENTA,
                    'w' => color::WHITE,
                    'W' => color::BRIGHT_WHITE,
                    'd' => color::BLACK,
                    'D' => color::BRIGHT_BLACK,
                    _ => {
                        // Silently ignore unknown tags
                        foreground = false;
                        background = false;
                        continue;
                    }
                };
                if foreground {
                    self.style(Attr::ForegroundColor(color));
                } else if background {
                    self.style(Attr::BackgroundColor(color));
                }
                foreground = false;
                background = false;
            } else {
                match c {
                    'F' => foreground = true,
                    'B' => background = true,
                    'b' => self.style(Attr::Bold),
                    'i' => self.style(Attr::Italic(true)),
                    'u' => self.style(Attr::Underline(true)),
                    'c' => self.align(Alignment::CENTER),
                    'l' => self.align(Alignment::LEFT),
                    'r' => self.align(Alignment::RIGHT),
                    _ => { /* Silently ignore unknown tags */ }
                }
            }
        }
        self
    }

    /// Return the height of the cell
    pub fn get_height(&self) -> usize {
        self.content.get_height()
    }

    /// Return the width of the cell
    pub fn get_width(&self) -> usize {
        self.content.get_width()
    }

    /// Return a copy of the full string contained in the cell
    pub fn get_content(&self) -> String {
        self.content.get_lines().join("\n")
    }

    /// Print a partial cell to `out`. Since the cell may be multi-lined,
    /// `idx` is the line index to print. `col_width` is the column width used to
    /// fill the cells with blanks so it fits in the table.
    /// If `ìdx` is higher than this cell's height, it will print empty content
    pub fn print<W: Write + ?Sized>(&self,
                                    out: &mut W,
                                    idx: usize,
                                    col_width: usize,
                                    skip_right_fill: bool)
                                    -> Result<(), Error> {
        let lines = self.content.get_lines();
        let c = lines.get(idx).map(String::as_ref).unwrap_or("");
        print_align(out, self.align, c, ' ', col_width, skip_right_fill)
    }

    /// Apply style then call `print` to print the cell into a terminal
    pub fn print_term<W: Terminal + ?Sized>(&self,
                                            out: &mut W,
                                            idx: usize,
                                            col_width: usize,
                                            skip_right_fill: bool)
                                            -> Result<(), Error> {
        for a in &self.style {
            match out.attr(*a) {
                Ok(..) |
                Err(::term::Error::NotSupported) |
                Err(::term::Error::ColorOutOfRange) => (), // Ignore unsupported atrributes
                Err(e) => return Err(term_error_to_io_error(e)),
            };
        }
        self.print(out, idx, col_width, skip_right_fill)?;
        match out.reset() {
            Ok(..) |
            Err(::term::Error::NotSupported) |
            Err(::term::Error::ColorOutOfRange) => Ok(()),
            Err(e) => Err(term_error_to_io_error(e)),
        }
    }
}

fn term_error_to_io_error(te: ::term::Error) -> Error {
    match te {
        ::term::Error::Io(why) => why,
        _ => Error::new(::std::io::ErrorKind::Other, te),
    }
}

impl<'a, T: CellContent + Clone> From<&'a T> for Cell<T> {
    fn from(value: &T) -> Cell<T> {
        Cell::new(value)
    }
}

impl<T: CellContent> ToString for Cell<T> {
    fn to_string(&self) -> String {
        self.get_content()
    }
}

impl<T: CellContent + Default> Default for Cell<T> {
    /// Return a cell initialized with a single empty `String`, with LEFT alignment
    fn default() -> Cell<T> {
        Cell {
            content: T::default(),
            align: Alignment::LEFT,
            style: Vec::new(),
        }
    }
}

/// This macro simplifies `Cell` creation
///
/// Support 2 syntax : With and without style specification.
/// # Syntax
/// ```text
/// cell!(value);
/// ```
/// or
///
/// ```text
/// cell!(spec:value);
/// ```
/// Value must implement the `std::string::ToString` trait
///
/// For details about style specifier syntax, check doc for [`Cell::style_spec`](cell/struct.Cell.html#method.style_spec) method
/// # Example
/// ```
/// # #[macro_use] extern crate prettytable;
/// # fn main() {
/// let cell = cell!("value");
/// // Do something with the cell
/// # drop(cell);
/// // Create a cell with style (Red foreground, Bold, aligned to left);
/// let styled = cell!(Frbl->"value");
/// # drop(styled);
/// # }
/// ```
#[macro_export]
macro_rules! cell {
    () => ($crate::cell::Cell::default());
    ($value:expr) => ($crate::cell::Cell::new(&From::from($value)));
    ($style:ident -> $value:expr) => (cell!($value).style_spec(stringify!($style)));

    ($type:ty) => ($crate::cell::Cell::<$type>::default());
    ($type:ty, $value:expr) => ($crate::cell::Cell::<$type>::new(&From::from($value)));
    ($type:ty, $style:ident -> $value:expr) => (cell!($type, $value).style_spec(stringify!($style)));
}

#[cfg(test)]
mod tests {
    use cell::Cell;
    use utils::StringWriter;
    use format::Alignment;
    use term::{Attr, color};

    #[test]
    fn get_content() {
        let cell = Cell::new("test");
        assert_eq!(cell.get_content(), "test");
    }

    #[test]
    fn print_ascii() {
        let ascii_cell = Cell::new("hello");
        assert_eq!(ascii_cell.get_width(), 5);

        let mut out = StringWriter::new();
        let _ = ascii_cell.print(&mut out, 0, 10, false);
        assert_eq!(out.as_string(), "hello     ");
    }

    #[test]
    fn print_unicode() {
        let unicode_cell = Cell::new("привет");
        assert_eq!(unicode_cell.get_width(), 6);

        let mut out = StringWriter::new();
        let _ = unicode_cell.print(&mut out, 0, 10, false);
        assert_eq!(out.as_string(), "привет    ");
    }

    #[test]
    fn print_cjk() {
        let unicode_cell = Cell::new("由系统自动更新");
        assert_eq!(unicode_cell.get_width(), 14);
        let mut out = StringWriter::new();
        let _ = unicode_cell.print(&mut out, 0, 20, false);
        assert_eq!(out.as_string(), "由系统自动更新      ");
    }

    #[test]
    fn align_left() {
        let cell = Cell::new_align("test", Alignment::LEFT);
        let mut out = StringWriter::new();
        let _ = cell.print(&mut out, 0, 10, false);
        assert_eq!(out.as_string(), "test      ");
    }

    #[test]
    fn align_center() {
        let cell = Cell::new_align("test", Alignment::CENTER);
        let mut out = StringWriter::new();
        let _ = cell.print(&mut out, 0, 10, false);
        assert_eq!(out.as_string(), "   test   ");
    }

    #[test]
    fn align_right() {
        let cell = Cell::new_align("test", Alignment::RIGHT);
        let mut out = StringWriter::new();
        let _ = cell.print(&mut out, 0, 10, false);
        assert_eq!(out.as_string(), "      test");
    }

    #[test]
    fn style_spec() {
        let mut cell = Cell::new("test").style_spec("FrBBbuic");
        assert_eq!(cell.style.len(), 5);
        assert!(cell.style.contains(&Attr::Underline(true)));
        assert!(cell.style.contains(&Attr::Italic(true)));
        assert!(cell.style.contains(&Attr::Bold));
        assert!(cell.style.contains(&Attr::ForegroundColor(color::RED)));
        assert!(cell.style
                    .contains(&Attr::BackgroundColor(color::BRIGHT_BLUE)));
        assert_eq!(cell.align, Alignment::CENTER);

        cell = cell.style_spec("FDBwr");
        assert_eq!(cell.style.len(), 2);
        assert!(cell.style
                    .contains(&Attr::ForegroundColor(color::BRIGHT_BLACK)));
        assert!(cell.style.contains(&Attr::BackgroundColor(color::WHITE)));
        assert_eq!(cell.align, Alignment::RIGHT);

        // Test with invalid sepcifier chars
        cell = cell.clone();
        cell = cell.style_spec("FzBr");
        assert!(cell.style.contains(&Attr::BackgroundColor(color::RED)));
        assert_eq!(cell.style.len(), 1);
        cell = cell.style_spec("zzz");
        assert!(cell.style.is_empty());
    }

    #[test]
    fn reset_style() {
        let mut cell = Cell::new("test")
            .with_style(Attr::ForegroundColor(color::BRIGHT_BLACK))
            .with_style(Attr::BackgroundColor(color::WHITE));
        cell.align(Alignment::RIGHT);

        //style_spec("FDBwr");
        assert_eq!(cell.style.len(), 2);
        assert_eq!(cell.align, Alignment::RIGHT);
        cell.reset_style();
        assert_eq!(cell.style.len(), 0);
        assert_eq!(cell.align, Alignment::LEFT);
    }

    #[test]
    fn default_empty_cell() {
        let cell = Cell::default();
        assert_eq!(cell.align, Alignment::LEFT);
        assert!(cell.style.is_empty());
        assert_eq!(cell.get_content(), "");
        assert_eq!(cell.to_string(), "");
        assert_eq!(cell.get_height(), 1);
        assert_eq!(cell.get_width(), 0);
    }
}
