use crate::loader::ModuleId;
use crate::tokeniser::{punctuation, IdentId, PunctuationType, TokenType};
use crate::util::{Pos, Position};

#[derive(Debug)]
pub struct AST {
    module: ModuleId,
    nodes: Vec<Node>,
}

#[allow(dead_code)]
impl AST {
    pub fn new(module: ModuleId) -> Self {
        Self {
            module,
            nodes: Vec::new(),
        }
    }

    pub fn add_node(&mut self, node: Node) {
        self.nodes.push(node);
    }

    pub fn iter_nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter()
    }

    pub fn iter_nodes_mut(&mut self) -> impl Iterator<Item = &mut Node> {
        self.nodes.iter_mut()
    }

    pub fn module_id(&self) -> ModuleId {
        self.module
    }
}

impl IntoIterator for AST {
    type Item = Node;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.nodes.into_iter()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinDirection {
    Input,
    Output,
    Transput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionDirection {
    LeftToRight,
    RightToLeft,
    Bidrectional,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Associativity {
    Left,
    Right,
    Unary,
}

macro_rules! const_expr_ops {
    (
        $(
            $name:ident(
                $precedence:literal,
                $associativity:ident
                $(,
                    $token:ident
                    $(,
                        $unary_equiv:ident
                    )?
                )?
            )
        ),*
        $(,)?
    ) =>{
        #[derive(Debug, Eq, PartialEq, Copy, Clone)]
        pub enum ConstExprOpType {
            $($name),*
        }

        impl ConstExprOpType {
            pub fn precedence(&self) -> usize {
                match self {
                    $(Self::$name => $precedence,)*
                }
            }

            pub fn higher_precedence(&self, other: &Self) -> bool {
                self.precedence() > other.precedence()
            }

            pub fn lower_precedence(&self, other: &Self) -> bool {
                self.precedence() < other.precedence()
            }

            pub fn associativity(&self) -> Associativity {
                match self {
                    $(Self::$name => Associativity::$associativity,)*
                }
            }

            pub fn is_unary(&self) -> bool {
                match self {
                    $(Self::$name => Associativity::$associativity == Associativity::Unary,)*
                }
            }

            pub fn unary_equivalent(&self) -> Option<Self> {
                match self {
                    $(
                        $(
                            $(Self::$name => Some(Self::$unary_equiv),)?
                        )?
                    )*
                    _ => None,
                }
            }

            pub fn as_string(&self) -> &'static str {
                match self {
                    $($(Self::$name => (PunctuationType::$token).value(),)?)*
                    $($($(Self::$unary_equiv => (PunctuationType::$token.value()),)?)?)*
                }
            }
        }

        impl TryFrom<&TokenType> for ConstExprOpType {
            type Error = ();

            fn try_from(tt: &TokenType) -> Result<Self, Self::Error> {
                match tt {
                    $($(token_type if token_type == &punctuation!($token) => Ok(Self::$name),)?)*
                    _ => Err(()),
                }
            }
        }
    };
}

const_expr_ops! {
    Add(3, Left, Plus),
    Sub(3, Left, Minus, UnaryMinus),
    Mul(4, Left, Star),
    Div(4, Left, Slash),
    Mod(4, Left, Percent),
    And(0, Left, DAmpersand),
    Or(0, Left, DPipe),
    Xor(0, Left, DCaret),
    Not(7, Unary, Exclamation),
    Shl(6, Left, DLessThan),
    Shr(6, Left, DGreaterThan),
    Eq(1, Left, DEqual),
    Neq(1, Left, ExclamationEqual),
    Lt(1, Left, LessThan),
    Lte(1, Left, LessThanEqual),
    Gt(1, Left, GreaterThan),
    Gte(1, Left, GreaterThanEqual),
    BitAnd(5, Left, Ampersand),
    BitOr(5, Left, Pipe),
    BitXor(5, Left, Caret),
    BitNot(8, Unary, Tilde),
    UnaryMinus(8, Unary),
    Range(2, Left, Range),
}

macro_rules! pin_expr_ops {
    (
        $(
            $name:ident(
                $precedence:literal,
                $associativity:ident,
                $token:ident
            )
        ),*
        $(,)?
    ) =>{
        #[derive(Debug, Eq, PartialEq, Copy, Clone)]
        pub enum PinExprOpType {
            $($name),*
        }

        impl PinExprOpType  {
            pub fn precedence(&self) -> usize {
                match self {
                    $(Self::$name => $precedence,)*
                }
            }

            pub fn higher_precedence(&self, other: &Self) -> bool {
                self.precedence() > other.precedence()
            }

            pub fn lower_precedence(&self, other: &Self) -> bool {
                self.precedence() < other.precedence()
            }

            pub fn associativity(&self) -> Associativity {
                match self {
                    $(Self::$name => Associativity::$associativity,)*
                }
            }

            pub fn is_unary(&self) -> bool {
                match self {
                    $(Self::$name => Associativity::$associativity == Associativity::Unary,)*
                }
            }

            pub fn as_string(&self) -> &'static str {
                match self {
                    $(Self::$name => (PunctuationType::$token).value(),)*
                }
            }
        }

        impl TryFrom<&TokenType> for PinExprOpType {
            type Error = ();

            fn try_from(tt: &TokenType) -> Result<Self, Self::Error> {
                match tt {
                    $(token_type if token_type == &punctuation!($token) => Ok(Self::$name),)*
                    _ => Err(()),
                }
            }
        }
    };
}

pin_expr_ops! {
    Index(1, Left, LSquare),
    GetChild(1, Left, Period),
}

#[derive(Debug)]
pub enum NodeType {
    None,
    Identifier(IdentId),
    Path(Vec<Node>),
    Integer(isize),
    ConstDecl {
        r#type: Box<Node>,
        name: Box<Node>,
        expr: Box<Node>,
    },
    Circ {
        name: Box<Node>,
        args: Vec<Node>,
        children: Vec<Node>,
    },
    CircArg {
        r#type: Box<Node>,
        name: Box<Node>,
        default: Option<Box<Node>>,
    },
    Connection {
        lhs: Box<Node>,
        direction: ConnectionDirection,
        rhs: Box<Node>,
    },
    Enum {
        name: Box<Node>,
        variants: Vec<Node>,
    },
    Statement(Box<Node>),
    Type {
        name: Box<Node>,
        args: Vec<Node>,
    },
    TypeArg {
        name: Option<Box<Node>>,
        value: Box<Node>,
    },
    PinExpr {
        lhs: Option<Box<Node>>,
        op: PinExprOpType,
        rhs: Box<Node>,
    },
    ConstExpr {
        lhs: Option<Box<Node>>,
        op: ConstExprOpType,
        rhs: Option<Box<Node>>,
    },
    Assert(Box<Node>),
    Import(Box<Node>),
    If {
        condition: Box<Node>,
        then: Vec<Node>,
        r#else: Vec<Node>,
    },
    Decl {
        name: Box<Node>,
        count: Option<Box<Node>>,
    },
    Decls {
        r#type: Box<Node>,
        decls: Vec<Node>,
    },
    PinDecls {
        direction: PinDirection,
        decls: Vec<Node>,
    },
    With(Vec<Node>),
    Use {
        path: Box<Node>,
        as_name: Option<Box<Node>>,
    },
}

#[derive(Debug)]
pub struct Node {
    node_type: NodeType,
    pos: Pos,
}

impl Node {
    pub const BUILTIN: Self = Self {
        node_type: NodeType::None,
        pos: Pos::Builtin,
    };

    pub fn new(node_type: NodeType, pos: Pos) -> Self {
        Self { node_type, pos }
    }

    pub fn node_type(&self) -> &NodeType {
        &self.node_type
    }

    pub fn parts(self) -> (NodeType, crate::util::Pos) {
        let pos = self.pos();
        (self.node_type, pos)
    }
}

impl Position for Node {
    fn pos(&self) -> Pos {
        self.pos
    }
}

mod formatter {
    use super::{Node, NodeType};
    use crate::util::Interner;

    struct NodeFormatter<'a> {
        interner: &'a Interner<String>,
        node: &'a Node,
    }

    impl<'a> std::fmt::Display for NodeFormatter<'a> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self.node.node_type() {
                NodeType::None => unreachable!(),
                NodeType::Identifier(ident_id) => self.interner.get(*ident_id).unwrap().fmt(f),
                NodeType::Path(nodes) => nodes
                    .iter()
                    .map(|n| n.format_node(&self.interner).to_string())
                    .collect::<Vec<_>>()
                    .join(":")
                    .fmt(f),
                NodeType::Integer(value) => value.fmt(f),
                NodeType::ConstDecl { r#type, name, expr } => format!(
                    "const {} {} = {};",
                    name.format_node(self.interner),
                    r#type.format_node(self.interner),
                    expr.format_node(self.interner)
                )
                .fmt(f),
                _ => todo!(),
            }
        }
    }

    trait FormatNode<'a> {
        fn format_node(&'a self, interner: &'a Interner<String>) -> NodeFormatter<'a>;
    }

    impl<'a> FormatNode<'a> for Node {
        fn format_node(&'a self, interner: &'a Interner<String>) -> NodeFormatter<'a> {
            NodeFormatter {
                interner,
                node: self,
            }
        }
    }
}
