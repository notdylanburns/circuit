mod builder;
mod constexpr;
mod local;
mod path_tag;
mod scope;

use builder::CircBuilder;
use constexpr::{ConstType, ConstValue};
// use local::{Local, LocalType, LocalValue};
use path_tag::{PathTag, PathTagged};
use scope::{
    CircArg, CircSymbol, ConstSymbol, EnumSymbol, LocalSymbol, Scope, ScopeSymbol, ScopeSymbolType,
};

use crate::analyser::builder::EndpointBuilder;
use crate::analyser::local::LocalType;
use crate::ast::{
    ConnectionDirection, ConstExprOpType, Node, NodeType, PinDirection, PinExprOpType, Range, AST,
};
use crate::diagnostics::diagnostic;
use crate::loader::{Loader, Module, ModuleId};
use crate::parser::Parser;
use crate::tokeniser::{IdentId, Tokeniser};
use crate::util::{extract, Interner};

use crate::util::{OrderedMap, Pos, Position};
use crate::{Diagnostic, Diagnostics};
use std::collections::HashMap;
use std::rc::Rc;

// macro_rules! extract_node {
//     ($node:expr, $variant:ident { $field:ident $(,$fields:ident)* }) => {
//         match $node.node_type() {
//             NodeType::$variant { $field $(,$fields)* , ..} => ($field, $($fields),*),
//             _ => unreachable!("parser bug"),
//         }
//     };
//     ($node:expr, $variant:ident ( $field:ident $(,$fields:ident)* )) => {
//         match $node.node_type() {
//             NodeType::$variant ( $field $(,$fields)* , ..) => ($field, $($fields),*),
//             _ => unreachable!("parser bug"),
//         }
//     };
// }

// macro_rules! return_or_error {
//     ($value:expr, $errors:expr) => {
//         if $errors.is_empty() {
//             Ok($value)
//         } else {
//             Err($errors)
//         }
//     };
// }

// macro_rules! add_error {
//     ($errors:expr, $ident:ident $(($($arg:expr),+))?) => {
//         $errors.push(CompilerError::new(AnalyserError::$ident $(($($arg),+))?));
//     };
//     ($errors:expr, $ident:ident $(($($arg:expr),+))?, $pos:expr) => {
//         $errors.push(CompilerError::new(AnalyserError::$ident $(($($arg),+))?).with_pos($pos));
//     };
// }

// macro_rules! new_error {
//     ($ident:ident $(($($arg:expr),+))?) => {
//         vec![CompilerError::new(AnalyserError::$ident $(($($arg),+))?)]
//     };
//     ($ident:ident $(($($arg:expr),+))?, $pos:expr) => {
//         vec![CompilerError::new(AnalyserError::$ident $(($($arg),+))?).with_pos($pos)]
//     };

// }

macro_rules! catch_error {
    ($self:expr, $expr:expr, $default:expr) => {
        match $expr {
            Ok(value) => value,
            Err(ds) => {
                $self.diagnostics.push(ds);
                $default
            }
        }
    };
}

macro_rules! catch_errors {
    ($self:expr, $expr:expr, $default:expr) => {
        match $expr {
            Ok(value) => value,
            Err(ds) => {
                $self.diagnostics.append(ds);
                $default
            }
        }
    };
}

// #[derive(Debug, Clone, Copy, Eq, PartialEq)]
// enum PinDirection {
//     Input,
//     Output,
// }

// impl TryFrom<&str> for PinDirection {
//     type Error = ();

//     fn try_from(value: &str) -> Result<Self, Self::Error> {
//         match value {
//             "Input" => Ok(Self::Input),
//             "Output" => Ok(Self::Output),
//             _ => Err(()),
//         }
//     }
// }

// #[derive(Debug)]
// struct CircPin<'i> {
//     r#type: LocalType,
//     node: &'i Node<'i>,
// }

// #[derive(Debug)]
// struct CircInterface<'i> {
//     named_pins: HashMap<&'i str, CircPin<'i>>,
//     anonymous_pins: Vec<CircPin<'i>>,
// }

// impl<'i> CircInterface<'i> {
//     fn get_named_pin(&self, name: &'i str) -> Option<&CircPin<'i>> {
//         self.named_pins.get(name)
//     }

//     fn get_anonymous_pin(&self, index: &mut usize) -> Option<&CircPin<'i>> {
//         if *index < self.anonymous_pins.len() {
//             let pin = &self.anonymous_pins[*index];
//             *index += 1;
//             Some(pin)
//         } else {
//             None
//         }
//     }
// }

// #[derive(Debug)]
// enum ConnectionPinSource<'i> {
//     This,
//     Local(&'i str, usize),
// }

// #[derive(Debug)]
// enum ConnectionEndpointType {
//     Input,
//     Output,
//     Circ,
// }

// #[derive(Debug)]
// struct ConnectionEndpoint<'i> {
//     source: ConnectionPinSource<'i>,
//     r#type: ConnectionEndpointType,
//     name: &'i str,
//     start: usize,
//     end: usize,
// }

// impl<'i> ConnectionEndpoint<'i> {
//     const fn width(&self) -> usize {
//         self.end - self.start + 1
//     }
// }

// #[derive(Debug)]
// struct CircDescriptor<'i> {
//     interface: CircInterface<'i>,
//     scope: Scope<'i>,
//     connections: Vec<(ConnectionEndpoint<'i>, ConnectionEndpoint<'i>)>,
//     node: &'i Node<'i>,
// }

// impl<'i> CircDescriptor<'i> {}

type CircId = usize;
type PinId = usize;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CircSignature {
    module_id: ModuleId,
    name: IdentId,
    args: Rc<[ConstValue]>,
}

#[derive(Debug, Clone, Copy)]
struct Pin {
    name: IdentId,
    direction: PinDirection,
    width: usize,
}

#[derive(Debug, Clone, Copy)]
enum ConnectionRange {
    Single(usize),
    Range(Option<usize>, Option<usize>),
}

impl std::fmt::Display for ConnectionRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Single(index) => write!(f, "{index}"),
            Self::Range(None, None) => write!(f, ".."),
            Self::Range(Some(start), None) => write!(f, "{start}.."),
            Self::Range(None, Some(end)) => write!(f, "..{end}"),
            Self::Range(Some(start), Some(end)) => write!(f, "{start}..{end}"),
        }
    }
}

impl ConnectionRange {
    const FULL: Self = Self::Range(None, None);
}

#[derive(Debug, Clone, Copy)]
enum ConnectionEndpoint {
    Pin {
        pin_id: PinId,
        range: ConnectionRange,
    },
    Dependency {
        dependency_id: usize,
        pin_id: PinId,
        range: ConnectionRange,
    },
}

#[derive(Debug, Clone, Copy)]
enum ConnectionType {
    Unidirectional,
    Bidirectional,
}

#[derive(Debug, Clone, Copy)]
struct Connection {
    source: ConnectionEndpoint,
    dest: ConnectionEndpoint,
    connection_type: ConnectionType,
}

#[derive(Debug, Clone)]
struct Circ {
    dependencies: Rc<[CircId]>,
    pins: Rc<OrderedMap<IdentId, Pin>>,
    connections: Rc<[Connection]>,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
enum PinOrArray {
    Pin,
    Array,
}

#[derive(Debug, Clone, Copy)]
enum PinExprType {
    Unknown,
    Index(usize),
    Range(Option<usize>, Option<usize>),
    Pin {
        pin_id: PinId,
        pin: Pin,
    },
    PinRange {
        pin_id: PinId,
        pin: Pin,
        start: Option<usize>,
        end: Option<usize>,
    },
    DependencyArray {
        name: IdentId,
        circ_id: CircId,
        len: usize,
    },
    Dependency {
        name: IdentId,
        circ_id: CircId,
        index: Option<usize>,
    },
    DependencyPin {
        name: IdentId,
        circ_id: CircId,
        index: Option<usize>,
        pin_id: PinId,
        pin: Pin,
    },
    DependencyPinRange {
        name: IdentId,
        circ_id: CircId,
        index: Option<usize>,
        pin_id: PinId,
        pin: Pin,
        start: Option<usize>,
        end: Option<usize>,
    },
}

#[derive(Debug)]
pub enum AnalyserError {
    DuplicateArgument(String, String, Pos),
    DuplicateSymbol(String, Pos),
    DuplicateVariant(String, String, Pos),
    UnknownType(String),
    UnknownIdentifier(String),
    InvalidConstantType(String),
    InvalidBinaryOp(String, String, String),
    InvalidUnaryOp(String, String),
    MismatchedTypes(String, String),
    ArgDefaultTypeMismatch(String, String, Pos),
    AssertionFailed(Option<String>),
    PinWithTypeArgs,
    UnknownCirc(String),
    TooManyArgs(usize, usize),
    PositionalArgAfterNamedArg(Pos),
    UnknownArgument(String),
    MissingRequiredArg(String),
    DuplicatePin(String, Pos),
    InvalidSize(PinOrArray, isize),
    InvalidSizeType(PinOrArray, String),
    InvalidIndex(isize),
    InvalidIndexType(String),
    InvalidConstIdentifier(String),
    DeclConstType(String),
    UnknownEnumVariant(String, String),
}

impl AnalyserError {
    fn invalid_unary_op(op: &str, rhs_type: &str) -> String {
        format!("cannot perform operation '{op}' on type '{rhs_type}'")
    }

    fn invalid_binary_op(op: &str, lhs_type: &str, rhs_type: &str) -> String {
        format!("cannot perform operation '{op}' between types '{lhs_type}' and '{rhs_type}'")
    }
}

// impl std::fmt::Display for AnalyserError {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         match self {
//             Self::DuplicateArgument(circ_name, arg_name, first_defined) => write!(
//                 f,
//                 "argument '{arg_name}' already defined for circuit '{circ_name}' (first definition at line {}, col {})",
//                 first_defined.line(),
//                 first_defined.col(),
//             ),
//             Self::DuplicateSymbol(symbol, first_defined) => write!(
//                 f,
//                 "symbol '{symbol}' already defined (first definition at line {}, col {})",
//                 first_defined.line(),
//                 first_defined.col(),
//             ),
//             Self::DuplicateVariant(enum_name, variant_name, first_defined) => write!(
//                 f,
//                 "variant '{variant_name}' already defined in enum '{enum_name}' (first definition at line {}, col {})",
//                 first_defined.line(),
//                 first_defined.col(),
//             ),
//             Self::UnknownType(name) => write!(f, "unknown type: '{name}'"),
//             Self::UnknownIdentifier(name) => write!(f, "unknown identifier: '{name}'"),
//             Self::InvalidConstantType(type_name) => write!(f, "cannot create a constant of type {type_name}, you may want to use decl inside a circuit instead"),
//             Self::InvalidBinaryOp(op, lhs_type, rhs_type) => write!(
//                 f,
//                 "cannot perform operation '{op}' on types '{lhs_type}' and '{rhs_type}'",
//             ),
//             Self::InvalidUnaryOp(op, operand_type) => write!(
//                 f,
//                 "cannot perform operation '{op}' on type '{operand_type}'",
//             ),
//             Self::MismatchedTypes(expected, got) => {
//                 write!(f, "mismatched types, expected: {expected}, got: {got}")
//             }
//             Self::ArgDefaultTypeMismatch(expected, got, arg_type_defined_at) => write!(
//                 f,
//                 "default value of type '{got}' does not match argument type '{expected}' (argument type defined at line {}, col {})",
//                 arg_type_defined_at.line(),
//                 arg_type_defined_at.col(),
//             ),
//             Self::AssertionFailed(Some(message)) => write!(f, "assertion failed: {message}"),
//             Self::AssertionFailed(None) => write!(f, "assertion failed"),
//             Self::PinWithTypeArgs => write!(f, "pins cannot have type arguments"),
//             Self::UnknownCirc(name) => write!(f, "unknown circuit: '{name}'"),
//             Self::TooManyArgs(expected, got) => write!(f, "too many arguments (expected {expected}, got {got})"),
//             Self::PositionalArgAfterNamedArg(pos) => write!(
//                 f,
//                 "positional argument after named argument (last named argument specified at line {}, col {})",
//                 pos.line(),
//                 pos.col()
//             ),
//             Self::UnknownArgument(name) => write!(f, "unknown argument: {name}"),
//             Self::MissingRequiredArg(name) => write!(f, "required argument {name} not specified"),
//             Self::DuplicatePin(name, first_defined) => write!(
//                 f,
//                 "pin {name} already defined (first definition at line {}, col {})",
//                 first_defined.line(),
//                 first_defined.col(),
//             ),
//             Self::InvalidSize(PinOrArray::Pin, value) => write!(f, "invalid pin width: {value}"),
//             Self::InvalidSize(PinOrArray::Array, value) => write!(f, "invalid array length: {value}"),
//             Self::InvalidSizeType(PinOrArray::Pin, name) => write!(f, "pin width must be an integer (got {name})"),
//             Self::InvalidSizeType(PinOrArray::Array, name) => write!(f, "array length must be an integer (got {name})"),
//             Self::InvalidIndex(value) => write!(f, "invalid index: {value}"),
//             Self::InvalidIndexType(name) => write!(f, "indices must be integers (got {name})"),
//             Self::InvalidConstIdentifier(name) => write!(f, "{name} is not known at compile time"),
//             Self::DeclConstType(name) => write!(f, "cannot use decl with type {name}, use const instead"),
//             Self::UnknownEnumVariant(enum_name, variant) => write!(f, "no variant '{variant}' on enum '{enum_name}'")
//         }
//     }
// }

pub enum BuildConst {
    Bool(bool),
    Int(isize),
}

impl Into<ConstValue> for BuildConst {
    fn into(self) -> ConstValue {
        match self {
            Self::Bool(v) => ConstValue::Bool(v),
            Self::Int(v) => ConstValue::Int(v),
        }
    }
}

#[derive(Debug)]
pub struct AnalyserResult {
    diagnostics: Diagnostics,
}

struct AnalyserContext {
    scope: Scope,
    module_id: ModuleId,
}

#[derive(Debug)]
pub struct Analyser {
    loader: Loader,
    interner: Interner<String>,
    global_scope: Rc<Scope>,
    diagnostics: Diagnostics,
    module_exports: HashMap<ModuleId, Rc<Scope>>,
    import_stack: Vec<ModuleId>,
    circs: OrderedMap<CircSignature, Circ>,
}

impl Analyser {
    pub fn new(build_consts: HashMap<String, BuildConst>, loader: Loader) -> Self {
        let mut interner = Interner::new();
        let global_scope = Rc::new(Self::get_global_scope(&mut interner, build_consts));

        Self {
            loader,
            interner,
            global_scope,
            diagnostics: Diagnostics::new(),
            module_exports: HashMap::new(),
            import_stack: Vec::new(),
            circs: OrderedMap::new(),
        }
    }

    pub fn analyse(mut self) -> Result<AnalyserResult, Diagnostics> {
        self.process_module(Loader::ROOT_MODULE);

        if self.diagnostics.has_errors() {
            return Err(self.diagnostics);
        };

        self.type_check_circs()?;

        if self.diagnostics.has_errors() {
            Err(self.diagnostics)
        } else {
            Ok(AnalyserResult {
                diagnostics: self.diagnostics,
            })
        }
    }

    fn import_module(
        &mut self,
        name: &str,
        relative_to: ModuleId,
    ) -> Result<ModuleId, Diagnostics> {
        let module_id = self.loader.load_module(&name, relative_to)?;

        if self.import_stack.contains(&module_id) {
            let chain: Vec<_> = self
                .import_stack
                .iter()
                .take_while(|&&id| id != module_id)
                .collect();

            todo!("circular import error")
        }

        self.process_module(module_id)?;

        Ok(module_id)
    }

    fn process_module(&mut self, module_id: ModuleId) -> Result<(), Diagnostics> {
        let ast = self.parse_module(module_id)?;

        let mut ctx = AnalyserContext {
            scope: self.global_scope.child(),
            module_id,
        };

        self.import_stack.push(module_id);
        self.evaluate_items(&mut ctx, ast.into_iter())?;
        self.import_stack.pop();

        self.module_exports.insert(module_id, Rc::new(ctx.scope));

        Ok(())
    }

    fn get_global_scope(
        interner: &mut Interner<String>,
        build_consts: HashMap<String, BuildConst>,
    ) -> Scope {
        let builtin_scope = Scope::builtin(interner);
        let mut global_scope = builtin_scope.child();

        for (name, value) in build_consts {
            let name = interner.intern(name);

            let value: ConstValue = value.into();

            global_scope.add_const(ConstSymbol {
                name,
                r#type: value.get_type(),
                value,
                pos: Pos::Builtin,
            });
        }

        global_scope
    }

    fn parse_module(&mut self, module_id: ModuleId) -> Result<AST, Diagnostics> {
        let module = self.loader.get_module_mut(module_id).unwrap();

        let content = module.read()?;

        let tokeniser = Tokeniser::new(module.id, &content, &mut self.interner);
        let tokeniser_result = tokeniser.tokenise()?;

        let parser = Parser::new(module.id, tokeniser_result.tokens());
        let parser_result = parser.parse()?;

        self.diagnostics.append(tokeniser_result.diagnostics);
        self.diagnostics.append(parser_result.diagnostics);

        Ok(parser_result.ast)
    }

    // fn parse_module(&mut self, module: &mut Module) -> Result<AST, Diagnostics> {
    //     let content = module.read()?;

    //     let tokeniser = Tokeniser::new(module.id, &content, &mut self.interner);
    //     let tokeniser_result = tokeniser.tokenise()?;

    //     let parser = Parser::new(module.id, tokeniser_result.tokens());
    //     let parser_result = parser.parse()?;

    //     self.diagnostics.append(tokeniser_result.diagnostics);
    //     self.diagnostics.append(parser_result.diagnostics);

    //     Ok(parser_result.ast)
    // }

    fn resolve_ident(&self, ident_id: IdentId) -> &str {
        self.interner
            .get(ident_id)
            .unwrap_or_else(|| unreachable!("identifier {ident_id} not found in interner"))
    }

    fn evaluate_items(
        &mut self,
        ctx: &mut AnalyserContext,
        items: impl Iterator<Item = Node>,
    ) -> Result<(), Diagnostics> {
        for node in items {
            match node.node_type() {
                NodeType::Assert(_) => self.evaluate_assert(ctx, &node)?,
                NodeType::Circ { .. } => self.evaluate_circ(ctx, node)?,
                NodeType::ConstDecl { .. } => self.evaluate_constdecl(ctx, &node)?,
                NodeType::Enum { .. } => self.evaluate_enum(ctx, node)?,
                NodeType::If { .. } => self.evaluate_global_if(ctx, node)?,
                NodeType::Import(_) => self.evaluate_import(ctx, node)?,
                _ => unreachable!("parser bug - node not allowed at top level: {node:#?}"),
            }
        }

        Ok(())
    }

    fn evaluate_assert(&mut self, ctx: &AnalyserContext, node: &Node) -> Result<(), Diagnostics> {
        let pos = node.pos();

        let condition = extract!(node.node_type(), NodeType::Assert[condition]);
        let condition_pos = condition.pos();
        let result = self.evaluate_constexpr(ctx, condition)?;

        if result.is_unknown() {
            return Ok(());
        }

        let ConstValue::Bool(result) = result else {
            return Err(diagnostic!(
                Error,
                (if let ConstType::Enum { enum_name, .. } = result.get_type() {
                    format!(
                        "expected boolean value in assertion condition, got enum '{}'",
                        self.resolve_ident(enum_name)
                    )
                } else {
                    format!(
                        "expected boolean value in assertion condition, got '{}'",
                        result.get_type().type_name()
                    )
                }),
                pos = condition_pos,
            )
            .into());
        };

        if !result {
            return Err(diagnostic!(Error, "assertion failed!", pos = pos).into());
        }

        Ok(())
    }

    fn evaluate_circ(&mut self, ctx: &mut AnalyserContext, node: Node) -> Result<(), Diagnostics> {
        let (nt, pos) = node.parts();

        let (circ_name, circ_args, statements) = extract!(
            nt,
            NodeType::Circ {
                name,
                args,
                children
            }
        );

        let name = extract_identifier(&circ_name);

        let already_defined = self.check_symbol_exists(ctx, name, pos);

        let mut args: OrderedMap<IdentId, CircArg> = OrderedMap::new();

        for arg in circ_args {
            let (arg, arg_pos) = arg.parts();
            let (arg_type, arg_name, arg_default) = extract!(
                arg,
                NodeType::CircArg {
                    r#type,
                    name,
                    default
                }
            );

            let resolved_type = self.get_const_type(ctx, &arg_type);
            let arg_name_id = extract_identifier(&arg_name);

            if let Some(existing) = args.get(&arg_name_id) {
                self.diagnostics.push(diagnostic!(
                    Error,
                    format!(
                        "duplicate argument '{}' for circuit '{}'",
                        self.resolve_ident(arg_name_id),
                        self.resolve_ident(name),
                    ),
                    pos = arg_name.pos(),
                    note = diagnostic!(
                        Info,
                        format!("'{}' first defined here", self.resolve_ident(arg_name_id)),
                        pos = existing.pos
                    )
                ));
            };

            let default = match arg_default {
                Some(node) => Some(catch_errors!(
                    self,
                    self.evaluate_constexpr(ctx, &node),
                    ConstValue::Unknown
                )),
                None => None,
            };

            if let Some(default) = default {
                if default.get_type() != resolved_type {
                    self.diagnostics.push(
                        self.const_type_mismatch_error(resolved_type, default.get_type())
                            .with_pos(pos)
                            .add_note(diagnostic!(
                                Info,
                                format!(
                                    "type declared here as '{}'",
                                    (if let ConstType::Enum { enum_name, .. } = resolved_type {
                                        self.resolve_ident(enum_name)
                                    } else {
                                        resolved_type.type_name()
                                    })
                                ),
                                pos = arg_type.pos()
                            )),
                    );
                }
            }

            let _ = args.insert(
                arg_name_id,
                CircArg {
                    r#type: resolved_type,
                    name: arg_name_id,
                    default,
                    pos: arg_pos,
                },
            );
        }

        if already_defined {
            return Ok(());
        }

        ctx.scope.add_circ(CircSymbol {
            module_id: ctx.module_id,
            name,
            args,
            statements,
            pos,
        });

        Ok(())
    }

    fn evaluate_constdecl(
        &mut self,
        ctx: &mut AnalyserContext,
        node: &Node,
    ) -> Result<(), Diagnostics> {
        let pos = node.pos();

        let (r#type, name_node, expr) =
            extract!(node.node_type(), NodeType::ConstDecl { r#type, name, expr });

        let const_type = self.get_const_type(ctx, &r#type);

        let name = extract_identifier(&name_node);
        let already_defined = self.check_symbol_exists(ctx, name, name_node.pos());

        let mut value = catch_errors!(
            self,
            self.evaluate_constexpr(ctx, &expr),
            ConstValue::Unknown
        );

        if value.get_type().is_unknown() || const_type.is_unknown() {
            return Ok(());
        }

        if const_type != value.get_type() {
            self.diagnostics.push(
                self.const_type_mismatch_error(const_type, value.get_type())
                    .with_pos(pos)
                    .add_note(diagnostic!(
                        Info,
                        format!(
                            "type declared here as '{}'",
                            (if let ConstType::Enum { enum_name, .. } = const_type {
                                self.resolve_ident(enum_name)
                            } else {
                                const_type.type_name()
                            })
                        ),
                        pos = r#type.pos()
                    )),
            );
            value = ConstValue::Unknown;
        }

        if already_defined {
            return Ok(());
        }

        ctx.scope.add_const(ConstSymbol {
            name,
            r#type: const_type,
            value,
            pos,
        });

        Ok(())
    }

    fn get_const_type(&mut self, ctx: &AnalyserContext, path_node: &Node) -> ConstType {
        let type_path = extract!(path_node.node_type(), NodeType::Path[path]);

        let type_symbol = catch_error!(
            self,
            self.resolve_path(&ctx.scope, type_path).map(Some),
            None
        );

        if let Some(type_symbol) = type_symbol {
            match type_symbol {
                ScopeSymbol::PrimitiveType(t) => t.clone(),
                ScopeSymbol::EnumType(t) => ConstType::Enum {
                    module_id: t.module_id,
                    enum_name: t.name,
                },
                ScopeSymbol::Circ(circ) => {
                    self.diagnostics.push(diagnostic!(
                        Error,
                        format!(
                            "cannot use circuit type '{}' in a const context",
                            self.resolve_ident(circ.name)
                        ),
                        pos = path_node.pos(),
                        note = diagnostic!(
                            Info,
                            format!(
                                "'{}' defined here, as a circuit",
                                self.resolve_ident(circ.name)
                            ),
                            pos = circ.pos,
                        )
                    ));
                    ConstType::Unknown
                }
                _ => {
                    self.diagnostics.push(diagnostic!(
                        Error,
                        format!(
                            "cannot use value '{}' as a type",
                            self.resolve_ident(type_symbol.name())
                        ),
                        pos = path_node.pos(),
                    ));
                    ConstType::Unknown
                }
            }
        } else {
            ConstType::Unknown
        }
    }

    fn evaluate_enum(&mut self, ctx: &mut AnalyserContext, node: Node) -> Result<(), Diagnostics> {
        let (nt, pos) = node.parts();
        let (name_node, variants) = extract!(nt, NodeType::Enum { name, variants });

        let name = extract_identifier(&name_node);
        let already_defined = self.check_symbol_exists(ctx, name, name_node.pos());

        let mut variant_map = HashMap::new();

        for variant_node in variants {
            let variant_name = extract_identifier(&variant_node);
            if let Some(pos) = variant_map.get(&variant_name) {
                self.diagnostics.push(diagnostic!(
                    Error,
                    format!(
                        "duplicate variant '{}' in enum '{}'",
                        self.resolve_ident(variant_name),
                        self.resolve_ident(name)
                    ),
                    pos = variant_node.pos(),
                    note = diagnostic!(
                        Info,
                        format!("'{}' first defined here", self.resolve_ident(variant_name)),
                        pos = *pos
                    )
                ));
                continue;
            }

            variant_map.insert(variant_name, variant_node.pos());
        }

        if already_defined {
            return Ok(());
        }

        ctx.scope.add_enum(EnumSymbol {
            module_id: name_node.pos().module_id().unwrap(),
            name,
            variants: variant_map,
            pos,
        });

        Ok(())
    }

    fn evaluate_global_if(
        &mut self,
        ctx: &mut AnalyserContext,
        node: Node,
    ) -> Result<(), Diagnostics> {
        let (nt, pos) = node.parts();

        let (condition, then, r#else) = extract!(
            nt,
            NodeType::If {
                condition,
                then,
                r#else
            }
        );

        let condition_pos = condition.pos();

        let result = catch_errors!(
            self,
            self.evaluate_constexpr(ctx, &condition),
            ConstValue::Unknown
        );

        if result.is_unknown() {
            // TODO: speculative evaluation?
            return Ok(());
        }

        let ConstValue::Bool(result) = result else {
            self.diagnostics.push(diagnostic!(
                Error,
                (if let ConstType::Enum { enum_name, .. } = result.get_type() {
                    format!(
                        "if conditions must evaluate to a boolean value, got enum '{}'",
                        self.resolve_ident(enum_name)
                    )
                } else {
                    format!(
                        "if conditions must evaluate to a boolean value, got enum '{}'",
                        result.get_type().type_name()
                    )
                }),
                pos = condition_pos,
                note = diagnostic!(
                    Warning,
                    "none of the branches of this if have been evaluated",
                )
            ));

            return Ok(());
        };

        if result {
            self.evaluate_items(ctx, then.into_iter())
        } else {
            self.evaluate_items(ctx, r#else.into_iter())
        }
    }

    fn evaluate_import(
        &mut self,
        ctx: &mut AnalyserContext,
        node: Node,
    ) -> Result<(), Diagnostics> {
        let (nt, pos) = node.parts();

        let name_node = extract!(nt, NodeType::Import[name]);
        let name_id = extract_identifier(&name_node);
        let name = self.resolve_ident(name_id).to_string();

        let already_defined = self.check_symbol_exists(ctx, name_id, pos);

        if !already_defined {
            self.import_module(&name, ctx.module_id)?;
        }

        Ok(())
    }

    fn evaluate_constexpr(
        &mut self,
        ctx: &AnalyserContext,
        expr: &Node,
    ) -> Result<ConstValue, Diagnostics> {
        let pos = expr.pos();

        let (lhs, op, rhs) = match expr.node_type() {
            NodeType::ConstExpr { lhs, op, rhs } => (lhs, op, rhs),
            NodeType::Integer(value) => return Ok(ConstValue::Int(*value)),
            NodeType::Path(path) => {
                let symbol = match self.resolve_path(&ctx.scope, &path) {
                    Ok(symbol) => symbol,
                    Err(d) => {
                        self.diagnostics.push(d);
                        return Ok(ConstValue::Unknown);
                    }
                };

                match symbol {
                    ScopeSymbol::Const(ConstSymbol { value, .. }) => {
                        return Ok(value);
                    }
                    ScopeSymbol::EnumValue {
                        module_id,
                        enum_name,
                        variant,
                    } => {
                        return Ok(ConstValue::Enum {
                            module_id,
                            enum_name,
                            variant,
                        })
                    }
                    _ => todo!("non const symbol in path"),
                }
            }
            _ => unreachable!("parser bug"),
        };

        let lhs = lhs.as_ref().map(|node| {
            catch_errors!(
                self,
                self.evaluate_constexpr(ctx, node),
                ConstValue::Unknown
            )
        });

        let rhs = catch_errors!(
            self,
            self.evaluate_constexpr(ctx, &rhs),
            ConstValue::Unknown
        );

        let result = match (lhs, rhs) {
            (Some(lhs @ ConstValue::Int(x)), rhs @ ConstValue::Int(y)) => match op {
                ConstExprOpType::Add => ConstValue::Int(x + y),
                ConstExprOpType::Sub => ConstValue::Int(x - y),
                ConstExprOpType::Mul => ConstValue::Int(x * y),
                ConstExprOpType::Div => ConstValue::Int(x / y),
                ConstExprOpType::Mod => ConstValue::Int(x % y),
                ConstExprOpType::BitAnd => ConstValue::Int(x & y),
                ConstExprOpType::BitOr => ConstValue::Int(x | y),
                ConstExprOpType::BitXor => ConstValue::Int(x ^ y),
                ConstExprOpType::Shl => ConstValue::Int(x << y),
                ConstExprOpType::Shr => ConstValue::Int(x >> y),
                ConstExprOpType::Eq => ConstValue::Bool(x == y),
                ConstExprOpType::Neq => ConstValue::Bool(x != y),
                ConstExprOpType::Lt => ConstValue::Bool(x < y),
                ConstExprOpType::Lte => ConstValue::Bool(x <= y),
                ConstExprOpType::Gt => ConstValue::Bool(x > y),
                ConstExprOpType::Gte => ConstValue::Bool(x >= y),
                ConstExprOpType::Range => ConstValue::Range(x, y),
                _ => {
                    self.diagnostics.push(diagnostic!(
                        Error,
                        AnalyserError::invalid_binary_op(
                            op.as_string(),
                            lhs.get_type().type_name(),
                            rhs.get_type().type_name()
                        ),
                        pos = pos,
                    ));
                    ConstValue::Unknown
                }
            },
            (None, rhs @ ConstValue::Int(x)) => match op {
                ConstExprOpType::UnaryMinus => ConstValue::Int(-x),
                ConstExprOpType::BitNot => ConstValue::Int(!x),
                _ => {
                    self.diagnostics.push(diagnostic!(
                        Error,
                        AnalyserError::invalid_unary_op(op.as_string(), rhs.get_type().type_name()),
                        pos = pos,
                    ));
                    ConstValue::Unknown
                }
            },
            (Some(lhs @ ConstValue::Bool(x)), rhs @ ConstValue::Bool(y)) => match op {
                ConstExprOpType::Eq => ConstValue::Bool(x == y),
                ConstExprOpType::Neq => ConstValue::Bool(x != y),
                ConstExprOpType::And => ConstValue::Bool(x && y),
                ConstExprOpType::Or => ConstValue::Bool(x || y),
                ConstExprOpType::Xor => ConstValue::Bool(x != y),
                _ => {
                    self.diagnostics.push(diagnostic!(
                        Error,
                        AnalyserError::invalid_binary_op(
                            op.as_string(),
                            lhs.get_type().type_name(),
                            rhs.get_type().type_name()
                        ),
                        pos = pos,
                    ));
                    ConstValue::Unknown
                }
            },
            (None, rhs @ ConstValue::Bool(x)) => match op {
                ConstExprOpType::Not => ConstValue::Bool(!x),
                _ => {
                    self.diagnostics.push(diagnostic!(
                        Error,
                        AnalyserError::invalid_unary_op(op.as_string(), rhs.get_type().type_name()),
                        pos = pos,
                    ));
                    ConstValue::Unknown
                }
            },
            (
                Some(
                    lhs @ ConstValue::Enum {
                        module_id: lhs_module,
                        enum_name: lhs_enum,
                        variant: x,
                    },
                ),
                rhs @ ConstValue::Enum {
                    module_id: rhs_module,
                    enum_name: rhs_enum,
                    variant: y,
                },
            ) if lhs_module == rhs_module && lhs_enum == rhs_enum => match op {
                ConstExprOpType::Eq => ConstValue::Bool(x == y),
                ConstExprOpType::Neq => ConstValue::Bool(x != y),
                _ => {
                    self.diagnostics.push(diagnostic!(
                        Error,
                        AnalyserError::invalid_binary_op(
                            op.as_string(),
                            lhs.get_type().type_name(),
                            rhs.get_type().type_name()
                        ),
                        pos = pos,
                    ));
                    ConstValue::Unknown
                }
            },
            (
                Some(
                    lhs @ ConstValue::Enum {
                        module_id: lhs_module,
                        enum_name: lhs_enum,
                        variant: x,
                    },
                ),
                rhs @ ConstValue::Enum {
                    module_id: rhs_module,
                    enum_name: rhs_enum,
                    variant: y,
                },
            ) => {
                todo!("different enums cannot be compared");
                ConstValue::Unknown
            }
            (Some(ConstValue::Unknown), _) | (_, ConstValue::Unknown) => ConstValue::Unknown,
            (Some(lhs), rhs) => {
                self.diagnostics.push(diagnostic!(
                    Error,
                    AnalyserError::invalid_binary_op(
                        op.as_string(),
                        lhs.get_type().type_name(),
                        rhs.get_type().type_name()
                    ),
                    pos = pos,
                ));
                ConstValue::Unknown
            }
            (None, rhs) => {
                self.diagnostics.push(diagnostic!(
                    Error,
                    AnalyserError::invalid_unary_op(op.as_string(), rhs.get_type().type_name()),
                    pos = pos,
                ));
                ConstValue::Unknown
            }
        };

        Ok(result)
    }

    fn check_symbol_exists(
        &mut self,
        ctx: &AnalyserContext,
        name: IdentId,
        defined_at: Pos,
    ) -> bool {
        let Some(sym) = ctx.scope.get_symbol(name) else {
            return false;
        };

        if let Pos::Builtin = sym.pos() {
            self.diagnostics.push(diagnostic!(
                Error,
                format!(
                    "redefinition of builtin symbol '{}'",
                    self.resolve_ident(name)
                ),
                pos = defined_at,
            ))
        } else {
            self.diagnostics.push(diagnostic!(
                Error,
                format!("symbol '{}' already defined", self.resolve_ident(name)),
                pos = defined_at,
                note = diagnostic!(
                    Info,
                    format!("'{}' first defined here", self.resolve_ident(name)),
                    pos = sym.pos()
                ),
            ))
        }

        true
    }

    fn const_type_mismatch_error(&self, expected: ConstType, got: ConstType) -> Diagnostic {
        assert_ne!(expected, got);

        match (expected, got) {
            (
                ConstType::Enum {
                    enum_name: expected_name,
                    ..
                },
                ConstType::Enum {
                    enum_name: got_name,
                    ..
                },
            ) => diagnostic!(
                Error,
                format!(
                    "mismatched types: expected enum '{}', got enum '{}'",
                    self.resolve_ident(expected_name),
                    self.resolve_ident(got_name)
                ),
            ),
            (
                ConstType::Enum {
                    enum_name: expected_name,
                    ..
                },
                got,
            ) => diagnostic!(
                Error,
                format!(
                    "mismatched types: expected enum '{}', got '{}'",
                    self.resolve_ident(expected_name),
                    got.type_name(),
                ),
            ),
            (
                expected,
                ConstType::Enum {
                    enum_name: got_name,
                    ..
                },
            ) => diagnostic!(
                Error,
                format!(
                    "mismatched types: expected '{}', got enum '{}'",
                    expected.type_name(),
                    self.resolve_ident(got_name),
                ),
            ),
            (expected, got) => diagnostic!(
                Error,
                format!(
                    "mismatched types: expected '{}', got '{}'",
                    expected.type_name(),
                    got.type_name(),
                ),
            ),
        }
    }

    fn resolve_path(&self, scope: &Scope, path: &[Node]) -> Result<ScopeSymbol, Diagnostic> {
        let mut ident_path = Vec::new();
        let symbol_kind = self._resolve_path_inner(scope, path, &mut ident_path)?;
        // Ok(symbol_kind.tag_path(ident_path.into()))
        Ok(symbol_kind)
    }

    fn _resolve_path_inner(
        &self,
        scope: &Scope,
        path: &[Node],
        ident_path: &mut Vec<IdentId>,
    ) -> Result<ScopeSymbol, Diagnostic> {
        assert!(!path.is_empty());
        let ident_id = extract_identifier(&path[0]);
        let Some(symbol) = scope.get_symbol(ident_id) else {
            return Err(diagnostic!(
                Error,
                format!(
                    "could not resolve identifier '{}'",
                    self.resolve_ident(ident_id)
                ),
                pos = path[0].pos(),
            ));
        };

        ident_path.push(ident_id);

        if path.len() == 1 {
            return Ok(symbol);
        }

        match symbol {
            ScopeSymbol::EnumType(r#enum) => {
                let variant_id = extract_identifier(&path[1]);

                let Some(_) = r#enum.variants.get(&variant_id) else {
                    return Err(diagnostic!(
                        Error,
                        format!(
                            "unknown variant '{}' in enum '{}'",
                            self.resolve_ident(variant_id),
                            self.resolve_ident(ident_id)
                        ),
                        pos = path[1].pos(),
                    ));
                };

                if path.len() > 2 {
                    let extra = extract_identifier(&path[2]);
                    return Err(diagnostic!(
                        Error,
                        format!(
                            "cannot access member '{}' on enum variant '{}:{}'",
                            self.resolve_ident(extra),
                            self.resolve_ident(ident_id),
                            self.resolve_ident(variant_id)
                        ),
                        pos = path[2].pos(),
                    ));
                }

                ident_path.push(variant_id);

                Ok(ScopeSymbol::EnumValue {
                    module_id: r#enum.module_id,
                    enum_name: ident_id,
                    variant: variant_id,
                })
            }
            ScopeSymbol::Module(module) => {
                let Some(scope) = self.module_exports.get(&module.module_id) else {
                    unreachable!("module not loaded yet it has an id: {}", module.module_id);
                };

                self._resolve_path_inner(scope, &path[1..], ident_path)
            }
            symbol => {
                let extra = extract_identifier(&path[1]);

                Err(diagnostic!(
                    Error,
                    format!(
                        "cannot access member '{}' on {} '{}'",
                        self.resolve_ident(extra),
                        symbol.get_type().error_msg_str(),
                        self.resolve_ident(ident_id)
                    ),
                    pos = path[2].pos(),
                ))
            }
        }
    }

    fn type_check_circs(&mut self) -> Result<(), Diagnostics> {
        let root_scope = self
            .module_exports
            .get(&Loader::ROOT_MODULE)
            .unwrap_or_else(|| unreachable!("no root module scope"));

        let main_symbol = self
            .interner
            .get_index("Main")
            .and_then(|id| root_scope.get_symbol(id));

        let main = match main_symbol {
            Some(ScopeSymbol::Circ(main)) => main.clone(),
            Some(symbol) => {
                return Err(diagnostic!(
                    Error,
                    "symbol 'Main' is not a circuit in the root module",
                    pos = symbol.pos(),
                )
                .into())
            }
            None => {
                return Err(diagnostic!(
                    Error,
                    "no symbol 'Main' present in root module",
                    pos = Pos::Module(Loader::ROOT_MODULE)
                )
                .into())
            }
        };

        if main.args.len() > 0 {
            self.diagnostics.push(diagnostic!(
                Error,
                "circuit 'Main' cannot have arguments",
                pos = main.pos,
            ));
        }

        let main_sig = CircSignature {
            module_id: main.module_id,
            name: main.name,
            args: Rc::from([]),
        };

        self.type_check_circ(main_sig)?;

        todo!();
    }

    fn type_check_circ(&mut self, signature: CircSignature) -> Result<CircId, Diagnostics> {
        let mut scope = self
            .module_exports
            .get(&signature.module_id)
            .unwrap_or_else(|| unreachable!("invalid module_id in signature: {signature:#?}"))
            .child();

        let Some(ScopeSymbol::Circ(circ)) = scope.get_symbol(signature.name) else {
            unreachable!("invalid circ id in signature {signature:#?}")
        };

        for (index, value) in signature.args.iter().enumerate() {
            let Some(arg) = circ.args.get_by_index(index) else {
                unreachable!("circ arg index oob")
            };

            assert_eq!(arg.r#type, value.get_type());

            scope.add_const(ConstSymbol {
                name: arg.name,
                r#type: arg.r#type,
                value: *value,
                pos: arg.pos,
            });
        }

        let mut ctx = AnalyserContext {
            scope,
            module_id: circ.module_id,
        };

        let mut circ_builder = CircBuilder::default();

        self.evaluate_circ_items(&mut ctx, &mut circ_builder, circ.statements.iter())?;

        match self.circs.insert(signature, circ_builder.build()) {
            Ok(circ_id) => Ok(circ_id),
            Err(c) => unreachable!("circ already analysed: {c:#?}"),
        }
    }

    fn evaluate_circ_items<'a>(
        &mut self,
        ctx: &mut AnalyserContext,
        circ: &mut CircBuilder,
        items: impl Iterator<Item = &'a Node>,
    ) -> Result<(), Diagnostics> {
        for node in items {
            match node.node_type() {
                NodeType::Statement(node) => match node.node_type() {
                    NodeType::Assert(_) => self.evaluate_assert(ctx, node)?,
                    NodeType::ConstDecl { .. } => self.evaluate_constdecl(ctx, node)?,
                    NodeType::If { .. } => self.evaluate_circ_if(ctx, circ, node)?,
                    NodeType::PinDecls { .. } => self.evaluate_pin_decls(ctx, circ, node)?,
                    NodeType::With(_) => self.evaluate_with(ctx, circ, node)?,
                    NodeType::Connection { .. } => self.evaluate_connection(ctx, circ, node)?,
                    _ => unreachable!("parser bug - invalid statement node {node:#?}"),
                },
                _ => unreachable!("parser bug - node not allowed in circ item: {node:#?}"),
            }
        }

        Ok(())
    }

    fn evaluate_circ_if(
        &mut self,
        ctx: &mut AnalyserContext,
        circ: &mut CircBuilder,
        node: &Node,
    ) -> Result<(), Diagnostics> {
        let (condition, then, r#else) = extract!(
            node.node_type(),
            NodeType::If {
                condition,
                then,
                r#else
            }
        );

        let condition_pos = condition.pos();

        let result = catch_errors!(
            self,
            self.evaluate_constexpr(ctx, &condition),
            ConstValue::Unknown
        );

        if result.is_unknown() {
            // TODO: speculative evaluation?
            return Ok(());
        }

        let ConstValue::Bool(result) = result else {
            self.diagnostics.push(diagnostic!(
                Error,
                (if let ConstType::Enum { enum_name, .. } = result.get_type() {
                    format!(
                        "if conditions must evaluate to a boolean value, got enum '{}'",
                        self.resolve_ident(enum_name)
                    )
                } else {
                    format!(
                        "if conditions must evaluate to a boolean value, got enum '{}'",
                        result.get_type().type_name()
                    )
                }),
                pos = condition_pos,
                note = diagnostic!(
                    Warning,
                    "none of the branches of this if have been evaluated",
                )
            ));

            return Ok(());
        };

        if result {
            self.evaluate_circ_items(ctx, circ, then.iter())
        } else {
            self.evaluate_circ_items(ctx, circ, r#else.iter())
        }
    }

    fn evaluate_with(
        &mut self,
        ctx: &mut AnalyserContext,
        circ: &mut CircBuilder,
        node: &Node,
    ) -> Result<(), Diagnostics> {
        let decls = extract!(node.node_type(), NodeType::With[decls]);

        for decl in decls {
            self.evaluate_decls(ctx, circ, decl)?;
        }

        Ok(())
    }

    fn evaluate_decls(
        &mut self,
        ctx: &mut AnalyserContext,
        circ: &mut CircBuilder,
        node: &Node,
    ) -> Result<(), Diagnostics> {
        let (r#type, decls) = extract!(node.node_type(), NodeType::Decls { r#type, decls });

        let Some(sig) = self.evaluate_type(ctx, r#type)? else {
            todo!("add all decls to scope with unknown type")
        };

        let circ_id = if let Some(circ_id) = self.circs.get_index(&sig) {
            circ_id
        } else {
            self.type_check_circ(sig)?
        };

        for decl in decls {
            let (name_node, count) = extract!(decl.node_type(), NodeType::Decl { name, count });
            let name = extract_identifier(name_node);

            let already_exists = self.check_symbol_exists(ctx, name, name_node.pos());

            let width = count.as_ref().map(|count| {
                catch_errors!(
                    self,
                    self.evaluate_constexpr(&ctx, count),
                    ConstValue::Unknown
                )
            });

            let array_size = match width {
                Some(ConstValue::Int(s)) if s > 0 => Some(s as usize),
                Some(ConstValue::Int(s)) => {
                    self.diagnostics.push(diagnostic!(
                        Error,
                        "array size must be at least 1",
                        pos = decl.pos(),
                        note = diagnostic!(
                            Info,
                            format!("size expression evaluated to {s}"),
                            pos = count.as_ref().unwrap_or(name_node).pos(),
                        )
                    ));

                    Some(if s == 0 { 1 } else { -s } as usize)
                }
                Some(ConstValue::Unknown) => {
                    todo!("handle unknown width");
                }
                Some(_) => todo!("decl width must be an int"),
                None => None,
            };

            if already_exists {
                continue;
            };

            ctx.scope.add_local(LocalSymbol {
                name,
                r#type: if let Some(array_size) = array_size {
                    LocalType::Array(circ_id, array_size)
                } else {
                    LocalType::Circ(circ_id)
                },
                pos: decl.pos(),
            });

            circ.dependency(name, circ_id, array_size);
        }

        Ok(())
    }

    fn evaluate_type(
        &mut self,
        ctx: &AnalyserContext,
        node: &Node,
    ) -> Result<Option<CircSignature>, Diagnostics> {
        let (name, args) = extract!(node.node_type(), NodeType::Type { name, args });
        let name_path = extract!(name.node_type(), NodeType::Path[name_path]);

        let name_symbol = catch_error!(
            self,
            self.resolve_path(&ctx.scope, &name_path).map(Some),
            None
        );

        let Some(name_symbol) = name_symbol else {
            for arg in args {
                let value_expr = extract!(arg.node_type(), NodeType::TypeArg { value });

                // evaluate all args to catch any undefined consts or invalid expressions
                catch_errors!(self, self.evaluate_constexpr(ctx, value_expr), continue);
            }

            return Ok(None);
        };

        let ScopeSymbol::Circ(circ) = name_symbol else {
            todo!("add invalid type diagnostic");
            return Ok(None);
        };

        let args = self.get_resolved_circ_args(ctx, &circ, &args)?;

        return Ok(Some(CircSignature {
            module_id: circ.module_id,
            name: circ.name,
            args: args.into(),
        }));
    }

    fn get_resolved_circ_args(
        &mut self,
        ctx: &AnalyserContext,
        circ: &CircSymbol,
        type_args: &[Node],
    ) -> Result<Vec<ConstValue>, Diagnostics> {
        let mut args = HashMap::new();

        let mut extracted_args = type_args.iter().map(|arg_node| {
            (
                extract!(arg_node.node_type(), NodeType::TypeArg { name, value }),
                arg_node,
            )
        });

        let positional_args = extracted_args
            .by_ref()
            .take_while(|((name, _), _)| name.is_none())
            .map(|((_, value), _)| value)
            .enumerate();

        let mut args_count = 0;
        let mut first_unknown_arg = None;
        for (pos, value_node) in positional_args {
            args_count += 1;
            let value = catch_errors!(
                self,
                self.evaluate_constexpr(ctx, value_node),
                ConstValue::Unknown
            );

            let Some(arg_desc) = circ.args.get_by_index(pos) else {
                first_unknown_arg = Some(value_node);
                continue;
            };

            if arg_desc.r#type != value.get_type() {
                todo!("add diagnostic for invalid type")
            }

            args.insert(arg_desc.name, value);
        }

        if let Some(node) = first_unknown_arg {
            todo!("add diagnostic for too many args")
        }

        let mut did_positional_after_named = false;
        for ((name_node, value_node), node) in extracted_args {
            let name_node = match name_node {
                Some(name_node) => name_node,
                None => {
                    if !did_positional_after_named {
                        todo!("add diagnostic for positional after named");
                        did_positional_after_named = true;
                    }
                    continue;
                }
            };

            let value = catch_errors!(
                self,
                self.evaluate_constexpr(ctx, value_node),
                ConstValue::Unknown
            );

            let name = extract_identifier(name_node);

            let Some(arg_desc) = circ.args.get(&name) else {
                todo!("add error for unknown arg");
                continue;
            };

            if arg_desc.r#type != value.get_type() {
                todo!("add diagnostic for invalid type")
            }

            args.insert(arg_desc.name, value);
        }

        let mut args_ordered = Vec::new();

        for arg_desc in circ.args.values() {
            match args.remove(&arg_desc.name) {
                Some(value) => args_ordered.push(value),
                None => {
                    if let Some(default) = arg_desc.default {
                        args_ordered.push(default);
                    } else {
                        todo!("add error for missing required arg");
                    }
                }
            }
        }

        return Ok(args_ordered);
    }

    fn evaluate_pin_decls(
        &mut self,
        ctx: &mut AnalyserContext,
        circ: &mut CircBuilder,
        node: &Node,
    ) -> Result<(), Diagnostics> {
        let (direction, decls) =
            extract!(node.node_type(), NodeType::PinDecls { direction, decls });

        for decl in decls {
            let (name, count) = extract!(decl.node_type(), NodeType::Decl { name, count });
            let name_id = extract_identifier(name);

            let already_exists = self.check_symbol_exists(ctx, name_id, name.pos());

            let width = if let Some(expr) = count {
                catch_errors!(self, self.evaluate_constexpr(ctx, expr), ConstValue::Int(1))
            } else {
                ConstValue::Int(1)
            };

            let ConstValue::Int(width) = width else {
                todo!("pin width must be an int");
            };

            if width <= 0 {
                todo!("width must be at least 1");
            }

            if already_exists {
                continue;
            };

            ctx.scope.add_local(LocalSymbol {
                name: name_id,
                r#type: LocalType::Pin {
                    direction: *direction,
                    width: width as usize,
                },
                pos: decl.pos(),
            });

            circ.pin(
                name_id,
                Pin {
                    name: name_id,
                    direction: *direction,
                    width: width as usize,
                },
            );
        }

        Ok(())
    }

    fn evaluate_connection(
        &mut self,
        ctx: &mut AnalyserContext,
        circ: &mut CircBuilder,
        node: &Node,
    ) -> Result<(), Diagnostics> {
        let (mut lhs, direction, mut rhs) = extract!(
            node.node_type(),
            NodeType::Connection {
                lhs,
                direction,
                rhs
            }
        );

        let connection = match direction {
            ConnectionDirection::LeftToRight => circ.connection().unidirectional(),
            ConnectionDirection::RightToLeft => {
                std::mem::swap(&mut lhs, &mut rhs);
                circ.connection().unidirectional()
            }
            ConnectionDirection::Bidrectional => circ.connection().bidirectional(),
        };

        let lhs = self.evaluate_pinexpr(ctx, circ, lhs)?;

        let rhs = self.evaluate_pinexpr(ctx, circ, rhs)?;

        Ok(())
    }

    fn resolve_pinexpr_ident(
        &mut self,
        ctx: &mut AnalyserContext,
        circ: &mut CircBuilder,
        name: IdentId,
    ) -> Result<PinExprType, Diagnostics> {
        if let Some((pin_id, pin)) = circ.get_pin(name) {
            return Ok(PinExprType::Pin { pin_id, pin });
        }

        if let Some((circ_id, length)) = circ.get_dependency(name) {
            if let Some(len) = length {
                return Ok(PinExprType::DependencyArray { name, circ_id, len });
            } else {
                return Ok(PinExprType::Dependency {
                    name,
                    circ_id,
                    index: None,
                });
            }
        }

        todo!("not valid in pinexpr diagnostic")
    }

    fn evaluate_pinexpr_range(
        &mut self,
        ctx: &mut AnalyserContext,
        range: &Range,
    ) -> Result<PinExprType, Diagnostics> {
        match range {
            Range::Index(expr) => {
                let index = catch_errors!(
                    self,
                    self.evaluate_constexpr(ctx, &expr),
                    ConstValue::Unknown
                );

                if index.is_unknown() {
                    return Ok(PinExprType::Unknown);
                };

                match index {
                    ConstValue::Int(x) if x > 0 => Ok(PinExprType::Index(x as usize)),
                    ConstValue::Int(_) => todo!("invalid index diagnostic"),
                    _ => todo!("invalid index type diagnostic"),
                }
            }
            Range::Range(start, end, _) => {
                let start_val = start.as_ref().map(|start| {
                    catch_errors!(
                        self,
                        self.evaluate_constexpr(ctx, start),
                        ConstValue::Unknown
                    )
                });

                let start_val = match start_val {
                    None => Some(None),
                    Some(ConstValue::Int(x)) if x > 0 => Some(Some(x as usize)),
                    Some(ConstValue::Int(_)) => todo!("invalid index diagnostic"),
                    _ => todo!("invalid index type diagnostic"),
                };

                let end_val = end.as_ref().map(|end| {
                    catch_errors!(self, self.evaluate_constexpr(ctx, end), ConstValue::Unknown)
                });

                let end_val = match end_val {
                    None => Some(None),
                    Some(ConstValue::Int(x)) if x > 0 => Some(Some(x as usize)),
                    Some(ConstValue::Int(_)) => todo!("invalid index diagnostic"),
                    _ => todo!("invalid index type diagnostic"),
                };

                match (start_val, end_val) {
                    (Some(start), Some(end)) => Ok(PinExprType::Range(start, end)),
                    _ => Ok(PinExprType::Unknown),
                }
            }
        }
    }

    fn _pinexpr_bounds_check_index(&mut self, pos: Pos, lhs: PinExprType, index: usize) -> bool {
        match lhs {
            PinExprType::Pin { pin, .. } => {
                if index >= pin.width {
                    self.diagnostics.push(diagnostic!(
                        Error,
                        format!(
                            "index {} out of bounds for pin '{}' of width {}",
                            index,
                            self.resolve_ident(pin.name),
                            pin.width
                        ),
                        pos = pos,
                    ));
                    false
                } else {
                    true
                }
            }
            PinExprType::PinRange {
                pin, start, end, ..
            } => {
                let range_width = end.unwrap_or(pin.width) - start.unwrap_or(0);
                if index >= range_width {
                    self.diagnostics.push(diagnostic!(
                        Error,
                        format!(
                            "index {} out of bounds for bit slice [{}] of pin '{}'",
                            index,
                            ConnectionRange::Range(start, end),
                            self.resolve_ident(pin.name)
                        ),
                        pos = pos,
                    ));
                    false
                } else {
                    true
                }
            }
            PinExprType::DependencyArray { name, len, .. } => {
                if index >= len {
                    self.diagnostics.push(diagnostic!(
                        Error,
                        format!(
                            "index {} out of bounds for array '{}' of length {}",
                            index,
                            self.resolve_ident(name),
                            len
                        ),
                        pos = pos,
                    ));
                    false
                } else {
                    true
                }
            }
            PinExprType::DependencyPin {
                name,
                index: arr_index,
                pin,
                ..
            } => {
                if index >= pin.width {
                    self.diagnostics.push(diagnostic!(
                        Error,
                        if let Some(arr_index) = arr_index {
                            format!(
                                "index {} out of bounds for pin '{}[{arr_index}].{}' of width {}",
                                index,
                                self.resolve_ident(name),
                                self.resolve_ident(pin.name),
                                pin.width
                            )
                        } else {
                            format!(
                                "index {} out of bounds for pin '{}.{}' of width {}",
                                index,
                                self.resolve_ident(name),
                                self.resolve_ident(pin.name),
                                pin.width
                            )
                        },
                        pos = pos,
                    ));
                    false
                } else {
                    true
                }
            }
            PinExprType::DependencyPinRange {
                name,
                index: arr_index,
                pin,
                start,
                end,
                ..
            } => {
                let range_width = end.unwrap_or(pin.width) - start.unwrap_or(0);
                if index >= range_width {
                    self.diagnostics.push(diagnostic!(
                        Error,
                        if let Some(arr_index) = arr_index {
                            format!(
                                "index {} out of bounds for bit slice [{}] of pin '{}[{arr_index}].{}'",
                                index, ConnectionRange::Range(start, end), self.resolve_ident(name), self.resolve_ident(pin.name)
                            )
                        } else {
                            format!(
                                "index {} out of bounds for bit slice [{}] of pin '{}.{}'",
                                index, ConnectionRange::Range(start, end), self.resolve_ident(name), self.resolve_ident(pin.name)
                            )
                        },
                        pos = pos,
                    ));
                    false
                } else {
                    true
                }
            }
            _ => unreachable!(),
        }
    }

    fn _pinexpr_range_in_bounds(
        s_start: usize,
        s_end: usize,
        t_start: Option<usize>,
        t_end: Option<usize>,
    ) -> bool {
        // check if the target range (t_start, t_end) fits inside of the source range (s_start, s_end)
        let t_start = t_start.unwrap_or(0);
        let t_end = t_end.unwrap_or(s_end);
        if t_start >= s_start && t_end < s_end {
            true
        } else {
            false
        }
    }

    fn _pinexpr_bounds_check_range(
        &mut self,
        pos: Pos,
        lhs: PinExprType,
        start: Option<usize>,
        end: Option<usize>,
    ) -> bool {
        let range = ConnectionRange::Range(start, end);

        match lhs {
            PinExprType::Pin { pin, .. } => {
                let range_width = end.unwrap_or(pin.width) - start.unwrap_or(0) - 1;
                if range_width >= pin.width {
                    self.diagnostics.push(diagnostic!(
                        Error,
                        format!(
                            "range {} out of bounds for pin '{}' of width {}",
                            range,
                            self.resolve_ident(pin.name),
                            pin.width
                        ),
                        pos = pos,
                    ));
                    false
                } else {
                    true
                }
            }
            PinExprType::PinRange {
                pin,
                start: sl_start,
                end: sl_end,
                ..
            } => {
                if Self::_pinexpr_range_in_bounds(
                    sl_start.unwrap_or(0),
                    sl_end.unwrap_or(pin.width),
                    start,
                    end,
                ) {
                    self.diagnostics.push(diagnostic!(
                        Error,
                        format!(
                            "range {} out of bounds for bit slice [{}] of pin '{}'",
                            range,
                            ConnectionRange::Range(start, end),
                            self.resolve_ident(pin.name)
                        ),
                        pos = pos,
                    ));
                    false
                } else {
                    true
                }
            }
            PinExprType::DependencyPin {
                name,
                index: arr_index,
                pin,
                ..
            } => {
                let range_width = end.unwrap_or(pin.width) - start.unwrap_or(0) - 1;
                if range_width >= pin.width {
                    self.diagnostics.push(diagnostic!(
                        Error,
                        if let Some(arr_index) = arr_index {
                            format!(
                                "range {} out of bounds for pin '{}[{arr_index}].{}' of width {}",
                                range,
                                self.resolve_ident(name),
                                self.resolve_ident(pin.name),
                                pin.width
                            )
                        } else {
                            format!(
                                "range {} out of bounds for pin '{}.{}' of width {}",
                                range,
                                self.resolve_ident(name),
                                self.resolve_ident(pin.name),
                                pin.width
                            )
                        },
                        pos = pos,
                    ));
                    false
                } else {
                    true
                }
            }
            PinExprType::DependencyPinRange {
                name,
                index: arr_index,
                pin,
                start: sl_start,
                end: sl_end,
                ..
            } => {
                if Self::_pinexpr_range_in_bounds(
                    sl_start.unwrap_or(0),
                    sl_end.unwrap_or(pin.width),
                    start,
                    end,
                ) {
                    self.diagnostics.push(diagnostic!(
                        Error,
                        if let Some(arr_index) = arr_index {
                            format!(
                                "range {} out of bounds for bit slice [{}] of pin '{}[{arr_index}].{}'",
                                range, ConnectionRange::Range(start, end), self.resolve_ident(name), self.resolve_ident(pin.name)
                            )
                        } else {
                            format!(
                                "range {} out of bounds for bit slice [{}] of pin '{}.{}'",
                                range, ConnectionRange::Range(start, end), self.resolve_ident(name), self.resolve_ident(pin.name)
                            )
                        },
                        pos = pos,
                    ));
                    false
                } else {
                    true
                }
            }
            _ => unreachable!(),
        }
    }

    fn pinexpr_bounds_check(&mut self, pos: Pos, lhs: PinExprType, rhs: PinExprType) -> bool {
        match rhs {
            PinExprType::Index(index) => self._pinexpr_bounds_check_index(pos, lhs, index),
            PinExprType::Range(start, end) => {
                self._pinexpr_bounds_check_range(pos, lhs, start, end)
            }
            _ => unreachable!(),
        }
    }

    fn evaluate_pinexpr(
        &mut self,
        ctx: &mut AnalyserContext,
        circ: &mut CircBuilder,
        node: &Node,
    ) -> Result<PinExprType, Diagnostics> {
        let pos = node.pos();

        let (lhs, op, rhs) = match node.node_type() {
            NodeType::PinExpr { lhs, op, rhs } => (lhs, op, rhs),
            NodeType::Identifier(name) => return self.resolve_pinexpr_ident(ctx, circ, *name),
            NodeType::Range(range) => return self.evaluate_pinexpr_range(ctx, range),
            _ => unreachable!("parser bug"),
        };

        let lhs = lhs.as_ref().map(|node| {
            catch_errors!(
                self,
                self.evaluate_pinexpr(ctx, circ, node),
                PinExprType::Unknown
            )
        });

        // let result = match (lhs, rhs) {
        //     (Some(lhs @ PinExprType::DepedencyArray { name, circ_id, len }), rhs @ PinExprType::Index(index)) => match op {
        //         PinExprOpType::Index => PinExprType::Dependency {
        //             name,
        //             circ_id,
        //             index,
        //         },
        //         _ => todo!("invalid op between ...")
        //     },
        //     (Some(lhs @))

        let result = match op {
            PinExprOpType::GetChild => {
                let Some(lhs) = lhs else {
                    unreachable!("parser bug: lhs is None for get child operation");
                };

                let pin_name = match rhs.node_type() {
                    NodeType::Identifier(name) => *name,
                    _ => {
                        self.diagnostics.push(diagnostic!(
                            Error,
                            "get child operation requires an identifier on the right-hand side",
                            pos = rhs.pos(),
                        ));
                        return Ok(PinExprType::Unknown);
                    }
                };

                let PinExprType::Dependency {
                    name,
                    circ_id,
                    index,
                } = lhs
                else {
                    todo!("invalid lhs in get child operation: {lhs:#?}");
                };

                let circ = self
                    .circs
                    .get_by_index(circ_id)
                    .unwrap_or_else(|| unreachable!("invalid circ id: {circ_id}"));

                let pin_id = match circ.pins.get_index(&pin_name) {
                    Some(pin_id) => pin_id,
                    None => todo!("invalid pin diagnostic"),
                };

                let Some(&pin) = circ.pins.get(&pin_name) else {
                    unreachable!("pin id {pin_id} not found in pins for circ {circ:#?}");
                };

                PinExprType::DependencyPin {
                    name,
                    circ_id,
                    index,
                    pin_id,
                    pin,
                }
            }
            PinExprOpType::Index => {
                let Some(lhs) = lhs else {
                    unreachable!("parser bug: lhs is None for index operation");
                };

                let rhs = catch_errors!(
                    self,
                    self.evaluate_pinexpr(ctx, circ, rhs),
                    PinExprType::Unknown
                );

                if !self.pinexpr_bounds_check(pos, lhs, rhs) {
                    return Ok(PinExprType::Unknown);
                }

                // TODO: optimise this
                match (lhs, rhs) {
                    (
                        PinExprType::DependencyArray { name, circ_id, len },
                        PinExprType::Index(idx),
                    ) => PinExprType::Dependency {
                        name,
                        circ_id,
                        index: Some(idx),
                    },
                    (PinExprType::Pin { pin_id, pin }, PinExprType::Index(idx)) => {
                        PinExprType::PinRange {
                            pin_id,
                            pin,
                            start: Some(idx),
                            end: Some(idx + 1),
                        }
                    }
                    (
                        PinExprType::PinRange {
                            pin_id,
                            pin,
                            start,
                            end,
                        },
                        PinExprType::Index(idx),
                    ) => PinExprType::PinRange {
                        pin_id,
                        pin,
                        start: Some(start.unwrap_or(0) + idx),
                        end: Some(start.unwrap_or(0) + idx + 1),
                    },
                    (
                        PinExprType::DependencyPin {
                            name,
                            circ_id,
                            index,
                            pin_id,
                            pin,
                        },
                        PinExprType::Index(idx),
                    ) => PinExprType::DependencyPinRange {
                        name,
                        circ_id,
                        index,
                        pin_id,
                        pin,
                        start: Some(idx),
                        end: Some(idx + 1),
                    },
                    (
                        PinExprType::DependencyPinRange {
                            name,
                            circ_id,
                            index,
                            pin_id,
                            pin,
                            start,
                            end,
                        },
                        PinExprType::Index(idx),
                    ) => PinExprType::DependencyPinRange {
                        name,
                        circ_id,
                        index,
                        pin_id,
                        pin,
                        start: Some(start.unwrap_or(0) + idx),
                        end: Some(start.unwrap_or(0) + idx + 1),
                    },
                    // RANGE INDEXES
                    (PinExprType::Pin { pin_id, pin }, PinExprType::Range(s, e)) => {
                        PinExprType::PinRange {
                            pin_id,
                            pin,
                            start: Some(s.unwrap_or(0)),
                            end: Some(e.unwrap_or(pin.width)),
                        }
                    }
                    (
                        PinExprType::PinRange {
                            pin_id,
                            pin,
                            start,
                            end,
                        },
                        PinExprType::Range(s, e),
                    ) => PinExprType::PinRange {
                        pin_id,
                        pin,
                        start: Some(start.unwrap_or(0) + s.unwrap_or(0)),
                        end: Some(start.unwrap_or(0) + e.unwrap_or(pin.width)),
                    },
                    (
                        PinExprType::DependencyPin {
                            name,
                            circ_id,
                            index,
                            pin_id,
                            pin,
                        },
                        PinExprType::Range(s, e),
                    ) => PinExprType::DependencyPinRange {
                        name,
                        circ_id,
                        index,
                        pin_id,
                        pin,
                        start: Some(s.unwrap_or(0)),
                        end: Some(e.unwrap_or(pin.width)),
                    },
                    (
                        PinExprType::DependencyPinRange {
                            name,
                            circ_id,
                            index,
                            pin_id,
                            pin,
                            start,
                            end,
                        },
                        PinExprType::Range(s, e),
                    ) => PinExprType::DependencyPinRange {
                        name,
                        circ_id,
                        index,
                        pin_id,
                        pin,
                        start: Some(start.unwrap_or(0) + s.unwrap_or(0)),
                        end: Some(start.unwrap_or(0) + e.unwrap_or(pin.width)),
                    },
                    (PinExprType::Unknown, _) | (_, PinExprType::Unknown) => PinExprType::Unknown,
                    _ => todo!("cannot index lhs"),
                }
            }
        };

        Ok(result)
    }
}

fn extract_identifier(node: &Node) -> IdentId {
    match node.node_type() {
        NodeType::Identifier(name) => *name,
        _ => unreachable!("parser bug"),
    }
}
