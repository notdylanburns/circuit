use super::{NodeRef, NodeType, AST};
use super::block::BlockNode;
use super::circuit::CircuitNode;
use super::constexpr::ConstExprNode;
use super::expression::{ExpressionNode, ExpressionPart};
use super::ident::IdentNode;
use super::literal::LiteralNode;
use super::program::ProgramNode;
use super::statement::{DeclNode, OperationNode, Operator, UseNode};
use super::r#type::TypeNode;

macro_rules! indent {
    ($e:expr) => {
        $e.into_iter()
            .map(|s| format!("  {s}"))
    };

    (array $e:expr) => {
        $e.into_iter()
            .enumerate()
            .map(|(i, s)| match i {
                0 => format!("  - {s}"),
                _ => format!("    {s}"),
            })
    };
}

macro_rules! field {
    ($o:ident $name:literal: $value:expr) => {
        $o.push(format!("{}: {}", $name, $value));
    };
    ($o:ident $name:literal: map $value:expr) => {
        $o.push(format!("{}:", $name));
        $o.extend(indent!(yaml(&$value)));
    };
    ($o:ident $name:literal: arr $value:expr) => {
        $o.push(format!("{}:", $name));
        for node in &$value {
            $o.extend(indent!(array yaml(&node)));
        }
    };
}

macro_rules! fields {
    (
        $(
            $name1:literal: $(lit $value1:expr;)? $(map $value2:expr;)? $(arr $value3:expr;)?;
        )*
    ) => {
        let mut v = Vec::new();
        $(
            field!(v $name1: $($value1)? $(map $value2)? $(arr $value3)?);
        )*
        v
    }
}

macro_rules! yaml_match {
    ($n:ident { $($i:ident),*$(,)? }) => {
        match &$n.node_type {
            $(
                NodeType::$i(x) => {
                    let mut base = vec![format!("{}:", stringify!($i))];
                    base.extend(indent!(x.yaml()));
                    base
                },
            )*
        }
    };
}

fn yaml(r: &NodeRef) -> Vec<String> {
    match r {
        Some(n) => {
            let mut base_fields = vec![
                format!("start: {}", n.start),
                format!("end: {}", n.end),
            ];

            base_fields.extend(yaml_match!(n {
                Block,
                Circuit,
                ConstExpr,
                Decl,
                Expression,
                ExpressionPart,
                Ident,
                Literal,
                Operation,
                Program,
                Type,
                Use,
            }));

            base_fields
        },
        None => vec!["None".into()],
    }
}

pub trait ToYAML {
    fn yaml(&self) -> Vec<String>;
}

impl ToYAML for AST {
    fn yaml(&self) -> Vec<String> {
        let mut base_fields = vec![
            "AST:".into(),
            format!("  file: {:?}", self.file.borrow().path),
            "  root:".into()
        ];

        base_fields.extend(indent!(indent!(yaml(&self.root))));

        base_fields
    }
}

impl ToYAML for BlockNode {
    fn yaml(&self) -> Vec<String> {
        fields! {
            "statements": arr self.stmts ;;
        }
    }
}

impl ToYAML for CircuitNode {
    fn yaml(&self) -> Vec<String> {
        fields! {
            "name": map self.name ;;
            "params": arr self.params ;;
            "block": map self.block ;;
        }
    }
}

impl ToYAML for ConstExprNode {
    fn yaml(&self) -> Vec<String> {
        fields! {
            "lhs": map self.lhs ;;
        }
    }
}

impl ToYAML for ExpressionNode {
    fn yaml(&self) -> Vec<String> {
        fields! {
            "base": map self.base ;;
            "parts": arr self.parts ;;
        }
    }
}

impl ToYAML for ExpressionPart {
    fn yaml(&self) -> Vec<String> {
        match self {
            Self::Child(n) => { fields! { "child": map n;; } },
            Self::Index(n) => { fields! { "index": map n;; } },
            Self::Range(s, e) => { fields! { "start": map s;; "end": map e;; } },
        }
    }
}

impl ToYAML for IdentNode {
    fn yaml(&self) -> Vec<String> {
        fields! {
            "name": lit self.name ;;
        }
    }
}

impl ToYAML for LiteralNode {
    fn yaml(&self) -> Vec<String> {
        match self {
            Self::Numeric(n) => vec![format!("number: {n}")]
        }
    }
}

impl ToYAML for ProgramNode {
    fn yaml(&self) -> Vec<String> {
        fields! {
            "children": arr self.children ;;
        }
    }
}

impl ToYAML for DeclNode {
    fn yaml(&self) -> Vec<String> {
        fields! {
            "name": map self.name ;;
            "type": map self.r#type ;;
        }
    }
}

impl ToYAML for OperationNode {
    fn yaml(&self) -> Vec<String> {
        fields! {
            "lhs": map self.lhs ;;
            "op": lit match self.op {
                Operator::Write => "write"
            } ;;
            "rhs": map self.rhs ;;
        }
    }
}

impl ToYAML for UseNode {
    fn yaml(&self) -> Vec<String> {
        todo!();
    }
}

impl ToYAML for TypeNode {
    fn yaml(&self) -> Vec<String> {
        fields! {
            "type": map self.r#type ;;
            "vars": arr self.vars ;;
            "width": map self.width ;;
        }
    }
}