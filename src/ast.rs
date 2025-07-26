mod args;
mod block;
mod circuit;
mod constexpr;
mod expression;
mod ident;
mod literal;
mod params;
mod program;
mod statement;
mod r#type;

mod yaml;

use yaml::ToYAML;

use crate::file::FileRef;

use super::token::TokenStream;
use super::error::CktError;

macro_rules! NodeTypes {
    { $($i:ident($t:path)),*$(,)? } => {
        #[derive(Debug)]
        enum NodeType {
            $($i($t)),*
        }

        trait WithType<T: NodeTrait> {
            fn with(start: usize, end: usize, t: T) -> Self;
        }

        $(
            impl WithType<$t> for Node {
                fn with(start: usize, end: usize, t: $t) -> Self {
                    Self {
                        node_type: NodeType::$i(t),
                        start,
                        end,
                    }
                }
            }
        )*
    };
}

NodeTypes! {
    Block(block::BlockNode),
    Circuit(circuit::CircuitNode),
    ConstExpr(constexpr::ConstExprNode),
    Decl(statement::DeclNode),
    Expression(expression::ExpressionNode),
    ExpressionPart(expression::ExpressionPart),
    Ident(ident::IdentNode),
    Literal(literal::LiteralNode),
    Operation(statement::OperationNode),
    Program(program::ProgramNode),
    Type(r#type::TypeNode),
    Use(statement::UseNode),
}


trait NodeTrait {
    fn parse(tokens: &mut TokenStream) -> Result<(usize, usize, Self), CktError>
    where
        Self: Sized;
}

#[derive(Debug)]
pub struct Node {
    node_type: NodeType,
    start: usize,
    end: usize,
}

impl Node {
    fn as_ref(self) -> NodeRef {
        Some(Box::new(self))
    }

    fn parse<T>(tokens: &mut TokenStream) -> Result<Self, CktError>
    where
        T: NodeTrait,
        Node: WithType<T>
    {
        let (start, end, value) = T::parse(tokens)?;
        Ok(Self::with(start, end, value))
    }
}

pub type NodeRef = Option<Box<Node>>;

#[derive(Debug)]
pub struct AST {
    file: FileRef,
    root: NodeRef,
}

impl AST {
    pub fn new(mut tokens: TokenStream) -> Result<Self, CktError> {
        Ok(Self {
            file: tokens.source.clone(),
            root: Node::parse::<program::ProgramNode>(&mut tokens)?.as_ref(),
        })
    }
}

impl std::fmt::Display for AST {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.yaml().join("\n"))
    }
}