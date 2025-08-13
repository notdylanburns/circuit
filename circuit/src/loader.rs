use std::cell::OnceCell;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::diagnostics::{diagnostic, Diagnostic};
use crate::util::{Interner, Pos};

pub type ModuleId = usize;

#[derive(Debug)]
pub struct Module {
    pub id: ModuleId,
    path: Rc<Path>,
    path_string: String,
    file: File,
    length: usize,
    linemap: Vec<usize>,
    imports: OnceCell<HashMap<String, ModuleId>>,
}

impl Module {
    fn new(id: ModuleId, path: PathBuf) -> Result<Self, Diagnostic> {
        let path: Rc<Path> = Rc::from(path);
        let path_string = path.display().to_string();

        let mut file = File::open(&path).map_err(|e| {
            diagnostic!(
                Error,
                format!("failed to open module '{}': {}", &path_string, e),
            )
        })?;

        let mut content = String::new();
        let length = file.read_to_string(&mut content).map_err(|e| {
            diagnostic!(
                Error,
                format!("failed to read module '{}': {}", &path_string, e),
            )
        })?;

        file.seek(SeekFrom::Start(0)).map_err(|e| {
            diagnostic!(
                Error,
                format!("failed to seek in module '{}': {}", &path_string, e),
            )
        })?;

        let linemap = Self::get_linemap(&content);

        Ok(Self {
            id,
            path,
            path_string,
            file,
            length,
            linemap,
            imports: OnceCell::new(),
        })
    }

    pub fn id(&self) -> ModuleId {
        self.id
    }

    pub fn path(&self) -> Rc<Path> {
        Rc::clone(&self.path)
    }

    pub fn path_str(&self) -> &str {
        &self.path_string
    }

    pub fn set_imports(&mut self, imports: HashMap<String, ModuleId>) {
        self.imports
            .set(imports)
            .unwrap_or_else(|_| unreachable!("imports already set"));
    }

    pub fn read(&mut self) -> Result<String, Diagnostic> {
        let mut source = String::with_capacity(self.length);
        self.file.read_to_string(&mut source).map_err(|e| {
            diagnostic!(
                Error,
                format!("failed to read module '{}': {}", &self.path_string, e),
            )
        })?;

        Ok(source)
    }

    pub fn read_at(&mut self, offset: usize, length: usize) -> Result<String, Diagnostic> {
        if offset + length > self.length {
            unreachable!("read_at: offset + length exceeds source length")
        }

        self.file
            .seek(SeekFrom::Start(offset as u64))
            .map_err(|e| {
                diagnostic!(
                    Error,
                    format!("failed to seek in module '{}': {}", &self.path_string, e),
                )
            })?;

        let mut source = vec![0u8; length];
        self.file.read_exact(&mut source[..]).map_err(|e| {
            diagnostic!(
                Error,
                format!("failed to read from module '{}': {}", &self.path_string, e),
            )
        })?;

        Ok(String::from_utf8(source)
            .unwrap_or_else(|e| unreachable!("invalid utf-8 in module: {e}")))
    }

    pub fn line_count(&self, pos: Pos) -> usize {
        let Pos::Pos {
            line, start, span, ..
        } = pos
        else {
            unreachable!("is_multiline_span called with invalid pos: {pos:#?}");
        };

        let end_line = self.search_linemap(start + span);
        1 + end_line - line
    }

    pub fn read_line(&mut self, line_number: usize) -> Result<String, Diagnostic> {
        let (line_start, length) = self.get_line(line_number);

        self.read_at(line_start, length)
    }

    pub fn read_lines(&mut self, pos: Pos) -> Result<String, Diagnostic> {
        let Pos::Pos {
            line, start, span, ..
        } = pos
        else {
            unreachable!("read lines called with invalid pos: {pos:#?}");
        };

        let end_line = self.search_linemap(start + span);
        let (start, _) = self.get_line(line);
        let (_, end) = self.get_line(end_line);

        self.read_at(start, end - start)
    }

    fn get_linemap(source: &str) -> Vec<usize> {
        std::iter::repeat_n(0, 1)
            .chain(source.char_indices().filter_map(
                |(i, c)| {
                    if c == '\n' {
                        Some(i + 1)
                    } else {
                        None
                    }
                },
            ))
            .collect()
    }

    pub fn get_line(&self, line_number: usize) -> (usize, usize) {
        let line_start = self
            .linemap
            .get(line_number)
            .unwrap_or_else(|| unreachable!("line number out of bounds"));
        let line_end = self.linemap.get(line_number + 1).unwrap_or(&self.length);

        (*line_start, *line_end - *line_start - 1)
    }

    fn search_linemap(&self, pos: usize) -> usize {
        if pos >= self.length {
            unreachable!(
                "search_linemap: position {pos} exceeds module length {length}",
                length = self.length
            )
        }

        match self.linemap.binary_search(&pos) {
            Ok(index) => index,
            Err(0) => unreachable!("binary search returned 0 for position {pos}"),
            Err(index) => index - 1,
        }
    }
}

#[derive(Debug)]
pub struct Loader {
    interner: Interner<PathBuf>,
    modules: Vec<Module>,
}

impl Loader {
    pub const ROOT_MODULE: ModuleId = 0;

    #[cfg(target_os = "windows")]
    fn default_search_paths() -> [PathBuf; 2] {
        [PathBuf::from("."), PathBuf::from(r"C:\circuit\")]
    }

    #[cfg(target_os = "linux")]
    fn default_search_paths() -> [PathBuf; 3] {
        [
            PathBuf::from("."),
            PathBuf::from("/usr/lib/circuit"),
            PathBuf::from("/usr/local/lib/circuit"),
        ]
    }

    fn get_search_paths(module_name: &OsStr, relative_to: &Path) -> [PathBuf; 5] {
        let mut paths = <[PathBuf; 5]>::default();
        let _ = &mut paths[2..5].clone_from_slice(&Self::default_search_paths());

        let relative_to = relative_to.to_path_buf();

        paths[0] = relative_to.join(module_name);
        paths[1] = relative_to;

        paths
    }

    pub fn new() -> Self {
        Self {
            interner: Interner::new(),
            modules: Vec::new(),
        }
    }

    fn intern(&mut self, path: &Path) -> ModuleId {
        self.interner.intern_ref(path)
    }

    pub fn load_module(
        &mut self,
        name: &str,
        relative_to: ModuleId,
    ) -> Result<ModuleId, Diagnostic> {
        let module = self
            .modules
            .get(relative_to)
            .unwrap_or_else(|| unreachable!("invalid module id: {relative_to}"));

        let module_name = module.path.file_stem().unwrap_or_else(|| {
            unreachable!("module path '{}' has no file name", module.path.display())
        });

        let module_root = module.path.parent().unwrap_or_else(|| {
            unreachable!(
                "module path '{}' has no parent directory",
                module.path.display()
            )
        });

        let search_paths = Self::get_search_paths(module_name, module_root);
        for path in search_paths.iter() {
            let module_path = match path.join(name).with_extension("ckt").canonicalize() {
                Ok(path) => path,
                Err(_) => continue,
            };

            if !module_path.exists() {
                continue;
            }

            let module_id = self.intern(&module_path);
            if self.modules.len() <= module_id {
                self.modules.push(Module::new(module_id, module_path)?);
            }

            return Ok(module_id);
        }

        Err(diagnostic!(
            Error,
            format!(
                "module '{}' not found in search paths: {:?}",
                name, search_paths
            ),
        ))
    }

    pub fn load_root_module(&mut self, path: &str) -> Result<&mut Module, Diagnostic> {
        if !self.modules.is_empty() {
            unreachable!("root module should be first module loaded");
        };

        let path = match PathBuf::from(path).canonicalize() {
            Ok(path) => path,
            Err(e) => {
                return Err(diagnostic!(
                    Error,
                    format!("invalid root module path '{}': {}", path, e),
                ))
            }
        };

        let module_id = self.intern(&path);
        assert_eq!(module_id, Self::ROOT_MODULE, "root module id must be 0");
        self.modules.push(Module::new(module_id, path)?);

        return Ok(&mut self.modules[module_id]);
    }

    pub fn get_module(&self, module_id: ModuleId) -> Option<&Module> {
        self.modules.get(module_id)
    }

    pub fn get_module_mut(&mut self, module_id: ModuleId) -> Option<&mut Module> {
        self.modules.get_mut(module_id)
    }

    pub fn modules(&self) -> &[Module] {
        &self.modules
    }

    pub fn module_resolver<'a>(&'a self) -> impl Fn(ModuleId) -> Option<&'a Module> {
        move |module_id| self.modules.get(module_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_read_lines() {
        let mut module =
            Module::new(0, PathBuf::from("tests/loader/module_read_lines.ckt")).unwrap();
        assert_eq!(module.read_line(5).unwrap(), "    circ A {");
    }
}
