use std::str::Lines;
use super::Table;


pub trait CellContent {
    fn get_width(&self) -> usize;
    fn get_height(&self) -> usize;
    fn get_lines(&self) -> Vec<String>;
}

impl<T: CellContent + Default> CellContent for Table<T> {
    fn get_width(&self) -> usize {
        self.to_string().lines().map(str::len).max().unwrap_or(0)
    }

    fn get_height(&self) -> usize {
        self.to_string().lines().count()
    }

    fn get_lines(&self) -> Vec<String> {
        self.to_string().lines().map(str::to_owned).collect()
    }
}


#[derive(Clone, Default)]
pub struct CellLines {
    lines: Vec<String>,
}

/*impl<S: AsRef<str>> From<S> for CellLines {
    fn from(s: S) -> CellLines {
        CellLines {
            lines: s.as_ref().lines().map(str::to_owned).collect()
        }
    }
}*/

impl<T: ToString> From<T> for CellLines {
    fn from(value: T) -> CellLines {
        CellLines {
            lines: value.to_string().lines().map(str::to_owned).collect()
        }
    }
}

impl CellContent for CellLines {
    fn get_width(&self) -> usize {
        self.lines.iter().map(String::len).max().unwrap_or(0)
    }

    fn get_height(&self) -> usize {
        self.lines.len()
    }

    fn get_lines(&self) -> Vec<String> {
        self.lines.clone()
    }
}
