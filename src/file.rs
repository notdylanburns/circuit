use std::cell::RefCell;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::rc::Rc;

use crate::error::CktError;

macro_rules! wrap_io_error {
    ($i:literal, $f:expr, $($e:tt)+) => {
        ($($e)+).map_err(|e| CktError::IOError($i, $f.clone(), e))
    };
}

macro_rules! file_ref {
    (copy $f:expr) => {
        $f.clone()
    };

    ($f:expr) => {
        $f.borrow()
    };

    (mut $f:expr) => {
        $f.borrow_mut()
    };
}

#[derive(Debug)]
struct LineMap(Vec<usize>);

impl LineMap {
    pub fn new(input: &Vec<u8>) -> Self {
        Self(
            input.iter()
                .enumerate()
                .filter_map(|(i, c)| (*c == b'\n').then_some(i))
                .collect()
        )
    }

    fn get_location(&self, loc: usize) -> (usize, usize) {
        let (line, line_start) = self.0
            .iter()
            .take_while(|v| loc >= **v)
            .enumerate()
            .last()
            .and_then(|(line, line_start)| Some((line + 1, line_start)))
            .unwrap_or((0, &0));

        if line == 0 {
            (line + 1, loc - line_start + 1)
        } else {
            (line + 1, loc - line_start)
        }
    }
}

#[derive(Debug)]
pub struct File {
    pub path: String,
    lm: LineMap,
    file: fs::File,
}

impl File {
    pub fn new(path: String) -> Result<Self, CktError> {
        let mut file = wrap_io_error!("open", path, fs::File::open(&path))?;
        let mut content = Vec::new();
        wrap_io_error!("read", path, file.read_to_end(&mut content))?;

        Ok(Self {
            path: path.clone(),
            lm: LineMap::new(&content),
            file: wrap_io_error!("open", path, fs::File::open(&path))?,
        })
    }

    pub fn get_location(&self, loc: usize) -> (usize, usize) {
        self.lm.get_location(loc)
    }

    pub fn seek(&mut self, loc: usize) -> Result<u64, CktError> {
        wrap_io_error!("seek", self.path.clone(), self.file.seek(SeekFrom::Start(loc as u64)))
    }

    pub fn read_all(&mut self) -> Result<Vec<u8>, CktError> {
        let mut buf = Vec::new();
        wrap_io_error!("read", self.path.clone(), self.file.read_to_end(&mut buf))?;

        Ok(buf)
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, CktError> {
        wrap_io_error!("read", self.path.clone(), self.file.read(buf))
    }

    pub fn read_at(&mut self, loc: usize, buf: &mut [u8]) -> Result<usize, CktError> {
        self.seek(loc)?;
        self.read(buf)
    }

    pub fn read_line(&mut self, line: usize) -> Result<Vec<u8>, CktError> {
        if line == 0 {
            let mut buf = vec![0; self.lm.0[0]];
            self.read(&mut buf)?;
            return Ok(buf);
        }
        let loc = self.lm.0[line - 1];
        match self.lm.0.get(line) {
            Some(end) => {
                let mut buf = vec![0; end - loc];
                self.read_at(loc + 1, &mut buf)?;
                Ok(buf)
            },
            None => {
                self.seek(loc + 1)?;
                self.read_all()
            }
        }
    }

    pub fn as_ref(self) -> FileRef {
        std::rc::Rc::new(std::cell::RefCell::new(self))
    }
}

pub type FileRef = Rc<RefCell<File>>;

pub(crate) use file_ref;

#[test]
fn test_linemap() {
    let input_string = "01234\n56789\n\nABCDEF";
    let mut lm = LineMap::new(&Vec::from(input_string));
    println!("{:?}", lm.0);

    for (i, c) in input_string.chars().enumerate() {
        println!("{i} = {c:?} -> {:?}", lm.get_location(i));
    }
}

#[test]
fn test_readline() {
    let maybe_file = File::new("example.ckt".into());
    match maybe_file {
        Ok(mut file) => {
            let line = file.read_line(14).unwrap();
            println!("Line: '{}'", String::from_utf8(line).unwrap());
        },
        Err(e) => unreachable!()
    };
}