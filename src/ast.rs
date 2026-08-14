/// Abstract syntax tree produced by the parser. Shared verbatim by both
/// execution backends: the tree-walking interpreter evaluates it directly,
/// and the bytecode compiler walks it once to emit `OpCode`s.
#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LogicOp {
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    Str(String),
    Bool(bool),
    Nil,
    Ident(String, u32),
    Array(Vec<Expr>),
    Map(Vec<(String, Expr)>),
    Index(Box<Expr>, Box<Expr>, u32),
    Unary(UnOp, Box<Expr>, u32),
    Binary(BinOp, Box<Expr>, Box<Expr>, u32),
    Logical(LogicOp, Box<Expr>, Box<Expr>),
    Assign(String, Box<Expr>, u32),
    IndexAssign(Box<Expr>, Box<Expr>, Box<Expr>, u32),
    Call(String, Vec<Expr>, u32),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let(String, Expr, u32),
    ExprStmt(Expr),
    Print(Vec<Expr>, u32),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    While(Expr, Vec<Stmt>),
    /// A bare `{ ... }` block introducing its own lexical scope. Also used to
    /// desugar `for (init; cond; post) body` into
    /// `{ init; while (cond) { body; post; } }` without a dedicated AST node.
    Block(Vec<Stmt>),
    Return(Option<Expr>, u32),
}
