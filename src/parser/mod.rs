pub mod cursor;

use crate::ast::*;
use crate::lexer::{Span, Token, TokenWithContext};
use cursor::Cursor;
use miette::{Diagnostic, SourceSpan};
use std::collections::HashSet;
use thiserror::Error;

/// Parser error type
#[derive(Error, Debug, Diagnostic)]
#[error("Parse error: {message}")]
#[diagnostic(code(ved::parser))]
pub struct ParseError {
    pub message: String,
    #[label("here")]
    pub span: SourceSpan,
    #[source_code]
    pub source_code: String,
}

/// Parse result type
pub type ParseResult<T> = Result<T, ParseError>;

/// Parse tokens into an AST Program
pub fn parse(tokens: Vec<TokenWithContext>, source: &str) -> ParseResult<Program> {
    let mut cursor = Cursor::new(tokens);
    let start_span = cursor.span();

    let items = parse_items(&mut cursor, source)?;

    // Ensure we've consumed all tokens
    if !cursor.is_eof() {
        let span = cursor.span();
        return Err(ParseError {
            message: format!(
                "Unexpected token: {}",
                cursor
                    .peek()
                    .map(|t| t.token.to_string())
                    .unwrap_or_default()
            ),
            span: SourceSpan::from(span.start..span.end),
            source_code: source.to_string(),
        });
    }

    let end_span = cursor.span();

    Ok(Program {
        items,
        span: Span::new(start_span.start, end_span.end),
    })
}

/// Parse top-level items
fn parse_items(cursor: &mut Cursor, source: &str) -> ParseResult<Vec<Item>> {
    let mut items = Vec::new();

    loop {
        // Skip newlines and dedents at top level
        cursor.skip_newlines();
        while cursor.check(&Token::Dedent) {
            cursor.advance();
        }

        if cursor.is_eof() {
            break;
        }

        let item = parse_item(cursor, source)?;
        items.push(item);
    }

    Ok(items)
}

/// Parse a single top-level item
fn parse_item(cursor: &mut Cursor, source: &str) -> ParseResult<Item> {
    let _start_span = cursor.span();

    match cursor.peek() {
        Some(t) => match &t.token {
            Token::KwShape => parse_shape(cursor, source),
            Token::KwThink => parse_think(cursor, source),
            Token::KwScreen => parse_screen(cursor, source),
            Token::KwPiece => parse_piece(cursor, source),
            Token::KwRoutes => parse_routes(cursor, source),
            Token::KwServe => parse_serve(cursor, source),
            Token::KwDatabase => parse_database(cursor, source),
            Token::KwTask => parse_task(cursor, source),
            Token::KwQueue => parse_queue(cursor, source),
            Token::KwLive => parse_live(cursor, source),
            Token::KwAuth => parse_auth(cursor, source),
            _ => Err(make_error(
                cursor,
                source,
                &format!("Unexpected token: {:?}", t.token),
            )),
        },
        None => Err(make_error(cursor, source, "Unexpected end ")),
    }
}

/// Parse shape definition: shape Name { ... }
fn parse_shape(cursor: &mut Cursor, source: &str) -> ParseResult<Item> {
    let start_span = cursor.span();
    expect_token(cursor, source, Token::KwShape)?;

    let name = expect_ident(cursor, source)?;
    cursor.skip_newlines();

    let mut fields = Vec::new();
    while cursor.is_ident() {
        let field = parse_field_def(cursor, source)?;
        fields.push(field);
        cursor.skip_newlines();
    }

    Ok(Item::Shape(ShapeDef {
        name,
        fields,
        span: Span::new(start_span.start, cursor.span().end),
    }))
}

/// Parse field definition: name: Type = default
fn parse_field_def(cursor: &mut Cursor, source: &str) -> ParseResult<FieldDef> {
    let start_span = cursor.span();
    let name = expect_ident(cursor, source)?;

    expect_token(cursor, source, Token::Colon)?;

    let ty = parse_type(cursor, source)?;

    let default = if cursor.check(&Token::Eq) {
        cursor.advance();
        Some(parse_expr(cursor, source)?)
    } else {
        None
    };

    cursor.skip_newlines();

    Ok(FieldDef {
        name,
        ty,
        default,
        span: Span::new(start_span.start, cursor.span().end),
    })
}

/// Parse think (function) definition
fn parse_think(cursor: &mut Cursor, source: &str) -> ParseResult<Item> {
    let start_span = cursor.span();
    expect_token(cursor, source, Token::KwThink)?;

    let name = expect_ident(cursor, source)?;

    // Parse optional parameters: needs x: num, y: num
    let params = if cursor.check(&Token::KwNeeds) {
        cursor.advance();
        parse_params(cursor, source)?
    } else {
        Vec::new()
    };

    // Parse optional return type: -> Type
    let ret = if cursor.check(&Token::ThinArrow) {
        cursor.advance();
        Some(parse_type(cursor, source)?)
    } else {
        None
    };

    // Check for async
    let is_async = cursor.check(&Token::KwAsync);
    if is_async {
        cursor.advance();
    }

    cursor.skip_newlines();

    // Parse body: logic stmts and UI nodes into separate buckets
    let (body, ui_children) = parse_think_body(cursor, source)?;

    Ok(Item::Think(ThinkDef {
        name,
        params,
        ret,
        is_async,
        body,
        ui_children,
        span: Span::new(start_span.start, cursor.span().end),
    }))
}

/// Parse a think body, splitting UI nodes from logic statements.
/// Returns (logic_stmts, ui_nodes).
fn parse_think_body(cursor: &mut Cursor, source: &str) -> ParseResult<(Vec<Stmt>, Vec<UiNode>)> {
    let mut stmts: Vec<Stmt> = Vec::new();
    let mut ui_nodes: Vec<UiNode> = Vec::new();

    if cursor.check_indent().is_none() {
        return Ok((stmts, ui_nodes));
    }

    let indent_level = cursor
        .expect_indent()
        .map_err(|msg| make_error(cursor, source, &msg))?;

    loop {
        if cursor.check_dedent() || cursor.check_end_of_block() || cursor.is_eof() {
            while cursor.check_dedent() {
                cursor.advance();
            }
            break;
        }

        match cursor.peek().map(|t| t.token.clone()) {
            Some(Token::KwBox) | Some(Token::KwWords) | Some(Token::KwTap)
            | Some(Token::KwShow) | Some(Token::KwEach) => {
                let node = parse_ui_node(cursor, source)?;
                ui_nodes.push(node);
            }
            Some(_) => {
                let stmt = parse_stmt(cursor, source)?;
                stmts.push(stmt);
            }
            None => break,
        }

        cursor.skip_newlines();

        if let Some(level) = cursor.check_indent() {
            if level < indent_level {
                break;
            }
        } else if cursor.check_dedent() {
            break;
        }
    }

    Ok((stmts, ui_nodes))
}

/// Parse parameters list
fn parse_params(cursor: &mut Cursor, source: &str) -> ParseResult<Vec<Param>> {
    let mut params = Vec::new();

    loop {
        let start_span = cursor.span();
        let name = expect_ident(cursor, source)?;

        expect_token(cursor, source, Token::Colon)?;

        let ty = parse_type(cursor, source)?;

        params.push(Param {
            name,
            ty,
            span: Span::new(start_span.start, cursor.span().end),
        });

        if cursor.check(&Token::Comma) {
            cursor.advance();
        } else {
            break;
        }
    }

    Ok(params)
}

/// Parse type expression
fn parse_type(cursor: &mut Cursor, source: &str) -> ParseResult<TypeExpr> {
    let start_token = cursor.peek().cloned();

    match start_token {
        Some(t) => match &t.token {
            Token::TyNum => {
                cursor.advance();
                Ok(TypeExpr::Num)
            }
            Token::TyDec => {
                cursor.advance();
                Ok(TypeExpr::Dec)
            }
            Token::TyText => {
                cursor.advance();
                Ok(TypeExpr::Text)
            }
            Token::TyBool => {
                cursor.advance();
                Ok(TypeExpr::Bool)
            }
            Token::TyAny => {
                cursor.advance();
                Ok(TypeExpr::Any)
            }
            Token::KwNothing => {
                cursor.advance();
                Ok(TypeExpr::Nothing)
            }

            Token::TyList => {
                cursor.advance();
                expect_token(cursor, source, Token::LBracket)?;
                let inner = parse_type(cursor, source)?;
                expect_token(cursor, source, Token::RBracket)?;
                Ok(TypeExpr::List(Box::new(inner)))
            }

            Token::TyMap => {
                cursor.advance();
                expect_token(cursor, source, Token::LBracket)?;
                let key = parse_type(cursor, source)?;
                expect_token(cursor, source, Token::Comma)?;
                let val = parse_type(cursor, source)?;
                expect_token(cursor, source, Token::RBracket)?;
                Ok(TypeExpr::Map(Box::new(key), Box::new(val)))
            }

            Token::KwMaybe => {
                cursor.advance();
                expect_token(cursor, source, Token::LBracket)?;
                let inner = parse_type(cursor, source)?;
                expect_token(cursor, source, Token::RBracket)?;
                Ok(TypeExpr::Maybe(Box::new(inner)))
            }

            Token::KwOk => {
                cursor.advance();
                expect_token(cursor, source, Token::LBracket)?;
                let ok_ty = parse_type(cursor, source)?;
                expect_token(cursor, source, Token::Comma)?;
                let err_ty = parse_type(cursor, source)?;
                expect_token(cursor, source, Token::RBracket)?;
                Ok(TypeExpr::Ok(Box::new(ok_ty), Box::new(err_ty)))
            }

            Token::Ident(name) => {
                let name = name.clone();
                cursor.advance();
                Ok(TypeExpr::Named(name))
            }

            _ => Err(make_error(cursor, source, "Expected a type ")),
        },
        None => Err(make_error(cursor, source, "Unexpected end ")),
    }
}

/// Parse statement
fn parse_stmt(cursor: &mut Cursor, source: &str) -> ParseResult<Stmt> {
    let start_span = cursor.span();

    match cursor.peek() {
        Some(t) => match &t.token {
            Token::KwLet => parse_let(cursor, source),
            Token::KwRemember => parse_remember_stmt(cursor, source),
            Token::KwFetch => parse_fetch_stmt(cursor, source),
            Token::KwGive => parse_give(cursor, source),
            Token::KwFail => parse_fail(cursor, source),
            Token::KwWhen => parse_when(cursor, source),
            Token::KwEach => parse_each(cursor, source),
            Token::KwTransaction => parse_transaction(cursor, source),
            Token::KwOn => parse_on_event(cursor, source),
            Token::KwAsync => parse_async_stmt(cursor, source),

            // Arrow expression: => expr (shorthand for give expr)
            Token::FatArrow => {
                cursor.advance();
                let value = parse_expr(cursor, source)?;
                Ok(Stmt::Give {
                    value,
                    span: Span::new(start_span.start, cursor.span().end),
                })
            }

            // UI nodes in think body — parse and discard the node tree (UI output)
            Token::KwBox | Token::KwWords | Token::KwTap | Token::KwShow => {
                // consume any nested UI block gracefully
                cursor.advance(); // keyword
                cursor.skip_newlines();
                // consume optional name/label expression
                if cursor.check_indent().is_none()
                    && !cursor.is_eof()
                    && !cursor.check(&Token::Newline)
                {
                    let _ = parse_expr(cursor, source);
                }
                cursor.skip_newlines();
                // consume indented properties/children block
                if cursor.check_indent().is_some() {
                    cursor.advance();
                    let mut depth = 1i32;
                    while depth > 0 && !cursor.is_eof() {
                        if cursor.check_indent().is_some() {
                            depth += 1;
                        } else if cursor.check(&Token::Dedent) {
                            depth -= 1;
                        }
                        cursor.advance();
                    }
                }
                Ok(Stmt::Expr(Expr::Nothing(Span::new(
                    start_span.start,
                    cursor.span().end,
                ))))
            }

            // Assignment or expression statement
            Token::Ident(name) => {
                let name = name.clone();
                cursor.advance();

                if cursor.check(&Token::Eq) {
                    // Assignment
                    cursor.advance();
                    let value = parse_expr(cursor, source)?;
                    Ok(Stmt::Assign {
                        target: AssignTarget::Simple(
                            name,
                            Span::new(start_span.start, start_span.end),
                        ),
                        value,
                        span: Span::new(start_span.start, cursor.span().end),
                    })
                } else if cursor.check(&Token::LParen) {
                    // Function call statement
                    cursor.advance();
                    let args = parse_expr_list(cursor, source, Token::RParen)?;
                    expect_token(cursor, source, Token::RParen)?;
                    Ok(Stmt::Expr(Expr::Call {
                        func: Box::new(Expr::Ident(
                            name,
                            Span::new(start_span.start, start_span.end),
                        )),
                        args,
                        span: Span::new(start_span.start, cursor.span().end),
                    }))
                } else {
                    // Just an identifier expression
                    Ok(Stmt::Expr(Expr::Ident(
                        name,
                        Span::new(start_span.start, start_span.end),
                    )))
                }
            }

            // Expression statement
            _ => {
                let expr = parse_expr(cursor, source)?;
                Ok(Stmt::Expr(expr))
            }
        },
        None => Err(make_error(cursor, source, "Unexpected end ")),
    }
}

/// Parse let statement
fn parse_let(cursor: &mut Cursor, source: &str) -> ParseResult<Stmt> {
    let start_span = cursor.span();
    expect_token(cursor, source, Token::KwLet)?;

    let mutable = cursor.check(&Token::KwMut);
    if mutable {
        cursor.advance();
    }

    let name = expect_ident(cursor, source)?;

    // Optional type annotation
    let ty = if cursor.check(&Token::Colon) {
        cursor.advance();
        Some(parse_type(cursor, source)?)
    } else {
        None
    };

    expect_token(cursor, source, Token::Eq)?;

    let value = parse_expr(cursor, source)?;

    Ok(Stmt::Let {
        name,
        mutable,
        ty,
        value,
        span: Span::new(start_span.start, cursor.span().end),
    })
}

/// Parse remember statement
fn parse_remember_stmt(cursor: &mut Cursor, source: &str) -> ParseResult<Stmt> {
    let start_span = cursor.span();
    expect_token(cursor, source, Token::KwRemember)?;

    let name = expect_ident(cursor, source)?;

    let ty = if cursor.check(&Token::Colon) {
        cursor.advance();
        Some(parse_type(cursor, source)?)
    } else {
        None
    };

    expect_token(cursor, source, Token::Eq)?;

    let value = parse_expr(cursor, source)?;
    let dependencies = extract_expr_ident_refs(&value);
    let computed = !dependencies.is_empty();

    Ok(Stmt::Remember {
        name,
        ty,
        value,
        computed,
        dependencies,
        span: Span::new(start_span.start, cursor.span().end),
    })
}

/// Parse fetch statement
fn parse_fetch_stmt(cursor: &mut Cursor, source: &str) -> ParseResult<Stmt> {
    let start_span = cursor.span();
    expect_token(cursor, source, Token::KwFetch)?;

    let target = expect_ident(cursor, source)?;
    cursor.skip_newlines();

    let _indent_level = cursor
        .expect_indent()
        .map_err(|msg| make_error(cursor, source, &msg))?;

    let mut url: Option<Expr> = None;
    let mut when_deps: Vec<String> = Vec::new();
    let mut cache_duration: Option<String> = None;
    let mut loading_handler: Option<String> = None;
    let mut error_handler: Option<String> = None;

    loop {
        cursor.skip_newlines();

        if cursor.check(&Token::Dedent) {
            cursor.advance();
            break;
        }

        if cursor.is_eof() {
            break;
        }

        match cursor.peek() {
            Some(t) => match &t.token {
                Token::KwFrom => {
                    cursor.advance();
                    expect_token(cursor, source, Token::Colon)?;
                    url = Some(parse_expr(cursor, source)?);
                }
                Token::KwWhen => {
                    cursor.advance();
                    expect_token(cursor, source, Token::Colon)?;
                    expect_token(cursor, source, Token::LBracket)?;
                    when_deps = parse_ident_list(cursor, source)?;
                    expect_token(cursor, source, Token::RBracket)?;
                }
                Token::KwCache => {
                    cursor.advance();
                    expect_token(cursor, source, Token::Colon)?;
                    cache_duration = Some(parse_cache_duration(cursor, source)?);
                }
                Token::KwLoading => {
                    cursor.advance();
                    expect_token(cursor, source, Token::Colon)?;
                    loading_handler = Some(expect_ident(cursor, source)?);
                }
                Token::KwError => {
                    cursor.advance();
                    expect_token(cursor, source, Token::Colon)?;
                    error_handler = Some(expect_ident(cursor, source)?);
                }
                Token::Indent(_) | Token::Comma | Token::Newline => {
                    cursor.advance();
                }
                _ => {
                    return Err(make_error(
                        cursor,
                        source,
                        "Unexpected token in fetch block",
                    ));
                }
            },
            None => break,
        }
    }

    let Some(url) = url else {
        return Err(make_error(
            cursor,
            source,
            "Fetch statement requires 'from' URL",
        ));
    };

    Ok(Stmt::Fetch(FetchStmt {
        target,
        url,
        when_deps,
        cache_duration,
        loading_handler,
        error_handler,
        span: Span::new(start_span.start, cursor.span().end),
    }))
}

fn parse_ident_list(cursor: &mut Cursor, source: &str) -> ParseResult<Vec<String>> {
    let mut idents = Vec::new();

    if cursor.check(&Token::RBracket) {
        return Ok(idents);
    }

    loop {
        idents.push(expect_ident(cursor, source)?);

        if cursor.check(&Token::Comma) {
            cursor.advance();
        } else {
            break;
        }
    }

    Ok(idents)
}

fn parse_cache_duration(cursor: &mut Cursor, source: &str) -> ParseResult<String> {
    match cursor.peek() {
        Some(t) => match &t.token {
            Token::LitMs(s)
            | Token::LitSecs(s)
            | Token::LitHours(s)
            | Token::LitPercent(s)
            | Token::LitStr(s)
            | Token::LitStrRaw(s)
            | Token::Ident(s) => {
                let s = s.clone();
                cursor.advance();
                Ok(s)
            }
            Token::LitNum(Some(n)) => {
                let mut out = n.to_string();
                cursor.advance();

                if let Some(next) = cursor.peek() {
                    match &next.token {
                        Token::Ident(s) => {
                            out.push_str(s);
                            cursor.advance();
                        }
                        Token::LitMs(s)
                        | Token::LitSecs(s)
                        | Token::LitHours(s)
                        | Token::LitPercent(s) => {
                            out.push_str(s);
                            cursor.advance();
                        }
                        _ => {}
                    }
                }

                Ok(out)
            }
            _ => Err(make_error(cursor, source, "Expected cache duration value")),
        },
        None => Err(make_error(
            cursor,
            source,
            "Unexpected end while parsing cache duration",
        )),
    }
}

fn extract_expr_ident_refs(expr: &Expr) -> Vec<String> {
    let mut refs = Vec::new();
    collect_expr_ident_refs(expr, &mut refs);

    let mut seen = HashSet::new();
    refs.into_iter()
        .filter(|name| seen.insert(name.clone()))
        .collect()
}

fn collect_expr_ident_refs(expr: &Expr, refs: &mut Vec<String>) {
    match expr {
        Expr::Ident(name, _) => refs.push(name.clone()),

        Expr::Text(parts, _) => {
            for part in parts {
                if let TextPart::Interp(inner) = part {
                    collect_expr_ident_refs(inner, refs);
                }
            }
        }

        Expr::List(items, _) => {
            for item in items {
                collect_expr_ident_refs(item, refs);
            }
        }

        Expr::Map(entries, _) => {
            for (k, v) in entries {
                collect_expr_ident_refs(k, refs);
                collect_expr_ident_refs(v, refs);
            }
        }

        Expr::Construct { fields, .. } => {
            for (_, v) in fields {
                collect_expr_ident_refs(v, refs);
            }
        }

        Expr::Field { obj, .. } => collect_expr_ident_refs(obj, refs),
        Expr::Index { obj, index, .. } => {
            collect_expr_ident_refs(obj, refs);
            collect_expr_ident_refs(index, refs);
        }
        Expr::Call { func, args, .. } => {
            collect_expr_ident_refs(func, refs);
            for arg in args {
                collect_expr_ident_refs(arg, refs);
            }
        }
        Expr::Pipe { left, right, .. } | Expr::BinOp { left, right, .. } => {
            collect_expr_ident_refs(left, refs);
            collect_expr_ident_refs(right, refs);
        }
        Expr::Not { expr, .. }
        | Expr::Neg { expr, .. }
        | Expr::Wait { expr, .. }
        | Expr::Try { expr, .. } => collect_expr_ident_refs(expr, refs),

        Expr::Lambda { body, .. } => collect_expr_ident_refs(body, refs),

        Expr::With { base, fields, .. } => {
            collect_expr_ident_refs(base, refs);
            for (_, v) in fields {
                collect_expr_ident_refs(v, refs);
            }
        }

        Expr::Fetch(fetch) => {
            collect_expr_ident_refs(&fetch.url, refs);
            if let Some(body) = &fetch.body {
                collect_expr_ident_refs(body, refs);
            }
        }

        Expr::Handle {
            expr,
            ok_arm,
            fail_arm,
            ..
        } => {
            collect_expr_ident_refs(expr, refs);
            if let Some(arm) = ok_arm {
                collect_expr_ident_refs(&arm.body, refs);
            }
            if let Some(arm) = fail_arm {
                collect_expr_ident_refs(&arm.body, refs);
            }
        }

        Expr::DbQuery { sql, .. } => collect_expr_ident_refs(sql, refs),
        Expr::DbOne { key, .. } => collect_expr_ident_refs(key, refs),
        Expr::DbSave { record, .. } => collect_expr_ident_refs(record, refs),
        Expr::DbRemove { key, .. } => collect_expr_ident_refs(key, refs),

        Expr::Num(_, _)
        | Expr::Dec(_, _)
        | Expr::Bool(_, _)
        | Expr::Color(_, _)
        | Expr::Nothing(_)
        | Expr::Env { .. }
        | Expr::Param { .. }
        | Expr::DbAll { .. } => {}
    }
}

/// Parse give (return) statement
fn parse_give(cursor: &mut Cursor, source: &str) -> ParseResult<Stmt> {
    let start_span = cursor.span();
    expect_token(cursor, source, Token::KwGive)?;

    let value = parse_expr(cursor, source)?;

    Ok(Stmt::Give {
        value,
        span: Span::new(start_span.start, cursor.span().end),
    })
}

/// Parse fail (error) statement
fn parse_fail(cursor: &mut Cursor, source: &str) -> ParseResult<Stmt> {
    let start_span = cursor.span();
    expect_token(cursor, source, Token::KwFail)?;

    let value = parse_expr(cursor, source)?;

    Ok(Stmt::Fail {
        value,
        span: Span::new(start_span.start, cursor.span().end),
    })
}

/// Parse when (pattern matching) statement
fn parse_when(cursor: &mut Cursor, source: &str) -> ParseResult<Stmt> {
    let start_span = cursor.span();
    expect_token(cursor, source, Token::KwWhen)?;

    // Optional subject expression
    let _subject = if cursor.is_expr_start() {
        Some(parse_expr(cursor, source)?)
    } else {
        None
    };

    cursor.skip_newlines();

    let mut arms = Vec::new();

    // Parse arms: pattern => { body }
    while cursor.is_expr_start() || cursor.check(&Token::KwOtherwise) {
        if cursor.check(&Token::KwOtherwise) {
            break;
        }

        let arm_start = cursor.span();
        let cond = parse_expr(cursor, source)?;

        expect_token(cursor, source, Token::FatArrow)?;

        // Parse single statement or block
        let body = if cursor.check_indent().is_some() {
            parse_indented_block(cursor, source, parse_stmt)?
        } else {
            vec![parse_stmt(cursor, source)?]
        };

        arms.push(WhenArm {
            cond,
            body,
            span: Span::new(arm_start.start, cursor.span().end),
        });

        cursor.skip_newlines();
    }

    // Parse otherwise clause
    let otherwise = if cursor.check(&Token::KwOtherwise) {
        cursor.advance();
        expect_token(cursor, source, Token::FatArrow)?;

        let _body = if cursor.check_indent().is_some() {
            parse_indented_block(cursor, source, parse_stmt)?
        } else {
            vec![parse_stmt(cursor, source)?]
        };

        Some(Box::new(Stmt::Give {
            value: Expr::Nothing(Span::empty()), // Placeholder - should be last stmt
            span: Span::empty(),
        }))
    } else {
        None
    };

    Ok(Stmt::When {
        arms,
        otherwise,
        span: Span::new(start_span.start, cursor.span().end),
    })
}

/// Parse each (for) loop statement
fn parse_each(cursor: &mut Cursor, source: &str) -> ParseResult<Stmt> {
    let start_span = cursor.span();
    expect_token(cursor, source, Token::KwEach)?;

    let var = expect_ident(cursor, source)?;

    expect_token(cursor, source, Token::KwIn)?;

    let iter = parse_expr(cursor, source)?;

    cursor.skip_newlines();

    let body = parse_indented_block(cursor, source, parse_stmt)?;

    Ok(Stmt::Each {
        var,
        iter,
        body,
        span: Span::new(start_span.start, cursor.span().end),
    })
}

/// Parse transaction block
fn parse_transaction(cursor: &mut Cursor, source: &str) -> ParseResult<Stmt> {
    let start_span = cursor.span();
    expect_token(cursor, source, Token::KwTransaction)?;

    cursor.skip_newlines();

    let body = parse_indented_block(cursor, source, parse_stmt)?;

    Ok(Stmt::Transaction {
        body,
        span: Span::new(start_span.start, cursor.span().end),
    })
}

/// Parse on event handlers
fn parse_on_event(cursor: &mut Cursor, source: &str) -> ParseResult<Stmt> {
    let start_span = cursor.span();
    expect_token(cursor, source, Token::KwOn)?;

    // Parse event kind
    let event_kind = match cursor.peek() {
        Some(t) => match &t.token {
            Token::KwLoad => {
                cursor.advance();
                "load".to_string()
            }
            Token::KwTap => {
                cursor.advance();
                "tap".to_string()
            }
            Token::KwHover => {
                cursor.advance();
                "hover".to_string()
            }
            Token::LitStr(s) => {
                let s = s.clone();
                cursor.advance();
                s
            }
            _ => return Err(make_error(cursor, source, "Expected event type ")),
        },
        None => return Err(make_error(cursor, source, "Unexpected end ")),
    };

    expect_token(cursor, source, Token::FatArrow)?;

    let body = parse_indented_block(cursor, source, parse_stmt)?;

    // Return appropriate On* variant based on event kind
    match event_kind.as_str() {
        "load" => Ok(Stmt::OnLoad {
            body,
            span: Span::new(start_span.start, cursor.span().end),
        }),
        "tap" => Ok(Stmt::OnTap {
            body,
            span: Span::new(start_span.start, cursor.span().end),
        }),
        "hover" => Ok(Stmt::OnHover {
            body,
            span: Span::new(start_span.start, cursor.span().end),
        }),
        _ => Ok(Stmt::OnKey {
            key: event_kind,
            body,
            span: Span::new(start_span.start, cursor.span().end),
        }),
    }
}

/// Parse async statement
fn parse_async_stmt(cursor: &mut Cursor, source: &str) -> ParseResult<Stmt> {
    // For now, just parse as regular statement with async marker
    cursor.advance(); // consume async
    parse_stmt(cursor, source)
}

/// === Expression Parsing ===
/// Parse expression (entry point)
fn parse_expr(cursor: &mut Cursor, source: &str) -> ParseResult<Expr> {
    parse_pipe(cursor, source)
}

/// Parse pipe expression: expr |> func |> other
fn parse_pipe(cursor: &mut Cursor, source: &str) -> ParseResult<Expr> {
    let start_span = cursor.span();
    let mut left = parse_or(cursor, source)?;

    while cursor.check(&Token::PipeOp) {
        cursor.advance();
        let right = parse_or(cursor, source)?;

        // Transform: left |> right => right(left) if right is a function
        // Or: left |> f |> g => g(f(left))
        left = Expr::Pipe {
            left: Box::new(left),
            right: Box::new(right),
            span: Span::new(start_span.start, cursor.span().end),
        };
    }

    Ok(left)
}

/// Parse or expression
fn parse_or(cursor: &mut Cursor, source: &str) -> ParseResult<Expr> {
    let start_span = cursor.span();
    let mut left = parse_and(cursor, source)?;

    while cursor.check(&Token::KwOr) {
        cursor.advance();
        let right = parse_and(cursor, source)?;
        left = Expr::BinOp {
            left: Box::new(left),
            op: BinOp::Or,
            right: Box::new(right),
            span: Span::new(start_span.start, cursor.span().end),
        };
    }

    Ok(left)
}

/// Parse and expression
fn parse_and(cursor: &mut Cursor, source: &str) -> ParseResult<Expr> {
    let start_span = cursor.span();
    let mut left = parse_comparison(cursor, source)?;

    while cursor.check(&Token::KwAnd) {
        cursor.advance();
        let right = parse_comparison(cursor, source)?;
        left = Expr::BinOp {
            left: Box::new(left),
            op: BinOp::And,
            right: Box::new(right),
            span: Span::new(start_span.start, cursor.span().end),
        };
    }

    Ok(left)
}

/// Parse comparison expression
fn parse_comparison(cursor: &mut Cursor, source: &str) -> ParseResult<Expr> {
    let start_span = cursor.span();
    let left = parse_additive(cursor, source)?;

    let op = match cursor.peek() {
        Some(t) => match &t.token {
            Token::Eq | Token::EqEq => {
                cursor.advance();
                BinOp::Eq
            }
            Token::NotEq => {
                cursor.advance();
                BinOp::NotEq
            }
            Token::Lt => {
                cursor.advance();
                BinOp::Lt
            }
            Token::Gt => {
                cursor.advance();
                BinOp::Gt
            }
            Token::LtEq => {
                cursor.advance();
                BinOp::LtEq
            }
            Token::GtEq => {
                cursor.advance();
                BinOp::GtEq
            }
            _ => return Ok(left),
        },
        None => return Ok(left),
    };

    let right = parse_additive(cursor, source)?;

    Ok(Expr::BinOp {
        left: Box::new(left),
        op,
        right: Box::new(right),
        span: Span::new(start_span.start, cursor.span().end),
    })
}

/// Parse additive expression (+, -)
fn parse_additive(cursor: &mut Cursor, source: &str) -> ParseResult<Expr> {
    let start_span = cursor.span();
    let mut left = parse_multiplicative(cursor, source)?;

    while let Some(t) = cursor.peek() {
        let op = match &t.token {
            Token::Plus => {
                cursor.advance();
                BinOp::Add
            }
            Token::Minus => {
                cursor.advance();
                BinOp::Sub
            }
            _ => break,
        };

        let right = parse_multiplicative(cursor, source)?;
        left = Expr::BinOp {
            left: Box::new(left),
            op,
            right: Box::new(right),
            span: Span::new(start_span.start, cursor.span().end),
        };
    }

    Ok(left)
}

/// Parse multiplicative expression (*, /, %)
fn parse_multiplicative(cursor: &mut Cursor, source: &str) -> ParseResult<Expr> {
    let start_span = cursor.span();
    let mut left = parse_unary(cursor, source)?;

    while let Some(t) = cursor.peek() {
        let op = match &t.token {
            Token::Star => {
                cursor.advance();
                BinOp::Mul
            }
            Token::Slash => {
                cursor.advance();
                BinOp::Div
            }
            Token::Percent => {
                cursor.advance();
                BinOp::Mod
            }
            _ => break,
        };

        let right = parse_unary(cursor, source)?;
        left = Expr::BinOp {
            left: Box::new(left),
            op,
            right: Box::new(right),
            span: Span::new(start_span.start, cursor.span().end),
        };
    }

    Ok(left)
}

/// Parse unary expression (not, -)
fn parse_unary(cursor: &mut Cursor, source: &str) -> ParseResult<Expr> {
    let start_span = cursor.span();

    if cursor.check(&Token::KwNot) {
        cursor.advance();
        let expr = parse_unary(cursor, source)?;
        return Ok(Expr::Not {
            expr: Box::new(expr),
            span: Span::new(start_span.start, cursor.span().end),
        });
    }

    if cursor.check(&Token::Minus) {
        cursor.advance();
        let expr = parse_unary(cursor, source)?;
        return Ok(Expr::Neg {
            expr: Box::new(expr),
            span: Span::new(start_span.start, cursor.span().end),
        });
    }

    if cursor.check(&Token::KwWait) {
        cursor.advance();
        let expr = parse_unary(cursor, source)?;
        return Ok(Expr::Wait {
            expr: Box::new(expr),
            span: Span::new(start_span.start, cursor.span().end),
        });
    }

    if cursor.check(&Token::KwTry) {
        cursor.advance();
        let expr = parse_unary(cursor, source)?;
        return Ok(Expr::Try {
            expr: Box::new(expr),
            span: Span::new(start_span.start, cursor.span().end),
        });
    }

    parse_primary(cursor, source)
}

/// Parse primary expression
fn parse_primary(cursor: &mut Cursor, source: &str) -> ParseResult<Expr> {
    let start_span = cursor.span();

    match cursor.peek() {
        Some(t) => match &t.token {
            // Literals
            Token::LitNum(Some(n)) => {
                let n = *n;
                cursor.advance();
                Ok(Expr::Num(n, Span::new(start_span.start, cursor.span().end)))
            }
            Token::LitDec(Some(d)) => {
                let d = *d;
                cursor.advance();
                Ok(Expr::Dec(d, Span::new(start_span.start, cursor.span().end)))
            }
            Token::LitStr(s) => {
                let s = s.clone();
                cursor.advance();
                // Parse string for interpolation
                let parts =
                    parse_string_interpolation(&s, Span::new(start_span.start, cursor.span().end));
                Ok(Expr::Text(
                    parts,
                    Span::new(start_span.start, cursor.span().end),
                ))
            }
            Token::LitStrRaw(s) => {
                let s = s.clone();
                cursor.advance();
                Ok(Expr::Text(
                    vec![TextPart::Literal(s)],
                    Span::new(start_span.start, cursor.span().end),
                ))
            }
            Token::LitYes => {
                cursor.advance();
                Ok(Expr::Bool(
                    true,
                    Span::new(start_span.start, cursor.span().end),
                ))
            }
            Token::LitNo => {
                cursor.advance();
                Ok(Expr::Bool(
                    false,
                    Span::new(start_span.start, cursor.span().end),
                ))
            }
            Token::KwNothing => {
                cursor.advance();
                Ok(Expr::Nothing(Span::new(
                    start_span.start,
                    cursor.span().end,
                )))
            }
            Token::LitColor(c) => {
                let c = c.clone();
                cursor.advance();
                Ok(Expr::Color(
                    c,
                    Span::new(start_span.start, cursor.span().end),
                ))
            }

            // Identifier or function call
            Token::Ident(name) => {
                let name = name.clone();
                cursor.advance();

                // Check for struct construction: Type { field: value }
                if cursor.check(&Token::LBrace) {
                    cursor.advance();
                    let fields = parse_field_assigns(cursor, source)?;
                    expect_token(cursor, source, Token::RBrace)?;
                    return Ok(Expr::Construct {
                        name,
                        fields,
                        span: Span::new(start_span.start, cursor.span().end),
                    });
                }

                // Check for function call: func(args) or func arg (without parens)
                if cursor.check(&Token::LParen) {
                    cursor.advance();
                    let args = parse_expr_list(cursor, source, Token::RParen)?;
                    expect_token(cursor, source, Token::RParen)?;
                    return Ok(Expr::Call {
                        func: Box::new(Expr::Ident(
                            name,
                            Span::new(start_span.start, start_span.end),
                        )),
                        args,
                        span: Span::new(start_span.start, cursor.span().end),
                    });
                }

                // Check for function call without parentheses: func arg
                // Exclude Minus so `count - 1` parses as subtraction, not `count(-1)`.
                if cursor.check_expr_start() && !cursor.check(&Token::Minus) {
                    let arg = parse_expr(cursor, source)?;
                    return Ok(Expr::Call {
                        func: Box::new(Expr::Ident(
                            name,
                            Span::new(start_span.start, start_span.end),
                        )),
                        args: vec![arg],
                        span: Span::new(start_span.start, cursor.span().end),
                    });
                }

                // Just an identifier
                Ok(Expr::Ident(
                    name,
                    Span::new(start_span.start, cursor.span().end),
                ))
            }

            // Grouped expression
            Token::LParen => {
                cursor.advance();
                let expr = parse_expr(cursor, source)?;
                expect_token(cursor, source, Token::RParen)?;
                Ok(expr)
            }

            // List literal
            Token::LBracket => {
                cursor.advance();
                let elems = parse_expr_list(cursor, source, Token::RBracket)?;
                expect_token(cursor, source, Token::RBracket)?;
                Ok(Expr::List(
                    elems,
                    Span::new(start_span.start, cursor.span().end),
                ))
            }

            // Fetch expression: fetch "url"  or  fetch get "url"
            Token::KwFetch => {
                cursor.advance();
                let method = if cursor.check(&Token::HttpGet) {
                    cursor.advance();
                    HttpMethod::Get
                } else if cursor.check(&Token::HttpPost) {
                    cursor.advance();
                    HttpMethod::Post
                } else if cursor.check(&Token::HttpPut) {
                    cursor.advance();
                    HttpMethod::Put
                } else if cursor.check(&Token::HttpPatch) {
                    cursor.advance();
                    HttpMethod::Patch
                } else if cursor.check(&Token::HttpDel) {
                    cursor.advance();
                    HttpMethod::Del
                } else {
                    HttpMethod::Get
                };
                let url = parse_expr(cursor, source)?;
                Ok(Expr::Fetch(FetchExpr {
                    method,
                    url: Box::new(url),
                    body: None,
                    ok_arm: None,
                    fail_arm: None,
                    span: Span::new(start_span.start, cursor.span().end),
                }))
            }

            // Lambda expression: x => x * 2
            // or: (x, y) => x + y
            _ if cursor.check(&Token::LParen) || cursor.is_ident() => {
                // Try to parse as lambda
                parse_lambda(cursor, source)
            }

            _ => Err(make_error(
                cursor,
                source,
                &format!("Unexpected token in expression: {}", t.token),
            )),
        },
        None => Err(make_error(cursor, source, "Unexpected end of expr ")),
    }
}

/// Parse string interpolation: "Hello {name}!"
fn parse_string_interpolation(s: &str, span: Span) -> Vec<TextPart> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut chars = s.chars().peekable();

    // Skip surrounding quotes
    if chars.peek() == Some(&'"') {
        chars.next();
    }

    let mut in_interpolation = false;
    let mut brace_depth = 0;
    let mut interp_content = String::new();

    while let Some(c) = chars.next() {
        if in_interpolation {
            if c == '{' {
                brace_depth += 1;
                interp_content.push(c);
            } else if c == '}' {
                if brace_depth > 0 {
                    brace_depth -= 1;
                    interp_content.push(c);
                } else {
                    // End of interpolation
                    in_interpolation = false;
                    if !current.is_empty() {
                        parts.push(TextPart::Literal(current.clone()));
                        current.clear();
                    }
                    // Create expression from interpolation content
                    // For now, treat as identifier or simple expression
                    let expr = parse_simple_interp_expr(&interp_content, span);
                    parts.push(TextPart::Interp(Box::new(expr)));
                    interp_content.clear();
                }
            } else {
                interp_content.push(c);
            }
        } else {
            if c == '{' {
                if chars.peek() == Some(&'{') {
                    // `{{` escape — consume both and emit a literal `{`
                    chars.next();
                    current.push('{');
                } else {
                    in_interpolation = true;
                    if !current.is_empty() {
                        parts.push(TextPart::Literal(current.clone()));
                        current.clear();
                    }
                }
            } else if c == '}' && chars.peek() == Some(&'}') {
                // `}}` escape — consume both and emit a literal `}`
                chars.next();
                current.push('}');
            } else {
                current.push(c);
            }
        }
    }

    // Handle trailing literal
    if !current.is_empty() {
        // Remove trailing quote if present
        if current.ends_with('"') && !current.ends_with(r#"\""#) {
            current.pop();
        }
        parts.push(TextPart::Literal(current));
    }

    if parts.is_empty() {
        parts.push(TextPart::Literal(String::new()));
    }

    parts
}

/// Parse a simple expression from string interpolation content
/// Handles identifiers and basic field access like "obj.field"
fn parse_simple_interp_expr(content: &str, span: Span) -> Expr {
    let content = content.trim();

    // Check for field access: obj.field
    if let Some(dot_pos) = content.find('.') {
        let obj_name = &content[..dot_pos];
        let field_name = &content[dot_pos + 1..];
        return Expr::Field {
            obj: Box::new(Expr::Ident(obj_name.to_string(), span)),
            field: field_name.to_string(),
            span,
        };
    }

    // Simple identifier
    Expr::Ident(content.to_string(), span)
}

/// Parse lambda expression
fn parse_lambda(cursor: &mut Cursor, source: &str) -> ParseResult<Expr> {
    let start_span = cursor.span();

    // Try to parse parameters
    let mut params = Vec::new();

    if cursor.check(&Token::LParen) {
        cursor.advance();
        while cursor.is_ident() {
            params.push(expect_ident(cursor, source)?);
            if cursor.check(&Token::Comma) {
                cursor.advance();
            } else {
                break;
            }
        }
        expect_token(cursor, source, Token::RParen)?;
    } else if cursor.is_ident() {
        params.push(expect_ident(cursor, source)?);
    }

    // Expect =>
    expect_token(cursor, source, Token::FatArrow)?;

    // Parse body
    let body = parse_expr(cursor, source)?;

    Ok(Expr::Lambda {
        params,
        body: Box::new(body),
        span: Span::new(start_span.start, cursor.span().end),
    })
}

/// Parse field assignments for struct construction
fn parse_field_assigns(cursor: &mut Cursor, source: &str) -> ParseResult<Vec<(String, Expr)>> {
    let mut fields = Vec::new();

    while cursor.is_ident() {
        let name = expect_ident(cursor, source)?;
        expect_token(cursor, source, Token::Colon)?;
        let value = parse_expr(cursor, source)?;
        fields.push((name, value));

        if cursor.check(&Token::Comma) {
            cursor.advance();
        } else {
            break;
        }
    }

    Ok(fields)
}

/// Parse a list of expressions separated by delimiter
fn parse_expr_list(cursor: &mut Cursor, source: &str, end_token: Token) -> ParseResult<Vec<Expr>> {
    let mut exprs = Vec::new();

    if cursor.check(&end_token) {
        return Ok(exprs);
    }

    loop {
        let expr = parse_expr(cursor, source)?;
        exprs.push(expr);

        if cursor.check(&Token::Comma) {
            cursor.advance();
        } else {
            break;
        }
    }

    Ok(exprs)
}

/// === Screen/Piece Parsing ===
fn parse_screen(cursor: &mut Cursor, source: &str) -> ParseResult<Item> {
    let start_span = cursor.span();
    expect_token(cursor, source, Token::KwScreen)?;

    let name = expect_ident(cursor, source)?;
    cursor.skip_newlines();

    // Consume the initial indent for the screen body
    let _indent_level = cursor
        .expect_indent()
        .map_err(|msg| make_error(cursor, source, &msg))?;
    let screen_body_start = cursor.span().start;
    let mut screen_body_end = source.len();

    let mut state = Vec::new();
    let mut on_load = None;
    let mut children = Vec::new();
    let mut thinks = Vec::new();
    let mut ghost = false;
    let mut intents = Vec::new();
    let mut stream = None;

    loop {
        // Skip newlines and indentation at screen body level
        cursor.skip_newlines();

        // If we see a dedent, it means we're exiting the screen body
        if cursor.check(&Token::Dedent) {
            screen_body_end = cursor.span().start;
            cursor.advance();
            break;
        }

        // End of file
        if cursor.is_eof() {
            break;
        }

        // Check for top-level keywords that would end the screen
        if cursor.check(&Token::KwShape)
            || cursor.check(&Token::KwThink)
            || cursor.check(&Token::KwScreen)
            || cursor.check(&Token::KwPiece)
            || cursor.check(&Token::KwRoutes)
            || cursor.check(&Token::KwServe)
            || cursor.check(&Token::KwDatabase)
            || cursor.check(&Token::KwTask)
            || cursor.check(&Token::KwQueue)
            || cursor.check(&Token::KwLive)
            || cursor.check(&Token::KwAuth)
        {
            screen_body_end = cursor.span().start;
            break;
        }

        match cursor.peek() {
            Some(t) => match &t.token {
                Token::KwRemember => {
                    let decl = parse_remember(cursor, source)?;
                    state.push(decl);
                }
                Token::KwThink => {
                    let think = parse_think(cursor, source)?;
                    if let Item::Think(t) = think {
                        thinks.push(t);
                    }
                }
                Token::KwGhost => {
                    cursor.advance();
                    ghost = parse_ghost_mode(cursor);
                }
                Token::KwStream => {
                    cursor.advance();
                    expect_token(cursor, source, Token::Colon)?;
                    stream = Some(parse_bool_like(cursor));
                }
                Token::Comment => {
                    if let Some(text) = cursor.current_text() {
                        let trimmed = text.trim().trim_start_matches("--").trim();
                        if !trimmed.is_empty() {
                            intents.push(trimmed.to_string());
                        }
                    }
                    cursor.advance();
                }
                Token::KwBox
                | Token::KwWords
                | Token::KwTap
                | Token::KwEach
                | Token::KwShow
                | Token::At => {
                    let node = parse_ui_node(cursor, source)?;
                    children.push(node);
                }
                Token::KwOn => {
                    cursor.advance();
                    if cursor.check(&Token::KwLoad) {
                        cursor.advance();
                        expect_token(cursor, source, Token::FatArrow)?;
                        on_load = Some(parse_indented_block(cursor, source, |c, s| {
                            parse_stmt(c, s)
                        })?);
                    }
                }
                Token::Indent(_) => {
                    // Skip indentation tokens within the body
                    cursor.advance();
                }
                _ => {
                    break;
                }
            },
            None => break,
        }
    }

    Ok(Item::Screen(ScreenDef {
        name,
        state,
        on_load,
        children,
        thinks,
        ghost,
        intents: if ghost && intents.is_empty() {
            extract_intents_from_source(source, screen_body_start, screen_body_end)
        } else {
            intents
        },
        stream,
        span: Span::new(start_span.start, cursor.span().end),
    }))
}

fn extract_intents_from_source(source: &str, start: usize, end: usize) -> Vec<String> {
    if start >= end || end > source.len() {
        return Vec::new();
    }

    source[start..end]
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("--") {
                let text = trimmed.trim_start_matches("--").trim();
                if text.is_empty() {
                    None
                } else {
                    Some(text.to_string())
                }
            } else {
                None
            }
        })
        .collect()
}

fn parse_ghost_mode(cursor: &mut Cursor) -> bool {
    if cursor.check(&Token::Colon) {
        cursor.advance();
    }

    if cursor.check(&Token::KwOn) {
        cursor.advance();
        return true;
    }

    if cursor.check_ident("on") {
        cursor.advance();
        return true;
    }

    if cursor.check(&Token::LitYes) {
        cursor.advance();
        return true;
    }

    if cursor.check(&Token::LitNo) {
        cursor.advance();
        return false;
    }

    false
}

fn parse_remember(cursor: &mut Cursor, source: &str) -> ParseResult<StateDecl> {
    let start_span = cursor.span();
    expect_token(cursor, source, Token::KwRemember)?;

    let name = expect_ident(cursor, source)?;

    let ty = if cursor.check(&Token::Colon) {
        cursor.advance();
        Some(parse_type(cursor, source)?)
    } else {
        None
    };

    expect_token(cursor, source, Token::Eq)?;

    let default = parse_expr(cursor, source)?;

    // Consume optional reactive options block: when: [...], cache: 1min
    cursor.skip_newlines();
    if cursor.check_indent().is_some() {
        cursor.advance(); // consume Indent token
        loop {
            cursor.skip_newlines();
            if cursor.check(&Token::Dedent) {
                cursor.advance();
                break;
            }
            if cursor.is_eof() || cursor.check_end_of_block() {
                break;
            }
            // Check for same-level continuation indent
            if cursor.check_indent().is_some() {
                cursor.advance();
                continue;
            }
            cursor.advance(); // consume option tokens (when:, cache:, values, etc.)
        }
    }

    Ok(StateDecl {
        name,
        ty,
        default,
        span: Span::new(start_span.start, cursor.span().end),
    })
}

fn parse_piece(cursor: &mut Cursor, source: &str) -> ParseResult<Item> {
    let start_span = cursor.span();
    expect_token(cursor, source, Token::KwPiece)?;

    let name = expect_ident(cursor, source)?;

    let needs = if cursor.check(&Token::KwNeeds) {
        cursor.advance();
        parse_params(cursor, source)?
    } else {
        Vec::new()
    };

    cursor.skip_newlines();

    let mut children = Vec::new();
    let mut thinks = Vec::new();

    while cursor.check(&Token::KwBox)
        || cursor.check(&Token::KwWords)
        || cursor.check(&Token::KwTap)
        || cursor.check(&Token::KwThink)
    {
        match cursor.peek() {
            Some(t) => match &t.token {
                Token::KwBox | Token::KwWords | Token::KwTap => {
                    let node = parse_ui_node(cursor, source)?;
                    children.push(node);
                }
                Token::KwThink => {
                    let think = parse_think(cursor, source)?;
                    if let Item::Think(t) = think {
                        thinks.push(t);
                    }
                }
                _ => break,
            },
            None => break,
        }
    }

    Ok(Item::Piece(PieceDef {
        name,
        needs,
        children,
        thinks,
        span: Span::new(start_span.start, cursor.span().end),
    }))
}

fn parse_ui_node(cursor: &mut Cursor, source: &str) -> ParseResult<UiNode> {
    // Skip any indent tokens at the beginning
    while cursor.check_indent().is_some() {
        cursor.advance();
    }

    let lazy = parse_lazy_annotation(cursor, source)?;

    match cursor.peek() {
        Some(t) => match &t.token {
            Token::KwBox => parse_box(cursor, source, lazy),
            Token::KwWords => parse_words(cursor, source),
            Token::KwTap => parse_tap(cursor, source),
            Token::KwEach => parse_each_node(cursor, source),
            Token::KwShow => parse_show_when(cursor, source),
            Token::Ident(_) => {
                // Potential piece call
                let node_start = t.span.start;
                let name = expect_ident(cursor, source)?;
                // Check if it's a piece call or just a mistake
                Ok(UiNode::Piece(PieceCall {
                    name,
                    props: Vec::new(), // piece props parsed at call site if present
                    span: Span::new(node_start, cursor.span().end),
                }))
            }
            _ => Err(make_error(
                cursor,
                source,
                &format!("Expected UI component, found {:?}", t.token),
            )),
        },
        None => Err(make_error(cursor, source, "Unexpected end ")),
    }
}

fn parse_lazy_annotation(cursor: &mut Cursor, source: &str) -> ParseResult<bool> {
    if !cursor.check(&Token::At) {
        return Ok(false);
    }

    cursor.advance();
    if cursor.check(&Token::KwLazy) {
        cursor.advance();
        cursor.skip_newlines();
        Ok(true)
    } else {
        Err(make_error(cursor, source, "Expected 'lazy' after '@'"))
    }
}

fn parse_box(cursor: &mut Cursor, source: &str, lazy: bool) -> ParseResult<UiNode> {
    let start_span = cursor.span();
    expect_token(cursor, source, Token::KwBox)?;

    let name = if cursor.is_ident() {
        Some(expect_ident(cursor, source)?)
    } else {
        None
    };

    let mut props = BoxProps::default();
    let events = Vec::new();

    // Parse inline props on same line as box declaration
    while !cursor.check(&Token::Newline) && !cursor.is_eof() {
        match cursor.peek() {
            Some(t) => match &t.token {
                Token::KwFill => {
                    cursor.advance();
                    props.fill = Some(parse_expr(cursor, source)?);
                }
                Token::KwCenter => {
                    cursor.advance();
                    props.center = Some(parse_center_dir(cursor, source)?);
                }
                Token::ValLayer => {
                    cursor.advance();
                    if cursor.check(&Token::Colon) {
                        cursor.advance();
                        props.layer = Some(parse_expr(cursor, source)?);
                    }
                }
                Token::KwBackdrop => {
                    cursor.advance();
                    if cursor.check(&Token::Colon) {
                        cursor.advance();
                        if cursor.check(&Token::KwBlur) {
                            cursor.advance();
                        }
                        props.backdrop = Some(parse_expr(cursor, source)?);
                    }
                }
                Token::KwHover => {
                    cursor.advance();
                    if cursor.check(&Token::Colon) {
                        cursor.advance();
                    }
                    props.hover = Some(parse_anim_spec(cursor, source)?);
                }
                Token::KwFlow => {
                    cursor.advance();
                    // Parse flow direction
                }
                _ => {
                    cursor.advance();
                }
            },
            None => break,
        }
    }

    cursor.skip_newlines();

    // Check if there's an indented body with properties and/or children
    if let Some(indent_level) = cursor.check_indent() {
        cursor.advance(); // consume indent

        let mut children = Vec::new();

        loop {
            // Skip newlines and same-level indents within the body
            cursor.skip_newlines();
            if let Some(indent) = cursor.check_indent() {
                if indent == indent_level {
                    cursor.advance();
                } else if indent < indent_level {
                    break; // dedented, end of body
                }
            }

            if cursor.check(&Token::Dedent) || cursor.check_end_of_block() || cursor.is_eof() {
                break;
            }

            match cursor.peek() {
                Some(t) => match &t.token {
                    // Parse box properties
                    Token::KwFill => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        // Handle color keywords or expressions
                        if let Some(t) = cursor.peek() {
                            match &t.token {
                                Token::ValWhite => {
                                    cursor.advance();
                                    props.fill =
                                        Some(Expr::Color("white".to_string(), Span::new(0, 0)));
                                }
                                Token::ValBlack => {
                                    cursor.advance();
                                    props.fill =
                                        Some(Expr::Color("black".to_string(), Span::new(0, 0)));
                                }
                                Token::ValRed => {
                                    cursor.advance();
                                    props.fill =
                                        Some(Expr::Color("red".to_string(), Span::new(0, 0)));
                                }
                                Token::ValGreen => {
                                    cursor.advance();
                                    props.fill =
                                        Some(Expr::Color("green".to_string(), Span::new(0, 0)));
                                }
                                Token::ValBlue => {
                                    cursor.advance();
                                    props.fill =
                                        Some(Expr::Color("blue".to_string(), Span::new(0, 0)));
                                }
                                _ => {
                                    props.fill = Some(parse_expr(cursor, source)?);
                                }
                            }
                        }
                    }
                    Token::KwTall => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        props.tall = Some(parse_expr(cursor, source)?);
                    }
                    Token::KwFlow => {
                        cursor.advance();
                        if cursor.check(&Token::Colon) {
                            cursor.advance();
                        }
                        if let Some(t) = cursor.peek() {
                            match &t.token {
                                Token::ValDown => {
                                    cursor.advance();
                                    props.flow = Some(FlowDir::Down);
                                }
                                Token::ValAcross => {
                                    cursor.advance();
                                    props.flow = Some(FlowDir::Across);
                                }
                                Token::ValWrap => {
                                    cursor.advance();
                                    props.flow = Some(FlowDir::Wrap);
                                }
                                Token::ValLayer => {
                                    cursor.advance();
                                    props.flow = Some(FlowDir::Layer);
                                }
                                _ => {}
                            }
                        }
                    }
                    Token::KwGap => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        props.gap = Some(parse_expr(cursor, source)?);
                    }
                    Token::KwCenter => {
                        cursor.advance();
                        props.center = Some(parse_center_dir(cursor, source)?);
                    }
                    Token::ValLayer => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        props.layer = Some(parse_expr(cursor, source)?);
                    }
                    Token::KwBackdrop => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        if cursor.check(&Token::KwBlur) {
                            cursor.advance();
                        }
                        props.backdrop = Some(parse_expr(cursor, source)?);
                    }
                    Token::KwPadding => {
                        cursor.advance();
                        // padding value parsing not yet supported; consume optional expr
                        if cursor.peek().is_some_and(|t| t.token.is_expr_start()) {
                            let _ = parse_expr(cursor, source)?;
                        }
                    }
                    Token::KwColor => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        if let Some(t) = cursor.peek() {
                            match &t.token {
                                Token::LitColor(c) => {
                                    let c = c.clone();
                                    cursor.advance();
                                    props.color = Some(ColorExpr::Hex(c));
                                }
                                Token::ValWhite => {
                                    cursor.advance();
                                    props.color = Some(ColorExpr::Named("white".to_string()));
                                }
                                Token::ValBlack => {
                                    cursor.advance();
                                    props.color = Some(ColorExpr::Named("black".to_string()));
                                }
                                Token::ValRed => {
                                    cursor.advance();
                                    props.color = Some(ColorExpr::Named("red".to_string()));
                                }
                                Token::ValGreen => {
                                    cursor.advance();
                                    props.color = Some(ColorExpr::Named("green".to_string()));
                                }
                                Token::ValBlue => {
                                    cursor.advance();
                                    props.color = Some(ColorExpr::Named("blue".to_string()));
                                }
                                _ => {
                                    // Try to parse as expression
                                    if let Ok(_expr) = parse_expr(cursor, source) {
                                        // For now, just skip - proper handling would evaluate to color
                                    }
                                }
                            }
                        }
                    }
                    Token::KwRadius => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        props.radius = Some(parse_expr(cursor, source)?);
                    }
                    Token::KwSnap => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        if cursor.check(&Token::LitYes) {
                            cursor.advance();
                            props.snap = Some(true);
                        } else {
                            cursor.advance();
                            props.snap = Some(false);
                        }
                    }
                    Token::KwAnimate => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        props.animate = Some(parse_anim_spec(cursor, source)?);
                    }
                    Token::KwPan => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        props.gesture = Some(parse_gesture(cursor, source)?);
                    }
                    Token::KwEnter => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        props.enter = Some(parse_anim_spec(cursor, source)?);
                    }
                    Token::KwLeave => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        props.leave = Some(parse_anim_spec(cursor, source)?);
                    }
                    Token::KwHover => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        props.hover = Some(parse_anim_spec(cursor, source)?);
                    }
                    // Child UI nodes
                    Token::KwBox
                    | Token::KwWords
                    | Token::KwTap
                    | Token::KwEach
                    | Token::KwShow
                    | Token::At => {
                        let node = parse_ui_node(cursor, source)?;
                        children.push(node);
                    }
                    Token::FatArrow => {
                        // Action arrow for tap nodes - this is handled in parse_tap
                        // but if we see it here, we need to skip it as we're not in a tap context
                        cursor.advance();
                    }
                    Token::Dedent => {
                        cursor.advance();
                        break;
                    }
                    _ => {
                        // Unknown token, skip it
                        cursor.advance();
                    }
                },
                None => break,
            }

            cursor.skip_newlines();
        }

        // Consume any trailing dedent
        while cursor.check(&Token::Dedent) {
            cursor.advance();
        }

        Ok(UiNode::Box(BoxNode {
            name,
            props,
            events,
            children,
            lazy,
            span: Span::new(start_span.start, cursor.span().end),
        }))
    } else {
        // No body, just return the box with inline props
        Ok(UiNode::Box(BoxNode {
            name,
            props,
            events,
            children: Vec::new(),
            lazy,
            span: Span::new(start_span.start, cursor.span().end),
        }))
    }
}

fn parse_center_dir(cursor: &mut Cursor, source: &str) -> ParseResult<CenterDir> {
    if cursor.check(&Token::Colon) {
        cursor.advance();
    }

    if let Some(token) = cursor.peek() {
        match token.token {
            Token::ValBoth => {
                cursor.advance();
                Ok(CenterDir::Both)
            }
            Token::ValAcross => {
                cursor.advance();
                Ok(CenterDir::Across)
            }
            Token::ValDown => {
                cursor.advance();
                Ok(CenterDir::Down)
            }
            _ => Ok(CenterDir::Both),
        }
    } else {
        Err(make_error(
            cursor,
            source,
            "Unexpected end while parsing center direction",
        ))
    }
}

fn parse_words(cursor: &mut Cursor, source: &str) -> ParseResult<UiNode> {
    let start_span = cursor.span();
    expect_token(cursor, source, Token::KwWords)?;

    let content = parse_expr(cursor, source)?;
    let mut props = TextProps::default();
    let events = Vec::new();

    cursor.skip_newlines();
    if let Some(indent_level) = cursor.check_indent() {
        cursor.advance(); // consume indent

        loop {
            cursor.skip_newlines();

            if cursor.check(&Token::Dedent) {
                cursor.advance();
                break;
            }
            if cursor.check_end_of_block() || cursor.is_eof() {
                break;
            }

            if let Some(lvl) = cursor.check_indent() {
                if lvl == indent_level {
                    cursor.advance();
                } else {
                    break;
                }
            }

            match cursor.peek() {
                Some(t) => match t.token.clone() {
                    Token::KwSize => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        props.size = Some(parse_expr(cursor, source)?);
                    }
                    Token::KwWeight => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        if let Some(t) = cursor.peek() {
                            props.weight = match &t.token {
                                Token::ValFat => Some(TextWeight::Fat),
                                Token::ValThin => Some(TextWeight::Thin),
                                Token::ValNormal => Some(TextWeight::Normal),
                                _ => None,
                            };
                            cursor.advance();
                        }
                    }
                    Token::KwColor => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        if let Some(t) = cursor.peek() {
                            if let Token::LitColor(c) = &t.token {
                                let c = c.clone();
                                cursor.advance();
                                props.color = Some(ColorExpr::Hex(c));
                            } else {
                                cursor.advance();
                            }
                        }
                    }
                    Token::KwAlign => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        if let Some(t) = cursor.peek() {
                            props.align = match &t.token {
                                Token::KwCenter => Some(TextAlign::Center),
                                Token::ValRight => Some(TextAlign::Right),
                                _ => Some(TextAlign::Left),
                            };
                            cursor.advance();
                        }
                    }
                    Token::KwSpacing => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        props.spacing = Some(parse_expr(cursor, source)?);
                    }
                    Token::KwLines => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        props.lines = Some(parse_expr(cursor, source)?);
                    }
                    Token::Dedent => {
                        cursor.advance();
                        break;
                    }
                    _ => {
                        cursor.advance();
                    }
                },
                None => break,
            }
        }
    }

    Ok(UiNode::Words(WordsNode {
        content,
        props,
        events,
        span: Span::new(start_span.start, cursor.span().end),
    }))
}

fn parse_tap(cursor: &mut Cursor, source: &str) -> ParseResult<UiNode> {
    let start_span = cursor.span();
    expect_token(cursor, source, Token::KwTap)?;

    let label = parse_expr(cursor, source)?;

    let mut props = BoxProps::default();
    let mut guard = None;
    let mut confirm = false;

    cursor.skip_newlines();

    // Check if there are indented properties
    let indent_level = match cursor.check_indent() {
        Some(level) => {
            cursor.advance(); // consume indent
            level
        }
        None => {
            // No body, just return tap with label
            return Ok(UiNode::Tap(TapNode {
                label,
                action: Vec::new(),
                guard,
                confirm,
                props,
                span: Span::new(start_span.start, cursor.span().end),
            }));
        }
    };

    let mut action = Vec::new();

    loop {
        cursor.skip_newlines();

        // Check for end of block
        if cursor.check(&Token::Dedent) {
            cursor.advance();
            break;
        }
        if cursor.check_end_of_block() || cursor.is_eof() {
            break;
        }

        // Check for same-level indent (continuation)
        if let Some(indent) = cursor.check_indent() {
            if indent == indent_level {
                cursor.advance();
            } else if indent < indent_level {
                break;
            }
        }

        match cursor.peek() {
            Some(t) => {
                match &t.token {
                    // Parse tap properties
                    Token::KwFill => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        props.fill = Some(parse_expr(cursor, source)?);
                    }
                    Token::KwColor => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        if let Some(t) = cursor.peek() {
                            if let Token::LitColor(c) = &t.token {
                                let c = c.clone();
                                cursor.advance();
                                props.color = Some(ColorExpr::Hex(c));
                            }
                        }
                    }
                    Token::KwRadius => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        props.radius = Some(parse_expr(cursor, source)?);
                    }
                    Token::KwPadding => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        // padding: <v> <h>  or  padding: <all>
                        let v = if let Some(t) = cursor.peek() {
                            if let Token::LitNum(Some(n)) = &t.token {
                                let n = *n;
                                cursor.advance();
                                n
                            } else {
                                0
                            }
                        } else {
                            0
                        };
                        let h = if let Some(t) = cursor.peek() {
                            if let Token::LitNum(Some(n)) = &t.token {
                                let n = *n;
                                cursor.advance();
                                n
                            } else {
                                v
                            }
                        } else {
                            v
                        };
                        props.padding = Some(crate::ast::PaddingExpr {
                            top: v,
                            right: h,
                            bottom: v,
                            left: h,
                        });
                    }
                    Token::KwHover => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        props.hover = Some(parse_anim_spec(cursor, source)?);
                    }
                    Token::KwEnter => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        props.enter = Some(parse_anim_spec(cursor, source)?);
                    }
                    Token::KwLeave => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        props.leave = Some(parse_anim_spec(cursor, source)?);
                    }
                    Token::KwAnimate => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        props.animate = Some(parse_anim_spec(cursor, source)?);
                    }
                    Token::KwGuard => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        guard = Some(parse_expr(cursor, source)?);
                    }
                    Token::KwConfirm => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        confirm = parse_bool_like(cursor);
                    }
                    // Action arrow - parse the action statements
                    Token::FatArrow => {
                        cursor.advance();
                        cursor.skip_newlines();
                        // Multi-line body (=> on own line, stmts indented below)
                        // or single-line body (=> stmt on same line)
                        if cursor.check_indent().is_some() {
                            if let Ok(stmts) = parse_indented_block(cursor, source, parse_stmt) {
                                action.extend(stmts);
                            }
                        } else if let Ok(stmt) = parse_stmt(cursor, source) {
                            action.push(stmt);
                        }
                    }
                    Token::Dedent => {
                        cursor.advance();
                        break;
                    }
                    _ => {
                        // Skip unknown tokens
                        cursor.advance();
                    }
                } // Close inner match
            }
            None => break,
        }
    }

    Ok(UiNode::Tap(TapNode {
        label,
        action,
        guard,
        confirm,
        props,
        span: Span::new(start_span.start, cursor.span().end),
    }))
}

// Removed extra brace that was causing compilation error

/// === Server-side Parsing ===
fn parse_serve(cursor: &mut Cursor, source: &str) -> ParseResult<Item> {
    let start_span = cursor.span();
    expect_token(cursor, source, Token::KwServe)?;

    let name = expect_ident(cursor, source)?;
    cursor.skip_newlines();

    // Consume the initial indent for the serve body
    let _indent_level = cursor
        .expect_indent()
        .map_err(|msg| make_error(cursor, source, &msg))?;

    let mut port = None;
    let mut prefix = None;
    let mut routes = Vec::new();
    let guards = Vec::new();

    loop {
        cursor.skip_newlines();

        // End of serve body
        if cursor.check(&Token::Dedent) || cursor.is_eof() {
            break;
        }

        // Skip indent tokens
        if cursor.check_indent().is_some() {
            cursor.advance();
            continue;
        }

        if !(cursor.check(&Token::KwPort)
            || cursor.check(&Token::KwPrefix)
            || is_http_method(cursor))
        {
            break;
        }
        match cursor.peek() {
            Some(t) => match &t.token {
                Token::KwPort => {
                    cursor.advance();
                    expect_token(cursor, source, Token::Colon)?;
                    if let Some(Token::LitNum(Some(p))) = cursor.peek().map(|t| &t.token) {
                        port = Some(*p as u16);
                        cursor.advance();
                    }
                }
                Token::KwPrefix => {
                    cursor.advance();
                    expect_token(cursor, source, Token::Colon)?;
                    let prefix_str = expect_string(cursor, source)?;
                    prefix = Some(prefix_str);
                }
                _ if is_http_method(cursor) => {
                    let method = parse_http_method(cursor, source)?;
                    let pattern = expect_string(cursor, source)?;
                    expect_token(cursor, source, Token::FatArrow)?;
                    let handler = expect_ident(cursor, source)?;

                    routes.push(ServerRoute {
                        method,
                        pattern,
                        handler,
                        guards: Vec::new(),
                        span: Span::new(start_span.start, cursor.span().end),
                    });
                }
                _ => break,
            },
            None => break,
        }
    }

    Ok(Item::Serve(ServeDef {
        name,
        port,
        prefix,
        routes,
        guards,
        span: Span::new(start_span.start, cursor.span().end),
    }))
}

fn is_http_method(cursor: &Cursor) -> bool {
    matches!(cursor.peek(),
        Some(t) if matches!(t.token,
            Token::HttpGet | Token::HttpPost | Token::HttpPut | Token::HttpPatch | Token::HttpDel
        )
    )
}

fn parse_http_method(cursor: &mut Cursor, source: &str) -> ParseResult<HttpMethod> {
    match cursor.peek() {
        Some(t) => match &t.token {
            Token::HttpGet => {
                cursor.advance();
                Ok(HttpMethod::Get)
            }
            Token::HttpPost => {
                cursor.advance();
                Ok(HttpMethod::Post)
            }
            Token::HttpPut => {
                cursor.advance();
                Ok(HttpMethod::Put)
            }
            Token::HttpPatch => {
                cursor.advance();
                Ok(HttpMethod::Patch)
            }
            Token::HttpDel => {
                cursor.advance();
                Ok(HttpMethod::Del)
            }
            _ => Err(make_error(cursor, source, "Expected HTTP action ")),
        },
        None => Err(make_error(cursor, source, "Unexpected end ")),
    }
}

fn parse_database(cursor: &mut Cursor, source: &str) -> ParseResult<Item> {
    let start_span = cursor.span();
    expect_token(cursor, source, Token::KwDatabase)?;

    let name = expect_ident(cursor, source)?;
    cursor.skip_newlines();

    // Handle dedent from previous block if present
    while cursor.check(&Token::Dedent) {
        cursor.advance();
        cursor.skip_newlines();
    }

    // Consume the initial indent for the database body
    let _indent_level = cursor
        .expect_indent()
        .map_err(|msg| make_error(cursor, source, &msg))?;

    let mut kind = DbKind::Sqlite;
    let mut url = Expr::Text(vec![], Span::new(0, 0));
    let mut pool = None;

    loop {
        cursor.skip_newlines();

        // End of database body
        if cursor.check(&Token::Dedent) || cursor.is_eof() {
            break;
        }

        // Skip indent tokens
        if cursor.check_indent().is_some() {
            cursor.advance();
            continue;
        }

        match cursor.peek() {
            Some(t) => match &t.token {
                Token::KwKind => {
                    cursor.advance();
                    expect_token(cursor, source, Token::Colon)?;
                    if let Some(t) = cursor.peek() {
                        match &t.token {
                            Token::Ident(s) if s == "postgres" => {
                                cursor.advance();
                                kind = DbKind::Postgres;
                            }
                            Token::Ident(s) if s == "sqlite" => {
                                cursor.advance();
                                kind = DbKind::Sqlite;
                            }
                            Token::Ident(s) if s == "mysql" => {
                                cursor.advance();
                                kind = DbKind::Mysql;
                            }
                            _ => {}
                        }
                    }
                }
                Token::KwUrl => {
                    cursor.advance();
                    expect_token(cursor, source, Token::Colon)?;
                    url = parse_expr(cursor, source)?;
                }
                Token::KwPool => {
                    cursor.advance();
                    expect_token(cursor, source, Token::Colon)?;
                    if let Some(Token::LitNum(Some(p))) = cursor.peek().map(|t| &t.token) {
                        pool = Some(*p as u32);
                        cursor.advance();
                    }
                }
                _ => break,
            },
            None => break,
        }
    }

    if cursor.check(&Token::Dedent) {
        cursor.advance();
    }

    Ok(Item::Database(DatabaseDef {
        name,
        kind,
        url,
        pool,
        span: Span::new(start_span.start, cursor.span().end),
    }))
}

fn parse_task(cursor: &mut Cursor, source: &str) -> ParseResult<Item> {
    let start_span = cursor.span();
    expect_token(cursor, source, Token::KwTask)?;

    let name = expect_ident(cursor, source)?;

    let schedule = if cursor.check(&Token::KwEvery) {
        cursor.advance();
        let s = expect_string(cursor, source)?;
        TaskSchedule::Every(s)
    } else {
        TaskSchedule::Every("1h".to_string())
    };

    cursor.skip_newlines();

    let body = parse_indented_block(cursor, source, parse_stmt)?;

    Ok(Item::Task(TaskDef {
        name,
        schedule,
        body,
        span: Span::new(start_span.start, cursor.span().end),
    }))
}

fn parse_queue(cursor: &mut Cursor, source: &str) -> ParseResult<Item> {
    let start_span = cursor.span();
    expect_token(cursor, source, Token::KwQueue)?;

    let name = expect_ident(cursor, source)?;

    Ok(Item::Queue(QueueDef {
        name,
        workers: None,
        retry: None,
        span: Span::new(start_span.start, cursor.span().end),
    }))
}

fn parse_live(cursor: &mut Cursor, source: &str) -> ParseResult<Item> {
    let start_span = cursor.span();
    expect_token(cursor, source, Token::KwLive)?;

    let name = expect_ident(cursor, source)?;
    cursor.skip_newlines();

    let mut path = String::new();
    let mut sync = None;
    let mut presence = false;
    let mut transform = None;

    // Consume the initial indent for the live body if present
    if let Some(_indent) = cursor.check_indent() {
        cursor.advance();

        loop {
            cursor.skip_newlines();
            if cursor.check(&Token::Dedent) || cursor.is_eof() {
                break;
            }

            match cursor.peek() {
                Some(t) => match &t.token {
                    Token::KwPath => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        path = expect_string(cursor, source)?;
                    }
                    Token::KwSync => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        sync = match cursor.peek() {
                            Some(t) => match &t.token {
                                Token::KwAutomatic => {
                                    cursor.advance();
                                    Some(SyncMode::Automatic)
                                }
                                Token::KwManual => {
                                    cursor.advance();
                                    Some(SyncMode::Manual)
                                }
                                Token::Ident(s) if s.eq_ignore_ascii_case("automatic") => {
                                    cursor.advance();
                                    Some(SyncMode::Automatic)
                                }
                                Token::Ident(s) if s.eq_ignore_ascii_case("manual") => {
                                    cursor.advance();
                                    Some(SyncMode::Manual)
                                }
                                _ => {
                                    cursor.advance();
                                    None
                                }
                            },
                            None => None,
                        };
                    }
                    Token::KwPresence => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        presence = parse_bool_like(cursor);
                    }
                    Token::KwTransform => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        if let Some(t) = cursor.peek() {
                            match &t.token {
                                Token::KwLWW => {
                                    cursor.advance();
                                    transform = Some(CrdtType::Lww);
                                }
                                Token::KwPNCounter => {
                                    cursor.advance();
                                    transform = Some(CrdtType::PNCounter);
                                }
                                Token::KwGCounter => {
                                    cursor.advance();
                                    transform = Some(CrdtType::GCounter);
                                }
                                Token::KwMVRegister => {
                                    cursor.advance();
                                    transform = Some(CrdtType::MVRegister);
                                }
                                _ => {
                                    cursor.advance();
                                }
                            }
                        }
                    }
                    _ => {
                        cursor.advance();
                    }
                },
                None => break,
            }
        }

        if cursor.check(&Token::Dedent) {
            cursor.advance();
        }
    }

    Ok(Item::Live(LiveDef {
        name,
        path,
        sync,
        presence,
        transform,
        on_connect: None,
        on_message: None,
        on_leave: None,
        span: Span::new(start_span.start, cursor.span().end),
    }))
}

fn parse_anim_spec(cursor: &mut Cursor, source: &str) -> ParseResult<AnimSpec> {
    parse_animate_block(cursor, source)
}

fn parse_animate_block(cursor: &mut Cursor, source: &str) -> ParseResult<AnimSpec> {
    let mut spec = AnimSpec {
        name: "default".to_string(),
        duration: None,
        ease: None,
        physics: None,
        from: None,
        to: None,
    };

    if cursor.check(&Token::KwSpring) {
        cursor.advance();
        spec.name = "spring".to_string();
        spec.physics = Some(PhysicsSpec {
            kind: PhysicsKind::Spring,
            stiffness: None,
            damping: None,
            mass: None,
        });

        if cursor.check(&Token::LBrace) {
            cursor.advance();
            parse_physics_fields(cursor, source, &mut spec)?;
            expect_token(cursor, source, Token::RBrace)?;
        }

        return Ok(spec);
    }

    if cursor.check(&Token::LBrace) {
        cursor.advance();

        while !cursor.check(&Token::RBrace) && !cursor.is_eof() {
            match cursor.peek() {
                Some(t) => match &t.token {
                    Token::KwPhysics => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        if cursor.check(&Token::KwSpring) {
                            cursor.advance();
                            spec.name = "spring".to_string();
                            spec.physics = Some(PhysicsSpec {
                                kind: PhysicsKind::Spring,
                                stiffness: None,
                                damping: None,
                                mass: None,
                            });
                        }
                    }
                    Token::KwStiffness | Token::KwDamping | Token::KwMass => {
                        parse_physics_field(cursor, source, &mut spec)?;
                    }
                    Token::KwFrom => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        spec.from = Some(parse_anim_transform(cursor, source)?);
                    }
                    Token::KwTo => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        spec.to = Some(parse_anim_transform(cursor, source)?);
                    }
                    Token::KwDuration => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        spec.duration = Some(expect_string(cursor, source)?);
                    }
                    Token::KwEase => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        spec.ease = Some(expect_string(cursor, source)?);
                    }
                    Token::Comma | Token::Newline => {
                        cursor.advance();
                    }
                    _ => {
                        cursor.advance();
                    }
                },
                None => break,
            }
        }

        expect_token(cursor, source, Token::RBrace)?;
        return Ok(spec);
    }

    // Indented block style: hover:\n  scale: 1.05\n  physics: spring
    cursor.skip_newlines();
    if let Some(_indent_level) = cursor.check_indent() {
        cursor.advance(); // consume indent token

        loop {
            cursor.skip_newlines();

            if cursor.check(&Token::Dedent) || cursor.check_end_of_block() || cursor.is_eof() {
                if cursor.check(&Token::Dedent) {
                    cursor.advance();
                }
                break;
            }

            // Same-level indent continuation
            if cursor.check_indent().is_some() {
                cursor.advance();
                continue;
            }

            match cursor.peek() {
                Some(t) => match t.token.clone() {
                    Token::KwScale => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        let s = parse_numeric_value(cursor)?;
                        spec.to = Some(AnimTransform {
                            x: None,
                            y: None,
                            opacity: None,
                            scale: Some(s as f64),
                            rotate: None,
                        });
                    }
                    Token::KwOpacity => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        let v = parse_numeric_value(cursor)?;
                        let to = spec.to.get_or_insert(AnimTransform {
                            x: None,
                            y: None,
                            opacity: None,
                            scale: None,
                            rotate: None,
                        });
                        to.opacity = Some(v as f64);
                    }
                    Token::KwPhysics => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        if cursor.check(&Token::KwSpring) {
                            cursor.advance();
                            spec.name = "spring".to_string();
                            spec.physics = Some(PhysicsSpec {
                                kind: PhysicsKind::Spring,
                                stiffness: None,
                                damping: None,
                                mass: None,
                            });
                        }
                    }
                    Token::KwDuration => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        spec.duration = Some(expect_string(cursor, source)?);
                    }
                    Token::KwEase => {
                        cursor.advance();
                        expect_token(cursor, source, Token::Colon)?;
                        spec.ease = Some(expect_string(cursor, source)?);
                    }
                    Token::KwStiffness | Token::KwDamping | Token::KwMass => {
                        parse_physics_field(cursor, source, &mut spec)?;
                    }
                    _ => {
                        cursor.advance();
                    }
                },
                None => break,
            }
        }
    }

    Ok(spec)
}

fn parse_physics_fields(cursor: &mut Cursor, source: &str, spec: &mut AnimSpec) -> ParseResult<()> {
    while !cursor.check(&Token::RBrace) && !cursor.is_eof() {
        match cursor.peek() {
            Some(t) => match &t.token {
                Token::KwStiffness | Token::KwDamping | Token::KwMass => {
                    parse_physics_field(cursor, source, spec)?;
                }
                Token::Comma | Token::Newline => {
                    cursor.advance();
                }
                _ => {
                    cursor.advance();
                }
            },
            None => break,
        }
    }
    Ok(())
}

fn parse_physics_field(cursor: &mut Cursor, source: &str, spec: &mut AnimSpec) -> ParseResult<()> {
    let field = cursor.peek().map(|t| t.token.clone());
    cursor.advance();
    expect_token(cursor, source, Token::Colon)?;

    let value = parse_numeric_value(cursor)?;
    let physics = spec.physics.get_or_insert(PhysicsSpec {
        kind: PhysicsKind::Spring,
        stiffness: None,
        damping: None,
        mass: None,
    });

    match field {
        Some(Token::KwStiffness) => physics.stiffness = Some(value),
        Some(Token::KwDamping) => physics.damping = Some(value),
        Some(Token::KwMass) => physics.mass = Some(value),
        _ => {}
    }

    Ok(())
}

fn parse_anim_transform(cursor: &mut Cursor, source: &str) -> ParseResult<AnimTransform> {
    expect_token(cursor, source, Token::LBrace)?;

    let mut transform = AnimTransform {
        x: None,
        y: None,
        opacity: None,
        scale: None,
        rotate: None,
    };

    while !cursor.check(&Token::RBrace) && !cursor.is_eof() {
        match cursor.peek() {
            Some(t) => match &t.token {
                Token::Ident(name) if name == "x" || name == "y" => {
                    let axis = name.clone();
                    cursor.advance();
                    expect_token(cursor, source, Token::Colon)?;
                    let val = parse_stringish_value(cursor);
                    if axis == "x" {
                        transform.x = Some(val);
                    } else {
                        transform.y = Some(val);
                    }
                }
                Token::KwOpacity => {
                    cursor.advance();
                    expect_token(cursor, source, Token::Colon)?;
                    transform.opacity = Some(parse_numeric_value(cursor)?);
                }
                Token::KwScale => {
                    cursor.advance();
                    expect_token(cursor, source, Token::Colon)?;
                    transform.scale = Some(parse_numeric_value(cursor)?);
                }
                Token::KwRotate => {
                    cursor.advance();
                    expect_token(cursor, source, Token::Colon)?;
                    transform.rotate = Some(parse_numeric_value(cursor)?);
                }
                Token::Comma | Token::Newline => {
                    cursor.advance();
                }
                _ => {
                    cursor.advance();
                }
            },
            None => break,
        }
    }

    expect_token(cursor, source, Token::RBrace)?;
    Ok(transform)
}

fn parse_gesture(cursor: &mut Cursor, source: &str) -> ParseResult<GestureSpec> {
    let direction = match cursor.peek() {
        Some(t) => match &t.token {
            Token::ValHorizontal => {
                cursor.advance();
                GestureDirection::Horizontal
            }
            Token::ValVertical => {
                cursor.advance();
                GestureDirection::Vertical
            }
            Token::ValBoth => {
                cursor.advance();
                GestureDirection::Both
            }
            _ => GestureDirection::Both,
        },
        None => GestureDirection::Both,
    };

    let mut on_release = None;
    if cursor.check(&Token::LBrace) {
        cursor.advance();
        let mut actions = Vec::new();

        while !cursor.check(&Token::RBrace) && !cursor.is_eof() {
            match cursor.peek() {
                Some(t) => match &t.token {
                    Token::KwOn => {
                        cursor.advance();
                        if cursor.check(&Token::KwRelease) {
                            cursor.advance();
                            expect_token(cursor, source, Token::Colon)?;
                            actions.push(parse_gesture_action(cursor, source)?);
                        }
                    }
                    Token::Comma | Token::Newline => {
                        cursor.advance();
                    }
                    _ => {
                        cursor.advance();
                    }
                },
                None => break,
            }
        }

        expect_token(cursor, source, Token::RBrace)?;
        if !actions.is_empty() {
            on_release = Some(actions);
        }
    }

    Ok(GestureSpec {
        direction,
        on_release,
    })
}

fn parse_gesture_action(cursor: &mut Cursor, _source: &str) -> ParseResult<GestureAction> {
    if cursor.check(&Token::KwSnap) || cursor.check(&Token::KwBack) {
        cursor.advance();
        return Ok(GestureAction::SnapBack);
    }

    if cursor.check_ident("next") || cursor.check_ident("next_slide") {
        cursor.advance();
        let mut velocity_threshold = None;
        let mut offset_threshold = None;

        while !cursor.check(&Token::Comma)
            && !cursor.check(&Token::Newline)
            && !cursor.check(&Token::RBrace)
            && !cursor.is_eof()
        {
            if cursor.check(&Token::KwVelocity) {
                cursor.advance();
                if cursor.check(&Token::Colon) {
                    cursor.advance();
                }
                velocity_threshold = parse_numeric_value(cursor).ok();
                continue;
            }

            if cursor.check(&Token::KwOffset) {
                cursor.advance();
                if cursor.check(&Token::Colon) {
                    cursor.advance();
                }
                offset_threshold = Some(parse_stringish_value(cursor));
                continue;
            }

            cursor.advance();
        }

        return Ok(GestureAction::NextSlide {
            velocity_threshold,
            offset_threshold,
        });
    }

    Ok(GestureAction::SnapBack)
}

fn parse_numeric_value(cursor: &mut Cursor) -> ParseResult<f64> {
    if let Some(t) = cursor.peek() {
        match &t.token {
            Token::LitNum(Some(n)) => {
                let v = *n as f64;
                cursor.advance();
                Ok(v)
            }
            Token::LitDec(Some(d)) => {
                let v = *d;
                cursor.advance();
                Ok(v)
            }
            _ => Err(ParseError {
                message: "Expected numeric literal".to_string(),
                span: SourceSpan::from(t.span.start..t.span.end),
                source_code: String::new(),
            }),
        }
    } else {
        Err(ParseError {
            message: "Expected numeric literal".to_string(),
            span: SourceSpan::from(0..0),
            source_code: String::new(),
        })
    }
}

fn parse_stringish_value(cursor: &mut Cursor) -> String {
    if let Some(t) = cursor.peek() {
        let value = match &t.token {
            Token::LitPercent(s) => s.clone(),
            Token::LitMs(s) => s.clone(),
            Token::LitSecs(s) => s.clone(),
            Token::LitHours(s) => s.clone(),
            Token::LitNum(Some(n)) => n.to_string(),
            Token::LitDec(Some(d)) => d.to_string(),
            Token::LitStr(s) | Token::LitStrRaw(s) => s.clone(),
            Token::Ident(s) => s.clone(),
            _ => t.token.to_string(),
        };
        cursor.advance();
        value
    } else {
        String::new()
    }
}

fn parse_bool_like(cursor: &mut Cursor) -> bool {
    if cursor.check(&Token::LitYes) {
        cursor.advance();
        return true;
    }
    if cursor.check(&Token::LitNo) {
        cursor.advance();
        return false;
    }

    if let Some(t) = cursor.peek() {
        if let Token::Ident(name) = &t.token {
            let v = name.eq_ignore_ascii_case("true") || name.eq_ignore_ascii_case("yes");
            cursor.advance();
            return v;
        }
        cursor.advance();
    }

    false
}

fn parse_each_node(cursor: &mut Cursor, source: &str) -> ParseResult<UiNode> {
    let start_span = cursor.span();
    expect_token(cursor, source, Token::KwEach)?;

    let var = expect_ident(cursor, source)?;

    expect_token(cursor, source, Token::KwIn)?;

    let iter = parse_expr(cursor, source)?;

    cursor.skip_newlines();

    // Parse indented child UI nodes
    let mut children = Vec::new();
    if cursor.check_indent().is_some() {
        cursor.advance(); // consume Indent
        loop {
            cursor.skip_newlines();
            if cursor.check(&Token::Dedent) {
                cursor.advance();
                break;
            }
            if cursor.is_eof() || cursor.check_end_of_block() {
                break;
            }
            if cursor.check_indent().is_some() {
                cursor.advance();
                continue;
            }
            match cursor.peek() {
                Some(t) => match t.token.clone() {
                    Token::KwBox
                    | Token::KwWords
                    | Token::KwTap
                    | Token::KwEach
                    | Token::KwShow => {
                        let node = parse_ui_node(cursor, source)?;
                        children.push(node);
                    }
                    Token::Dedent => {
                        cursor.advance();
                        break;
                    }
                    _ => {
                        cursor.advance();
                    }
                },
                None => break,
            }
        }
    }

    Ok(UiNode::Each(EachNode {
        var,
        iter,
        children,
        span: Span::new(start_span.start, cursor.span().end),
    }))
}

fn parse_show_when(cursor: &mut Cursor, source: &str) -> ParseResult<UiNode> {
    let start_span = cursor.span();
    // `when <expr>` — consume `when` keyword
    if cursor.check(&Token::KwWhen) {
        cursor.advance();
    } else if cursor.check(&Token::KwShow) {
        cursor.advance();
        if cursor.check(&Token::KwWhen) {
            cursor.advance();
        }
    }

    let cond = parse_expr(cursor, source)?;

    cursor.skip_newlines();

    let mut children = Vec::new();
    if cursor.check_indent().is_some() {
        cursor.advance(); // consume Indent
        loop {
            cursor.skip_newlines();
            if cursor.check(&Token::Dedent) || cursor.is_eof() || cursor.check_end_of_block() {
                if cursor.check(&Token::Dedent) {
                    cursor.advance();
                }
                break;
            }
            // Skip extra indent tokens (same-level re-indents)
            while cursor.check_indent().is_some() {
                cursor.advance();
            }
            // Stop if we consumed indents and now see dedent/eof
            if cursor.check(&Token::Dedent) || cursor.is_eof() || cursor.check_end_of_block() {
                if cursor.check(&Token::Dedent) {
                    cursor.advance();
                }
                break;
            }
            // Parse an actual UI child node
            match cursor.peek() {
                Some(t) => match &t.token {
                    Token::KwBox
                    | Token::KwWords
                    | Token::KwTap
                    | Token::KwEach
                    | Token::KwShow
                    | Token::At => {
                        match parse_ui_node(cursor, source) {
                            Ok(node) => children.push(node),
                            Err(_) => { cursor.advance(); }
                        }
                    }
                    _ => { cursor.advance(); }
                },
                None => break,
            }
        }
    }

    Ok(UiNode::ShowWhen(ShowWhenNode {
        cond,
        children,
        span: Span::new(start_span.start, cursor.span().end),
    }))
}

#[allow(dead_code)]
fn parse_show_when_placeholder(cursor: &mut Cursor, _source: &str) -> ParseResult<UiNode> {
    // Placeholder for actual show node parsing
    Err(make_error(
        cursor,
        _source,
        "Show node parsing not fully implemented in this phase",
    ))
}

fn parse_auth(cursor: &mut Cursor, source: &str) -> ParseResult<Item> {
    let start_span = cursor.span();
    expect_token(cursor, source, Token::KwAuth)?;

    let kind = match cursor.peek() {
        Some(t) => match &t.token {
            Token::Ident(s) if s == "jwt" => {
                cursor.advance();
                AuthKind::Jwt
            }
            Token::Ident(s) if s == "basic" => {
                cursor.advance();
                AuthKind::Basic
            }
            Token::Ident(s) if s == "oauth" => {
                cursor.advance();
                AuthKind::OAuth
            }
            _ => AuthKind::Jwt,
        },
        None => AuthKind::Jwt,
    };

    expect_token(cursor, source, Token::KwSecret)?;
    let secret = parse_expr(cursor, source)?;

    Ok(Item::Auth(AuthDef {
        kind,
        secret,
        expires: None,
        span: Span::new(start_span.start, cursor.span().end),
    }))
}

fn parse_routes(cursor: &mut Cursor, source: &str) -> ParseResult<Item> {
    let start_span = cursor.span();
    expect_token(cursor, source, Token::KwRoutes)?;
    cursor.skip_newlines();

    let mut routes = Vec::new();

    while cursor.is_expr_start() {
        let pattern = expect_string(cursor, source)?;
        expect_token(cursor, source, Token::FatArrow)?;
        let screen = expect_ident(cursor, source)?;

        routes.push(Route {
            pattern,
            screen,
            span: Span::new(start_span.start, cursor.span().end),
        });

        cursor.skip_newlines();
    }

    Ok(Item::Routes(RoutesDef {
        routes,
        otherwise: None,
        span: Span::new(start_span.start, cursor.span().end),
    }))
}

/// === Helper Functions ===
/// Parse an indented block of items
fn parse_indented_block<T, F>(
    cursor: &mut Cursor,
    source: &str,
    mut parse_item: F,
) -> ParseResult<Vec<T>>
where
    F: FnMut(&mut Cursor, &str) -> ParseResult<T>,
{
    let mut items = Vec::new();

    // Expect an indent
    if cursor.check_indent().is_none() {
        // Single-line block (no indentation)
        return Ok(items);
    }

    let indent_level = cursor
        .expect_indent()
        .map_err(|msg| make_error(cursor, source, &msg))?;

    loop {
        // Check for dedent or EOF
        if cursor.check_dedent() || cursor.check_end_of_block() || cursor.is_eof() {
            // Consume dedent tokens
            while cursor.check_dedent() {
                cursor.advance();
            }
            break;
        }

        // Parse item
        let item = parse_item(cursor, source)?;
        items.push(item);

        // Skip newlines
        cursor.skip_newlines();

        // Check if we've dedented
        if let Some(level) = cursor.check_indent() {
            if level < indent_level {
                // We've dedented, end of block
                break;
            }
        } else if cursor.check_dedent() {
            break;
        }
    }

    Ok(items)
}

/// Expect an identifier token
fn expect_ident(cursor: &mut Cursor, source: &str) -> ParseResult<String> {
    match cursor.peek() {
        Some(t) => {
            // Accept actual identifiers
            if let Token::Ident(s) = &t.token {
                let s = s.clone();
                cursor.advance();
                return Ok(s);
            }
            // Accept property keywords used as parameter/variable names
            let kw_as_ident = match &t.token {
                Token::KwColor => Some("color"),
                Token::KwSize => Some("size"),
                Token::KwFill => Some("fill"),
                Token::KwGap => Some("gap"),
                Token::KwRadius => Some("radius"),
                Token::KwFrom => Some("from"),
                Token::KwTo => Some("to"),
                Token::KwUrl => Some("url"),
                _ => None,
            };
            if let Some(name) = kw_as_ident {
                let s = name.to_string();
                cursor.advance();
                return Ok(s);
            }
            Err(make_error(
                cursor,
                source,
                &format!("Expected identifier, found {}", t.token),
            ))
        }
        None => Err(make_error(cursor, source, "Expected identifier ")),
    }
}

/// Expect a string literal
fn expect_string(cursor: &mut Cursor, source: &str) -> ParseResult<String> {
    match cursor.peek() {
        Some(t) => match &t.token {
            Token::LitStr(s) | Token::LitStrRaw(s) => {
                let s = s.clone();
                cursor.advance();
                // Remove surrounding quotes
                let s = s.strip_prefix(r#"""#).unwrap_or(&s);
                let s = s.strip_suffix(r#"""#).unwrap_or(s);
                Ok(s.to_string())
            }
            Token::Ident(s) => {
                let s = s.clone();
                cursor.advance();
                Ok(s)
            }
            _ => Err(make_error(
                cursor,
                source,
                &format!("Expected string, found {}", t.token),
            )),
        },
        None => Err(make_error(cursor, source, "Expected string ")),
    }
}

/// Expect a specific token
fn expect_token(cursor: &mut Cursor, source: &str, expected: Token) -> ParseResult<()> {
    match cursor.expect(expected.clone()) {
        Ok(_) => Ok(()),
        Err(msg) => Err(make_error(cursor, source, &msg)),
    }
}

/// Create a parse error at current position
fn make_error(cursor: &Cursor, source: &str, message: &str) -> ParseError {
    let span = cursor.span();
    ParseError {
        message: message.to_string(),
        span: SourceSpan::from(span.start..span.end),
        source_code: source.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::{filter_tokens, tokenize};

    fn parse_program(source: &str) -> Program {
        let tokens = filter_tokens(tokenize(source).expect("tokenize should succeed"));
        parse(tokens, source).expect("parse should succeed")
    }

    #[test]
    fn test_parse_stream_screen() {
        let source = r#"screen Dashboard
  stream: yes
  box Root
    words "Hello"
"#;
        let program = parse_program(source);
        let screen = match &program.items[0] {
            Item::Screen(s) => s,
            _ => panic!("expected screen"),
        };
        assert_eq!(screen.stream, Some(true));
    }

    #[test]
    fn test_parse_lazy_box() {
        let source = r#"screen LazyDemo
  @lazy
  box Hero
    words "Hi"
"#;
        let program = parse_program(source);
        let screen = match &program.items[0] {
            Item::Screen(s) => s,
            _ => panic!("expected screen"),
        };
        let first = screen.children.first().expect("expected child node");
        match first {
            UiNode::Box(node) => assert!(node.lazy),
            _ => panic!("expected box"),
        }
    }

    #[test]
    fn test_parse_spring_animation() {
        let source = r#"screen Flow
  box Card
    animate: spring { stiffness: 120, damping: 14 }
"#;
        let program = parse_program(source);
        let screen = match &program.items[0] {
            Item::Screen(s) => s,
            _ => panic!("expected screen"),
        };
        let first = screen.children.first().expect("expected child node");
        match first {
            UiNode::Box(node) => {
                let anim = node.props.animate.as_ref().expect("animate should exist");
                let physics = anim.physics.as_ref().expect("physics should exist");
                assert_eq!(physics.stiffness, Some(120.0));
                assert_eq!(physics.damping, Some(14.0));
            }
            _ => panic!("expected box"),
        }
    }

    #[test]
    fn test_parse_gesture() {
        let source = r#"screen Gestures
  box Slider
    pan: horizontal { on release: next velocity: 0.5 offset: 30% }
"#;
        let program = parse_program(source);
        let screen = match &program.items[0] {
            Item::Screen(s) => s,
            _ => panic!("expected screen"),
        };
        let first = screen.children.first().expect("expected child node");
        match first {
            UiNode::Box(node) => {
                let gesture = node.props.gesture.as_ref().expect("gesture should exist");
                assert_eq!(gesture.direction, GestureDirection::Horizontal);
                assert!(gesture.on_release.is_some());
            }
            _ => panic!("expected box"),
        }
    }

    #[test]
    fn test_parse_box_center_direction_values() {
        let source = r#"screen Layout
  box Modal
    center: across
"#;

        let program = parse_program(source);
        let screen = match &program.items[0] {
            Item::Screen(s) => s,
            _ => panic!("expected screen"),
        };

        let first = screen.children.first().expect("expected child node");
        match first {
            UiNode::Box(node) => {
                assert_eq!(node.props.center, Some(CenterDir::Across));
            }
            _ => panic!("expected box"),
        }
    }

    #[test]
    fn test_parse_box_layer_and_backdrop() {
        let source = r#"screen Layout
  box Modal
    layer: 2
    backdrop: blur 8
"#;

        let program = parse_program(source);
        let screen = match &program.items[0] {
            Item::Screen(s) => s,
            _ => panic!("expected screen"),
        };

        let first = screen.children.first().expect("expected child node");
        match first {
            UiNode::Box(node) => {
                assert!(matches!(node.props.layer, Some(Expr::Num(2, _))));
                assert!(matches!(node.props.backdrop, Some(Expr::Num(8, _))));
            }
            _ => panic!("expected box"),
        }
    }

    #[test]
    fn test_parse_box_hover_animation() {
        let source = r#"screen Flow
  box Card
    hover: { to: { scale: 1.02 } }
"#;

        let program = parse_program(source);
        let screen = match &program.items[0] {
            Item::Screen(s) => s,
            _ => panic!("expected screen"),
        };

        let first = screen.children.first().expect("expected child node");
        match first {
            UiNode::Box(node) => {
                let hover = node.props.hover.as_ref().expect("hover should exist");
                let to = hover.to.as_ref().expect("hover.to should exist");
                assert_eq!(to.scale, Some(1.02));
            }
            _ => panic!("expected box"),
        }
    }

    #[test]
    fn test_parse_live_sync() {
        let source = r#"live chat
  sync: automatic
  presence: yes
  transform: lww
"#;
        let program = parse_program(source);
        let live = match &program.items[0] {
            Item::Live(l) => l,
            _ => panic!("expected live"),
        };
        assert_eq!(live.sync, Some(SyncMode::Automatic));
        assert!(live.presence);
        assert_eq!(live.transform, Some(CrdtType::Lww));
    }

    #[test]
    fn test_parse_ghost_screen() {
        let source = r#"screen Ghosted
  ghost on
  -- show me a chart of growth
  box root
    words "Ghost"
"#;

        let program = parse_program(source);
        let screen = match &program.items[0] {
            Item::Screen(s) => s,
            _ => panic!("expected screen"),
        };

        assert!(screen.ghost);
        assert!(screen.intents.iter().any(|i| i.contains("chart of growth")));
    }

    #[test]
    fn test_parse_ghost_scopes_intents_to_screen_body() {
        let source = r#"-- file level note
screen Ghosted
  ghost on
  -- keep this
  box root
    words "Ghost"

screen Other
  box a
    words "ignore"
"#;

        let program = parse_program(source);
        let screen = match &program.items[0] {
            Item::Screen(s) => s,
            _ => panic!("expected screen"),
        };

        assert!(screen.ghost);
        assert_eq!(screen.intents, vec!["keep this".to_string()]);
    }

    #[test]
    fn test_parse_tap_guard_and_confirm() {
        let source = r#"screen SafeActions
  tap "Delete"
    guard: canDelete
    confirm: yes
    => go "done"
"#;

        let program = parse_program(source);
        let screen = match &program.items[0] {
            Item::Screen(s) => s,
            _ => panic!("expected screen"),
        };

        let first = screen.children.first().expect("expected child node");
        match first {
            UiNode::Tap(tap) => {
                assert!(tap.confirm);
                assert!(matches!(tap.guard, Some(Expr::Ident(ref name, _)) if name == "canDelete"));
            }
            _ => panic!("expected tap"),
        }
    }

    #[test]
    fn test_parse_remember_stmt_in_think() {
        let source = r#"think main
  remember count: num = 0
  give count
"#;

        let program = parse_program(source);
        let think = match &program.items[0] {
            Item::Think(t) => t,
            _ => panic!("expected think"),
        };

        match &think.body[0] {
            Stmt::Remember {
                name,
                ty,
                computed,
                dependencies,
                ..
            } => {
                assert_eq!(name, "count");
                assert!(matches!(ty, Some(TypeExpr::Num)));
                assert!(!computed);
                assert!(dependencies.is_empty());
            }
            _ => panic!("expected remember stmt"),
        }
    }

    #[test]
    fn test_parse_remember_stmt_computed_dependencies() {
        let source = r#"think main
  remember base = 10
  remember multiplier = 2
  remember result = base * multiplier
  give result
"#;

        let program = parse_program(source);
        let think = match &program.items[0] {
            Item::Think(t) => t,
            _ => panic!("expected think"),
        };

        match &think.body[2] {
            Stmt::Remember {
                name,
                computed,
                dependencies,
                ..
            } => {
                assert_eq!(name, "result");
                assert!(*computed);
                assert_eq!(
                    dependencies,
                    &vec!["base".to_string(), "multiplier".to_string()]
                );
            }
            _ => panic!("expected remember stmt"),
        }
    }

    #[test]
    fn test_parse_fetch_stmt_with_when_cache_handlers() {
        let source = r#"think main
  fetch user
    from: "/api/users/{userId}"
    when: [userId]
    cache: 5min
    loading: showSpinner
    error: showError
  give user
"#;

        let program = parse_program(source);
        let think = match &program.items[0] {
            Item::Think(t) => t,
            _ => panic!("expected think"),
        };

        match &think.body[0] {
            Stmt::Fetch(fetch) => {
                assert_eq!(fetch.target, "user");
                assert_eq!(fetch.when_deps, vec!["userId".to_string()]);
                assert_eq!(fetch.cache_duration.as_deref(), Some("5min"));
                assert_eq!(fetch.loading_handler.as_deref(), Some("showSpinner"));
                assert_eq!(fetch.error_handler.as_deref(), Some("showError"));
            }
            _ => panic!("expected fetch stmt"),
        }
    }
}
