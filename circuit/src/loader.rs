use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::analyser::BuildConst;
use crate::diagnostics::{diagnostic, Diagnostic};
use crate::extlib::{self, DefinedAt, FromFfi};
use crate::util::{Interner, Pos};

pub type ModuleId = usize;

#[derive(Debug)]
pub struct StandardModule {
    pub id: ModuleId,
    path: Rc<Path>,
    path_string: String,
    file: File,
    length: usize,
    linemap: Vec<usize>,
}

impl StandardModule {
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
pub struct LibraryModule {
    pub id: ModuleId,
    pub path: Rc<Path>,
    pub path_string: String,
    lib: Option<libloading::Library>,
}

impl LibraryModule {
    fn new(id: ModuleId, path: PathBuf) -> Self {
        let path: Rc<Path> = Rc::from(path);
        let path_string = path.display().to_string();

        Self {
            id,
            path,
            path_string,
            lib: None,
        }
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

    pub fn initialise(
        &mut self,
        build_consts: &HashMap<String, BuildConst>,
    ) -> Result<extlib::Library, Diagnostic> {
        let build_consts = build_consts
            .iter()
            .map(|(name, value)| circuit_extlib::BuildConst {
                name: name.as_bytes().as_ptr() as *const u8,
                value: match value {
                    BuildConst::Int(i) => circuit_extlib::ConstValue::from(*i),
                    BuildConst::Bool(b) => circuit_extlib::ConstValue::from(*b),
                },
            })
            .collect::<Vec<_>>();

        unsafe {
            let lib = libloading::Library::new(self.path.as_os_str()).map_err(|e| {
                diagnostic!(
                    Error,
                    format!("failed to open module '{}': {e}", &self.path_string)
                )
            })?;

            let init: libloading::Symbol<'_, extlib::InitFn> = lib.get(b"init\0").map_err(|e| {
                diagnostic!(
                    Error,
                    format!("failed to initialise module '{}': {e}", &self.path_string)
                )
            })?;

            let result = self.initialise_internal(init, build_consts.len(), build_consts.as_ptr());

            // needed to keep the symbols loaded until we have finished analysing
            self.lib = Some(lib);

            result
        }
    }

    unsafe fn initialise_internal(
        &self,
        init: libloading::Symbol<extlib::InitFn>,
        argc: usize,
        argv: *const circuit_extlib::BuildConst,
    ) -> Result<extlib::Library, Diagnostic> {
        let circuit_extlib::InitResult { library, error } = init(argc, argv);

        if error.is_null() {
            return Ok(extlib::Library::from_ffi(&library));
        };

        Err(diagnostic!(
            Error,
            format!(
                "failed to initialise module '{}': {}",
                &self.path_string,
                std::ffi::CStr::from_ptr(error as *const i8).to_string_lossy(),
            )
        ))
    }
}

#[derive(Debug)]
pub struct LibrarySubmodule {
    pub id: ModuleId,
    pub parent: ModuleId,
    pub name: String,
    pub defined_at: DefinedAt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleType {
    Module,
    Library,
    Submodule,
}

#[derive(Debug)]
pub enum Module {
    Module(StandardModule),
    Library(LibraryModule),
    Submodule(LibrarySubmodule),
}

impl Module {
    pub fn id(&self) -> ModuleId {
        match self {
            Self::Module(m) => m.id,
            Self::Library(m) => m.id,
            Self::Submodule(m) => m.id,
        }
    }

    // pub fn path(&self) -> Rc<Path> {
    //     Rc::clone(match self {
    //         Self::Module(m) => &m.path,
    //         Self::Library(m) => &m.path,
    //     })
    // }

    // pub fn path_str(&self) -> &str {
    //     match self {
    //         Self::Module(m) => &m.path_string,
    //         Self::Library(m) => &m.path_string,
    //     }
    // }

    pub fn module_type(&self) -> ModuleType {
        match self {
            Self::Module(..) => ModuleType::Module,
            Self::Library(..) => ModuleType::Library,
            Self::Submodule(..) => ModuleType::Submodule,
        }
    }
}

impl From<StandardModule> for Module {
    fn from(value: StandardModule) -> Self {
        Self::Module(value)
    }
}

impl From<LibraryModule> for Module {
    fn from(value: LibraryModule) -> Self {
        Self::Library(value)
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
    fn system_search_paths() -> [PathBuf; 1] {
        [PathBuf::from(r"C:\circuit\")]
    }

    #[cfg(target_os = "windows")]
    fn default_search_paths() -> [PathBuf; 2] {
        let mut paths = <[PathBuf; 2]>::default();
        &mut paths[1..2].clone_from_slice(&Self::system_search_paths());

        paths[0] = PathBuf::from(".");

        paths
    }

    #[cfg(target_os = "linux")]
    fn system_search_paths() -> [PathBuf; 2] {
        [
            PathBuf::from("/usr/local/lib/circuit"),
            PathBuf::from("/usr/lib/circuit"),
        ]
    }

    #[cfg(target_os = "linux")]
    fn default_search_paths() -> [PathBuf; 3] {
        let mut paths = <[PathBuf; 3]>::default();
        &mut paths[1..3].clone_from_slice(&Self::system_search_paths());

        paths[0] = PathBuf::from(".");

        paths
    }

    fn get_search_paths(module_name: &OsStr, relative_to: &Path) -> [PathBuf; 5] {
        let mut paths = <[PathBuf; 5]>::default();
        &mut paths[2..5].clone_from_slice(&Self::default_search_paths());

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

        let search_paths: Box<[PathBuf]> = match module {
            Module::Module(module) => {
                let module_path = module.path();

                let module_name = module_path.file_stem().unwrap_or_else(|| {
                    unreachable!("module path '{}' has no file name", module.path_str())
                });

                let module_root = module_path.parent().unwrap_or_else(|| {
                    unreachable!(
                        "module path '{}' has no parent directory",
                        module.path_str()
                    )
                });

                Box::from(Self::get_search_paths(module_name, module_root))
            }
            _ => Box::from(Self::default_search_paths()),
        };

        for path in search_paths.iter() {
            if let Ok(module_path) = path.join(name).with_extension("ckt").canonicalize() {
                if module_path.exists() {
                    let module_id = self.intern(&module_path);
                    if self.modules.len() <= module_id {
                        self.modules
                            .push(StandardModule::new(module_id, module_path)?.into());
                    }

                    return Ok(module_id);
                }
            }

            let Ok(shared_obj_path) = path.join(name).with_extension("cktlib").canonicalize()
            else {
                continue;
            };

            if !shared_obj_path.exists() {
                continue;
            }

            let module_id = self.intern(&shared_obj_path);
            if self.modules.len() <= module_id {
                self.modules
                    .push(LibraryModule::new(module_id, shared_obj_path).into());
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
        self.modules
            .push(StandardModule::new(module_id, path)?.into());

        return Ok(&mut self.modules[module_id]);
    }

    #[cfg(target_os = "windows")]
    fn get_submodule_path(name: &str, defined_at: DefinedAt) -> String {
        // ':' is illegal in Windows file paths, so this cannot be a
        // real file on the system
        format!(
            "ckt:submodule:{name}:{}:{}:{}",
            defined_at.file, defined_at.line, defined_at.column
        )
    }

    #[cfg(target_os = "linux")]
    fn get_submodule_path(name: &str, defined_at: DefinedAt) -> String {
        format!(
            "ckt\0submodule\0{name}\0{}\0{}\0{}",
            defined_at.file, defined_at.line, defined_at.column
        )
    }

    pub fn insert_submodule(
        &mut self,
        parent: ModuleId,
        name: String,
        defined_at: DefinedAt,
    ) -> ModuleId {
        let path = Self::get_submodule_path(&name, defined_at);
        let module_id = self.intern(Path::new(&path));
        assert!(module_id >= self.modules.len(), "module id conflict");

        self.modules.push(Module::Submodule(LibrarySubmodule {
            id: module_id,
            parent,
            name,
            defined_at,
        }));

        module_id
    }

    pub fn get_submodule_backtrace<'a>(
        &'a self,
        mut module: &'a LibrarySubmodule,
    ) -> Box<[String]> {
        let mut backtrace = Vec::new();

        loop {
            backtrace.push(format!(
                "=> in module {} (defined {}:{}:{})",
                module.name,
                module.defined_at.file,
                module.defined_at.line,
                module.defined_at.column
            ));

            match &self.modules[module.parent] {
                Module::Submodule(parent) => {
                    module = parent;
                }
                Module::Library(..) => break,
                Module::Module(..) => break,
            }
        }

        backtrace.into_boxed_slice()
    }

    pub fn get_submodule_library_path(&self, module_id: ModuleId) -> &str {
        match &self.modules[module_id] {
            Module::Submodule(sub) => self.get_submodule_library_path(sub.parent),
            Module::Library(lib) => &lib.path_string,
            Module::Module(..) => unreachable!(),
        }
    }

    pub fn get_module_type(&self, module_id: ModuleId) -> Option<ModuleType> {
        self.get_module(module_id).map(|m| m.module_type())
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
            StandardModule::new(0, PathBuf::from("tests/loader/module_read_lines.ckt")).unwrap();
        assert_eq!(module.read_line(5).unwrap(), "    circ A {");
    }
}
