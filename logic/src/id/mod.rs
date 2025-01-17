//! Adds a unique ID to each node on the AST (unique in the current AST).

#[cfg(test)]
mod test;

use std::{fmt, marker::PhantomData, mem};

use rustc_hash::FxHashMap;

use crate::{
    diagnostics::span::{HasSpan, Span, Spanned},
    parse::{
        block::Block,
        expr::{BinOp, Expr, UnOp},
        func::{Func, Return},
        ident::Ident,
        lit::Literal,
        r#for::{Between, ForLoop},
        r#if::{Branch, If},
        r#while::While,
        Ast, Node,
    },
};

#[derive(Debug, Default)]
pub struct MonotonicIdGenerator {
    current: usize,
}

impl MonotonicIdGenerator {
    pub fn new(&mut self) -> Id {
        let ret = Id {
            inner: self.current,
        };
        self.current += 1;
        ret
    }
}

/// Uniquely identifies an item within a single file.
#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub struct Id {
    inner: usize,
}

impl Id {
    pub fn new(inner: usize) -> Self {
        Self { inner }
    }

    pub fn raw_id(&self) -> usize {
        self.inner
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Tagged<T> {
    pub(crate) token: T,
    pub(crate) id: Id,
}

impl<T: HasSpan> HasSpan for Tagged<T> {
    fn span(&self) -> Span {
        self.token.span()
    }
}

impl<T> std::ops::Deref for Tagged<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.token
    }
}

#[derive(Debug, Default)]
struct TaggingCtx<'a> {
    variable_ids: FxHashMap<Ident<'a>, Id>,
    id_to_names: FxHashMap<Id, Ident<'a>>,
    monotonic: MonotonicIdGenerator,
    scopes: Vec<Scope<'a>>,
}

impl<'a> TaggingCtx<'a> {
    fn push_scope(&mut self) {
        self.scopes.push(Scope::default())
    }

    fn pop_scope(&mut self, remove_additions: bool) {
        if let Some(scope) = self.scopes.pop() {
            let mut for_removal = vec![];
            self.variable_ids
                .iter_mut()
                .map(|(key, val)| {
                    match scope.edits.iter().find(|edit| match edit {
                        Edit::Overwrite { ident, id, with: _ } => ident == key && val != id,
                        Edit::Add(tagged) => tagged.token == *key,
                    }) {
                        Some(edit) => match edit {
                            Edit::Overwrite {
                                ident: _,
                                id,
                                with: _,
                            } => *val = *id,
                            Edit::Add(ident) if remove_additions => {
                                for_removal.push(ident.token);
                            }
                            _ => {}
                        },
                        _ => (),
                    }
                })
                .for_each(drop);
            for each in for_removal {
                self.variable_ids.remove(&each);
            }
        };
    }
}

fn tagged_ident<'a>(ident: Ident<'a>, ctx: &mut TaggingCtx<'a>) -> Tagged<Ident<'a>> {
    if let Some(id) = ctx.variable_ids.get(&ident) {
        let id: Id = *id;
        Tagged { token: ident, id }
    } else {
        let tagged = ctx.tag(ident);
        ctx.variable_ids.insert(ident, tagged.id);
        ctx.id_to_names.insert(tagged.id, ident);
        if let Some(scope) = ctx.scopes.iter_mut().last() {
            scope.edits.push(Edit::Add(tagged.clone()))
        }
        tagged
    }
}

#[derive(Debug, Default)]
pub struct Scope<'a> {
    edits: Vec<Edit<'a>>,
}

#[derive(Debug)]
pub enum Edit<'a> {
    Overwrite { ident: Ident<'a>, id: Id, with: Id },
    Add(Tagged<Ident<'a>>),
}

impl<'a> TaggingCtx<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    fn tag<T>(&mut self, token: T) -> Tagged<T> {
        Tagged {
            token,
            id: self.monotonic.new(),
        }
    }
}

pub fn tag<'a>(ast: Ast<'a>) -> Ast<'a, Tagged<Ident<'a>>, TaggedExpr<'a>> {
    let mut ctx = TaggingCtx::new();
    tagged_ast(ast, &mut ctx)
}

pub type TaggedAst<'a> = Ast<'a, TaggedIdent<'a>, TaggedExpr<'a>>;
pub type TaggedNode<'a> = Node<'a, TaggedIdent<'a>, TaggedExpr<'a>>;
pub type TaggedIdent<'a> = Tagged<Ident<'a>>;
pub type TaggedFunc<'a> = Func<'a, TaggedIdent<'a>, TaggedExpr<'a>>;
pub type TaggedBlock<'a> = Block<'a, TaggedIdent<'a>, TaggedExpr<'a>>;
pub type TaggedBranch<'a> = Branch<'a, TaggedIdent<'a>, TaggedExpr<'a>>;
pub type TaggedFor<'a> = ForLoop<'a, TaggedIdent<'a>, TaggedExpr<'a>>;
pub type TaggedIf<'a> = If<'a, TaggedIdent<'a>, TaggedExpr<'a>>;
pub type TaggedWhile<'a> = While<'a, TaggedIdent<'a>, TaggedExpr<'a>>;
pub type TaggedReturn<'a> = Return<'a, TaggedExpr<'a>>;

fn tagged_ast<'a>(ast: Ast<'a>, ctx: &mut TaggingCtx<'a>) -> TaggedAst<'a> {
    ctx.push_scope();
    let ret = Ast {
        nodes: ast
            .nodes
            .into_iter()
            .map(|node| tagged_node(node, ctx))
            .collect(),
        indent: ast.indent,
    };
    ctx.pop_scope(false);
    ret
}

fn tagged_node<'a>(
    node: Node<'a>,
    ctx: &mut TaggingCtx<'a>,
) -> Node<'a, Tagged<Ident<'a>>, TaggedExpr<'a>> {
    match node {
        Node::Expr(exp) => Node::Expr(tagged_expr(exp, ctx)),
        Node::For(for_loop) => Node::For(ForLoop {
            var: tagged_ident(for_loop.var, ctx),
            between: {
                Between {
                    start: tagged_expr(for_loop.between.start, ctx),
                    stop: tagged_expr(for_loop.between.stop, ctx),
                    step: for_loop.between.step.map(|expr| tagged_expr(expr, ctx)),
                    _i: PhantomData,
                }
            },
            block: tagged_block(for_loop.block, ctx, false),
            indent: for_loop.indent,
            span: for_loop.span,
        }),
        Node::If(stmt) => Node::If(tagged_if(stmt, ctx)),
        Node::While(block) => Node::While(tagged_while(block, ctx)),
        Node::Return(ret) => Node::Return(tagged_ret(ret, ctx)),
        Node::Func(func) => Node::Func(tagged_func(func, ctx)),
    }
}

fn tagged_func<'a>(
    func: Func<'a>,
    ctx: &mut TaggingCtx<'a>,
) -> Func<'a, Tagged<Ident<'a>>, TaggedExpr<'a>> {
    let name = tagged_ident(func.name, ctx);

    let mut local_variables = FxHashMap::default();
    mem::swap(&mut ctx.variable_ids, &mut local_variables);

    let parameters = func
        .parameters
        .into_iter()
        .map(|param| tagged_ident(param, ctx))
        .collect();

    let block = tagged_block(func.block, ctx, true);

    mem::swap(&mut ctx.variable_ids, &mut local_variables);

    Func {
        name,
        parameters,
        block,
        indent: func.indent,
    }
}

fn tagged_ret<'a>(ret: Return<'a>, ctx: &mut TaggingCtx<'a>) -> Return<'a, TaggedExpr<'a>> {
    Return {
        expr: tagged_expr(ret.expr, ctx),
        indent: ret.indent,
        _a: PhantomData,
    }
}

fn tagged_while<'a>(
    block: While<'a>,
    ctx: &mut TaggingCtx<'a>,
) -> While<'a, Tagged<Ident<'a>>, TaggedExpr<'a>> {
    While {
        condition: tagged_expr(block.condition, ctx),
        block: tagged_block(block.block, ctx, false),
        indent: block.indent,
        span: block.span,
    }
}

fn tagged_if<'a>(
    block: If<'a>,
    ctx: &mut TaggingCtx<'a>,
) -> If<'a, Tagged<Ident<'a>>, TaggedExpr<'a>> {
    If {
        r#if: tagged_branch(block.r#if, ctx),
        else_ifs: block
            .else_ifs
            .into_iter()
            .map(|branch| tagged_branch(branch, ctx))
            .collect(),
        r#else: block.r#else.map(|block| tagged_block(block, ctx, false)),
        indent: block.indent,
        span: block.span,
    }
}

fn tagged_branch<'a>(
    branch: Branch<'a>,
    ctx: &mut TaggingCtx<'a>,
) -> Branch<'a, Tagged<Ident<'a>>, TaggedExpr<'a>> {
    Branch {
        condition: tagged_expr(branch.condition, ctx),
        block: tagged_block(branch.block, ctx, false),
    }
}

fn tagged_block<'a>(
    block: Block<'a>,
    ctx: &mut TaggingCtx<'a>,
    remove_additions: bool,
) -> Block<'a, Tagged<Ident<'a>>, TaggedExpr<'a>> {
    ctx.push_scope();
    let tagged = tagged_ast(block.inner, ctx);
    let ret = Block { inner: tagged };
    ctx.pop_scope(remove_additions);
    ret
}

fn tagged_expr<'a>(expr: Expr<'a>, ctx: &mut TaggingCtx<'a>) -> TaggedExpr<'a> {
    match expr {
        Expr::Ident(ident) => {
            let tagged = tagged_ident(ident, ctx);
            ctx.tag(TaggedExprInner::Ident(tagged))
        }
        Expr::Literal(lit) => ctx.tag(TaggedExprInner::Literal(lit)),
        Expr::BinOp(op, left, right) => {
            let res = TaggedExprInner::BinOp(
                op,
                {
                    let tagged = tagged_expr(*left, ctx);
                    Box::new(tagged)
                },
                {
                    let tagged = tagged_expr(*right, ctx);
                    Box::new(tagged)
                },
            );
            ctx.tag(res)
        }
        Expr::UnOp(op, left) => {
            let res = TaggedExprInner::UnOp(op, {
                let tagged = tagged_expr(*left, ctx);
                Box::new(tagged)
            });
            ctx.tag(res)
        }
        Expr::FunctionCall(name, vars) => {
            let res = TaggedExprInner::FunctionCall(
                tagged_ident(name, ctx),
                vars.into_iter()
                    .map(|expr| tagged_expr(expr, ctx))
                    .collect(),
            );
            ctx.tag(res)
        }
    }
}

pub type TaggedExpr<'a> = Tagged<TaggedExprInner<'a, Tagged<Ident<'a>>>>;

#[derive(Debug, PartialEq, Eq)]
pub enum TaggedExprInner<'a, IDENT = Ident<'a>> {
    Ident(IDENT),
    Literal(Spanned<Literal<'a>>),
    BinOp(Spanned<BinOp>, Box<TaggedExpr<'a>>, Box<TaggedExpr<'a>>),
    UnOp(Spanned<UnOp>, Box<TaggedExpr<'a>>),
    FunctionCall(IDENT, Vec<TaggedExpr<'a>>),
}

impl<IDENT: HasSpan> HasSpan for TaggedExprInner<'_, IDENT> {
    fn span(&self) -> Span {
        todo!()
    }
}
