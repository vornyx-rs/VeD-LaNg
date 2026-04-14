use crate::ast::*;
use crate::lexer::Span;
use crate::stdlib::BUILTINS;
use miette::{Diagnostic, SourceSpan};
use std::collections::HashMap;
use std::collections::HashSet;
use thiserror::Error;

/// Type checking error
#[derive(Error, Debug, Diagnostic)]
#[error("Type error: {message}")]
#[diagnostic(code(ved::typeck))]
pub struct TypeError {
    pub message: String,
    #[label("here")]
    pub span: SourceSpan,
}

/// Type check result
pub type TypeResult<T> = Result<T, TypeError>;

/// Type environment for variable and function tracking
#[derive(Debug, Clone)]
pub struct TypeEnv {
    vars: HashMap<String, TypeExpr>,
    parent: Option<Box<TypeEnv>>,
}

impl TypeEnv {
    /// Create new global environment
    pub fn new() -> Self {
        let mut vars = HashMap::new();
        // Insert builtin functions
        for (name, sig) in BUILTINS {
            if let Some(ty) = parse_builtin_sig(sig) {
                vars.insert(name.to_string(), ty);
            }
        }
        Self { vars, parent: None }
    }

    /// Create a child scope
    pub fn child(&self) -> Self {
        Self {
            vars: HashMap::new(),
            parent: Some(Box::new(self.clone())),
        }
    }

    /// Look up a variable type
    pub fn get(&self, name: &str) -> Option<TypeExpr> {
        self.vars
            .get(name)
            .cloned()
            .or_else(|| self.parent.as_ref().and_then(|p| p.get(name)))
    }

    /// Define a variable
    pub fn set(&mut self, name: String, ty: TypeExpr) {
        self.vars.insert(name, ty);
    }

    /// Check if variable exists in current scope only
    pub fn has_local(&self, name: &str) -> bool {
        self.vars.contains_key(name)
    }
}

impl Default for TypeEnv {
    fn default() -> Self {
        Self::new()
    }
}

/// Type check a program
pub fn check(program: Program) -> TypeResult<Program> {
    let mut env = TypeEnv::new();

    // First pass: collect all type definitions and function signatures
    for item in &program.items {
        match item {
            Item::Shape(s) => {
                env.set(s.name.clone(), TypeExpr::Named(s.name.clone()));
            }
            Item::Think(t) => {
                let param_types: Vec<TypeExpr> = t.params.iter().map(|p| p.ty.clone()).collect();
                let ret_type = t.ret.clone().unwrap_or(TypeExpr::Nothing);
                let fn_type = TypeExpr::Function(param_types, Box::new(ret_type));
                env.set(t.name.clone(), fn_type);
            }
            _ => {}
        }
    }

    // Second pass: check each item
    for item in &program.items {
        check_item(item, &mut env)?;
    }

    Ok(program)
}

/// Type check a single item
fn check_item(item: &Item, env: &mut TypeEnv) -> TypeResult<()> {
    match item {
        Item::Shape(s) => check_shape(s, env),
        Item::Think(t) => check_think(t, env),
        Item::Screen(s) => check_screen(s, env),
        Item::Piece(p) => check_piece(p, env),
        Item::Serve(s) => check_serve(s, env),
        Item::Database(d) => check_database(d, env),
        Item::Task(t) => check_task(t, env),
        Item::Live(l) => check_live(l, env),
        Item::Routes(_) | Item::Queue(_) | Item::Auth(_) | Item::Config(_) => Ok(()),
    }
}

fn check_shape(shape: &ShapeDef, env: &mut TypeEnv) -> TypeResult<()> {
    for field in &shape.fields {
        if let Some(default) = &field.default {
            let default_ty = infer_expr(default, env)?;
            if !is_assignable(&default_ty, &field.ty) {
                return Err(TypeError {
                    message: format!(
                        "Field '{}' default value has type {:?}, expected {:?}",
                        field.name, default_ty, field.ty
                    ),
                    span: SourceSpan::from(field.span.start..field.span.end),
                });
            }
        }
    }
    Ok(())
}

fn check_think(think: &ThinkDef, env: &mut TypeEnv) -> TypeResult<()> {
    let mut local = env.child();
    let mut remember_graph: HashMap<String, Vec<String>> = HashMap::new();

    // Add parameters to local scope
    for param in &think.params {
        local.set(param.name.clone(), param.ty.clone());
    }

    // Check each statement
    for stmt in &think.body {
        match stmt {
            Stmt::Remember {
                name,
                ty,
                value,
                computed,
                dependencies,
                span,
            } => {
                if *computed == dependencies.is_empty() {
                    return Err(TypeError {
                        message: format!(
                            "Invalid remember metadata for '{}': computed={} but dependencies={:?}",
                            name, computed, dependencies
                        ),
                        span: SourceSpan::from(span.start..span.end),
                    });
                }

                check_remember_stmt(
                    name,
                    ty,
                    value,
                    dependencies,
                    *span,
                    &mut local,
                    Some(&mut remember_graph),
                )?;
            }
            _ => check_stmt(stmt, &mut local)?,
        }
    }

    // Check return type if specified
    if let Some(expected_ret) = &think.ret {
        // Find the last statement's type or any give statement
        for stmt in &think.body {
            if let Stmt::Give { value, span } = stmt {
                let actual_ty = infer_expr(value, &local)?;
                if !is_assignable(&actual_ty, expected_ret) {
                    return Err(TypeError {
                        message: format!(
                            "Return type mismatch: expected {:?}, found {:?}",
                            expected_ret, actual_ty
                        ),
                        span: SourceSpan::from(span.start..span.end),
                    });
                }
            }
        }
    }

    Ok(())
}

fn check_screen(screen: &ScreenDef, env: &mut TypeEnv) -> TypeResult<()> {
    let mut local = env.child();

    // Add state declarations
    for state in &screen.state {
        let ty = infer_expr(&state.default, &local)?;
        local.set(state.name.clone(), ty);
    }

    // Check UI nodes
    for child in &screen.children {
        check_ui_node(child, &mut local)?;
    }

    if screen.stream == Some(true) && !has_static_content(&screen.children) {
        eprintln!(
            "warning[typeck]: screen '{}' enables stream mode but has no obvious static content",
            screen.name
        );
    }

    if screen.ghost && screen.intents.is_empty() {
        eprintln!(
            "warning[typeck]: screen '{}' enables ghost mode but has no intent comments",
            screen.name
        );
    }

    // Check embedded thinks
    for think in &screen.thinks {
        check_think(think, &mut local)?;
    }

    Ok(())
}

fn check_piece(piece: &PieceDef, env: &mut TypeEnv) -> TypeResult<()> {
    let mut local = env.child();

    // Add needs as parameters
    for param in &piece.needs {
        local.set(param.name.clone(), param.ty.clone());
    }

    for child in &piece.children {
        check_ui_node(child, &mut local)?;
    }

    Ok(())
}

fn check_ui_node(node: &UiNode, env: &mut TypeEnv) -> TypeResult<()> {
    match node {
        UiNode::Box(b) => {
            validate_anim_spec(b.props.animate.as_ref(), b.span)?;
            validate_anim_spec(b.props.enter.as_ref(), b.span)?;
            validate_anim_spec(b.props.leave.as_ref(), b.span)?;
            validate_anim_spec(b.props.hover.as_ref(), b.span)?;
            validate_anim_spec(b.props.press.as_ref(), b.span)?;

            for child in &b.children {
                check_ui_node(child, env)?;
            }
            Ok(())
        }
        UiNode::Words(w) => {
            let content_ty = infer_expr(&w.content, env)?;
            if !matches!(content_ty, TypeExpr::Text | TypeExpr::Any) {
                return Err(TypeError {
                    message: format!("Words content must be text, found {:?}", content_ty),
                    span: SourceSpan::from(w.span.start..w.span.end),
                });
            }
            Ok(())
        }
        UiNode::Tap(t) => {
            if let Some(guard) = &t.guard {
                let guard_ty = infer_expr(guard, env)?;
                if !matches!(guard_ty, TypeExpr::Bool | TypeExpr::Any) {
                    return Err(TypeError {
                        message: format!("Tap guard must be boolean, found {:?}", guard_ty),
                        span: SourceSpan::from(t.span.start..t.span.end),
                    });
                }
            }

            for stmt in &t.action {
                check_stmt(stmt, &mut *env)?;
            }
            Ok(())
        }
        UiNode::Field(f) => {
            if !env.has_local(&f.bind) {
                return Err(TypeError {
                    message: format!("Field binding '{}' not found in scope", f.bind),
                    span: SourceSpan::from(f.span.start..f.span.end),
                });
            }
            Ok(())
        }
        UiNode::Image(_) => Ok(()),
        UiNode::Piece(p) => {
            // Check that piece exists
            if env.get(&p.name).is_none() {
                return Err(TypeError {
                    message: format!("Piece '{}' not found", p.name),
                    span: SourceSpan::from(p.span.start..p.span.end),
                });
            }
            Ok(())
        }
        UiNode::Each(e) => {
            let iter_ty = infer_expr(&e.iter, env)?;
            if !matches!(
                iter_ty,
                TypeExpr::List(_) | TypeExpr::Map(_, _) | TypeExpr::Any
            ) {
                return Err(TypeError {
                    message: format!("Each requires a list or map, found {:?}", iter_ty),
                    span: SourceSpan::from(e.span.start..e.span.end),
                });
            }
            // Bring loop variable into scope for children
            let elem_ty = match &iter_ty {
                TypeExpr::List(t) => *t.clone(),
                _ => TypeExpr::Any,
            };
            let mut child_env = env.child();
            child_env.set(e.var.clone(), elem_ty);
            for child in &e.children {
                check_ui_node(child, &mut child_env)?;
            }
            Ok(())
        }
        UiNode::ShowWhen(s) => {
            let cond_ty = infer_expr(&s.cond, env)?;
            if !matches!(cond_ty, TypeExpr::Bool | TypeExpr::Any) {
                return Err(TypeError {
                    message: format!("Show condition must be boolean, found {:?}", cond_ty),
                    span: SourceSpan::from(s.span.start..s.span.end),
                });
            }
            for child in &s.children {
                check_ui_node(child, env)?;
            }
            Ok(())
        }
    }
}

fn check_serve(serve: &ServeDef, _env: &mut TypeEnv) -> TypeResult<()> {
    // Verify handler names exist
    for _route in &serve.routes {
        // Would check against defined thinks
    }
    Ok(())
}

fn check_live(live: &LiveDef, _env: &mut TypeEnv) -> TypeResult<()> {
    if matches!(
        live.transform,
        Some(CrdtType::PNCounter | CrdtType::GCounter)
    ) {
        let has_message_handler = live.on_message.is_some();
        if !has_message_handler {
            eprintln!(
                "warning[typeck]: live '{}' uses counter CRDT without an on message handler for numeric updates",
                live.name
            );
        }
    }

    if live.path.is_empty() {
        return Err(TypeError {
            message: format!(
                "Live endpoint '{}' must declare a non-empty path",
                live.name
            ),
            span: SourceSpan::from(live.span.start..live.span.end),
        });
    }

    Ok(())
}

fn check_database(_db: &DatabaseDef, _env: &mut TypeEnv) -> TypeResult<()> {
    Ok(())
}

fn check_task(task: &TaskDef, env: &mut TypeEnv) -> TypeResult<()> {
    let mut local = env.child();
    for stmt in &task.body {
        check_stmt(stmt, &mut local)?;
    }
    Ok(())
}

/// Type check a statement
fn check_stmt(stmt: &Stmt, env: &mut TypeEnv) -> TypeResult<()> {
    match stmt {
        Stmt::Let {
            name,
            ty,
            value,
            span,
            ..
        } => {
            let value_ty = infer_expr(value, env)?;

            if let Some(annotated_ty) = ty {
                if !is_assignable(&value_ty, annotated_ty) {
                    return Err(TypeError {
                        message: format!(
                            "Type mismatch in let binding: expected {:?}, found {:?}",
                            annotated_ty, value_ty
                        ),
                        span: SourceSpan::from(span.start..span.end),
                    });
                }
                env.set(name.clone(), annotated_ty.clone());
            } else {
                env.set(name.clone(), value_ty);
            }
            Ok(())
        }

        Stmt::Remember {
            name,
            ty,
            value,
            computed,
            dependencies,
            span,
        } => {
            if *computed == dependencies.is_empty() {
                return Err(TypeError {
                    message: format!(
                        "Invalid remember metadata for '{}': computed={} but dependencies={:?}",
                        name, computed, dependencies
                    ),
                    span: SourceSpan::from(span.start..span.end),
                });
            }

            check_remember_stmt(name, ty, value, dependencies, *span, env, None)
        }

        Stmt::Fetch(fetch) => {
            let url_ty = infer_expr(&fetch.url, env)?;
            if !matches!(url_ty, TypeExpr::Text | TypeExpr::Any) {
                return Err(TypeError {
                    message: format!("Fetch URL must be text, found {:?}", url_ty),
                    span: SourceSpan::from(fetch.span.start..fetch.span.end),
                });
            }

            for dep in &fetch.when_deps {
                if env.get(dep).is_none() {
                    return Err(TypeError {
                        message: format!("Undefined fetch dependency: {}", dep),
                        span: SourceSpan::from(fetch.span.start..fetch.span.end),
                    });
                }
            }

            env.set(fetch.target.clone(), TypeExpr::Any);
            Ok(())
        }

        Stmt::Assign {
            target,
            value,
            span,
        } => {
            let target_ty = infer_assign_target(target, env)?;
            let value_ty = infer_expr(value, env)?;

            if !is_assignable(&value_ty, &target_ty) {
                return Err(TypeError {
                    message: format!(
                        "Cannot assign value of type {:?} to target of type {:?}",
                        value_ty, target_ty
                    ),
                    span: SourceSpan::from(span.start..span.end),
                });
            }
            Ok(())
        }

        Stmt::Expr(expr) => {
            infer_expr(expr, env)?;
            Ok(())
        }

        Stmt::Give { value, span: _ } => {
            infer_expr(value, env)?;
            Ok(())
        }

        Stmt::Fail { value, span: _ } => {
            infer_expr(value, env)?;
            Ok(())
        }

        Stmt::When { arms, span, .. } => {
            for arm in arms {
                let cond_ty = infer_expr(&arm.cond, env)?;
                if !matches!(cond_ty, TypeExpr::Bool | TypeExpr::Any) {
                    return Err(TypeError {
                        message: format!("When condition must be boolean, found {:?}", cond_ty),
                        span: SourceSpan::from(span.start..span.end),
                    });
                }

                let mut local = env.child();
                for stmt in &arm.body {
                    check_stmt(stmt, &mut local)?;
                }
            }
            Ok(())
        }

        Stmt::Each {
            var,
            iter,
            body,
            span,
        } => {
            let iter_ty = infer_expr(iter, env)?;

            let item_ty = match iter_ty {
                TypeExpr::List(t) => *t,
                TypeExpr::Map(k, _) => *k,
                TypeExpr::Any => TypeExpr::Any,
                _ => {
                    return Err(TypeError {
                        message: format!("Each requires a list or map, found {:?}", iter_ty),
                        span: SourceSpan::from(span.start..span.end),
                    });
                }
            };

            let mut local = env.child();
            local.set(var.clone(), item_ty);

            for stmt in body {
                check_stmt(stmt, &mut local)?;
            }
            Ok(())
        }

        Stmt::Transaction { body, .. } => {
            let mut local = env.child();
            for stmt in body {
                check_stmt(stmt, &mut local)?;
            }
            Ok(())
        }

        _ => Ok(()),
    }
}

fn check_remember_stmt(
    name: &str,
    ty: &Option<TypeExpr>,
    value: &Expr,
    dependencies: &[String],
    span: Span,
    env: &mut TypeEnv,
    remember_graph: Option<&mut HashMap<String, Vec<String>>>,
) -> TypeResult<()> {
    if dependencies.iter().any(|dep| dep == name) {
        return Err(TypeError {
            message: format!(
                "Circular dependency detected: remember '{}' depends on itself",
                name
            ),
            span: SourceSpan::from(span.start..span.end),
        });
    }

    if let Some(graph) = remember_graph {
        for dep in dependencies {
            if has_path(dep, name, graph, &mut HashSet::new()) {
                return Err(TypeError {
                    message: format!(
                        "Circular dependency detected between remember signals '{}' and '{}'",
                        name, dep
                    ),
                    span: SourceSpan::from(span.start..span.end),
                });
            }
        }
        graph.insert(name.to_string(), dependencies.to_vec());
    }

    let value_ty = infer_expr(value, env)?;

    if let Some(annotated_ty) = ty {
        if !is_assignable(&value_ty, annotated_ty) {
            return Err(TypeError {
                message: format!(
                    "Type mismatch in remember binding: expected {:?}, found {:?}",
                    annotated_ty, value_ty
                ),
                span: SourceSpan::from(span.start..span.end),
            });
        }
        env.set(name.to_string(), annotated_ty.clone());
    } else {
        env.set(name.to_string(), value_ty);
    }

    Ok(())
}

fn has_path(
    from: &str,
    to: &str,
    graph: &HashMap<String, Vec<String>>,
    visited: &mut HashSet<String>,
) -> bool {
    if from == to {
        return true;
    }

    if !visited.insert(from.to_string()) {
        return false;
    }

    if let Some(next_deps) = graph.get(from) {
        for dep in next_deps {
            if has_path(dep, to, graph, visited) {
                return true;
            }
        }
    }

    false
}

/// Infer the type of an assignment target
fn infer_assign_target(target: &AssignTarget, env: &TypeEnv) -> TypeResult<TypeExpr> {
    match target {
        AssignTarget::Simple(name, _) => env.get(name).ok_or_else(|| TypeError {
            message: format!("Undefined variable: {}", name),
            span: SourceSpan::from(0..0),
        }),
        AssignTarget::Field(_, _, _) => Ok(TypeExpr::Any),
        AssignTarget::Index(_, _, _) => Ok(TypeExpr::Any),
    }
}

/// Infer the type of an expression
pub fn infer_expr(expr: &Expr, env: &TypeEnv) -> TypeResult<TypeExpr> {
    match expr {
        Expr::Num(_, _) => Ok(TypeExpr::Num),
        Expr::Dec(_, _) => Ok(TypeExpr::Dec),
        Expr::Text(_, _) => Ok(TypeExpr::Text),
        Expr::Bool(_, _) => Ok(TypeExpr::Bool),
        Expr::Nothing(_) => Ok(TypeExpr::Nothing),
        Expr::Color(_, _) => Ok(TypeExpr::Text),

        Expr::Ident(name, span) => env.get(name).ok_or_else(|| TypeError {
            message: format!("Undefined variable: {}", name),
            span: SourceSpan::from(span.start..span.end),
        }),

        Expr::List(elems, _span) => {
            if elems.is_empty() {
                return Ok(TypeExpr::List(Box::new(TypeExpr::Any)));
            }

            let first_ty = infer_expr(&elems[0], env)?;

            for elem in elems.iter().skip(1) {
                let elem_ty = infer_expr(elem, env)?;
                if !is_assignable(&elem_ty, &first_ty) {
                    return Ok(TypeExpr::List(Box::new(TypeExpr::Any)));
                }
            }

            Ok(TypeExpr::List(Box::new(first_ty)))
        }

        Expr::Map(entries, _span) => {
            if entries.is_empty() {
                return Ok(TypeExpr::Map(
                    Box::new(TypeExpr::Any),
                    Box::new(TypeExpr::Any),
                ));
            }

            let first_key_ty = infer_expr(&entries[0].0, env)?;
            let first_val_ty = infer_expr(&entries[0].1, env)?;

            Ok(TypeExpr::Map(
                Box::new(first_key_ty),
                Box::new(first_val_ty),
            ))
        }

        Expr::Construct {
            name,
            fields: _,
            span,
        } => {
            // Check if shape exists
            if env.get(name).is_none() {
                return Err(TypeError {
                    message: format!("Unknown shape: {}", name),
                    span: SourceSpan::from(span.start..span.end),
                });
            }
            Ok(TypeExpr::Named(name.clone()))
        }

        Expr::Field {
            obj,
            field: _,
            span: _,
        } => {
            let _obj_ty = infer_expr(obj, env)?;
            // For now, field access returns Any
            // In a full implementation, look up field type from shape
            Ok(TypeExpr::Any)
        }

        Expr::Index {
            obj,
            index,
            span: _,
        } => {
            let obj_ty = infer_expr(obj, env)?;
            let _index_ty = infer_expr(index, env)?;

            match obj_ty {
                TypeExpr::List(t) => Ok(*t),
                TypeExpr::Map(_, v) => Ok(*v),
                TypeExpr::Text => Ok(TypeExpr::Text),
                _ => Ok(TypeExpr::Any),
            }
        }

        Expr::Call {
            func,
            args,
            span: _,
        } => {
            if let Expr::Ident(name, _) = func.as_ref() {
                match name.as_str() {
                    "print" | "println" => return Ok(TypeExpr::Nothing),
                    "len" => return Ok(TypeExpr::Num),
                    "trim" | "upper" | "lower" => return Ok(TypeExpr::Text),
                    "first" | "last" => {
                        if let Some(arg) = args.first() {
                            let arg_ty = infer_expr(arg, env)?;
                            if let TypeExpr::List(t) = arg_ty {
                                return Ok(TypeExpr::Maybe(t));
                            }
                        }
                        return Ok(TypeExpr::Maybe(Box::new(TypeExpr::Any)));
                    }
                    _ => {}
                }
            }

            let fn_ty = infer_expr(func, env)?;

            match fn_ty {
                TypeExpr::Function(_, ret) => Ok(*ret),
                TypeExpr::Any => Ok(TypeExpr::Any),
                _ => {
                    // Allow calling Any (runtime check)
                    Ok(TypeExpr::Any)
                }
            }
        }

        Expr::Pipe {
            left,
            right,
            span: _,
        } => {
            let _left_ty = infer_expr(left, env)?;
            let right_ty = infer_expr(right, env)?;

            // Pipe transforms: left |> right means right(left)
            // If right is a function, return its return type
            match right_ty {
                TypeExpr::Function(_, ret) => Ok(*ret),
                _ => Ok(right_ty),
            }
        }

        Expr::BinOp {
            left,
            op,
            right,
            span: _,
        } => {
            let left_ty = infer_expr(left, env)?;
            let right_ty = infer_expr(right, env)?;

            match op {
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => match (&left_ty, &right_ty) {
                    (TypeExpr::Num, TypeExpr::Num) => Ok(TypeExpr::Num),
                    (TypeExpr::Dec, TypeExpr::Dec) => Ok(TypeExpr::Dec),
                    (TypeExpr::Text, TypeExpr::Text) if *op == BinOp::Add => Ok(TypeExpr::Text),
                    _ => Ok(TypeExpr::Any),
                },
                BinOp::Eq | BinOp::NotEq | BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq => {
                    Ok(TypeExpr::Bool)
                }
                BinOp::And | BinOp::Or => Ok(TypeExpr::Bool),
                _ => Ok(TypeExpr::Any),
            }
        }

        Expr::Not { expr, span: _ } => {
            let _expr_ty = infer_expr(expr, env)?;
            Ok(TypeExpr::Bool)
        }

        Expr::Neg { expr, span: _ } => {
            let expr_ty = infer_expr(expr, env)?;
            match expr_ty {
                TypeExpr::Num => Ok(TypeExpr::Num),
                TypeExpr::Dec => Ok(TypeExpr::Dec),
                _ => Ok(TypeExpr::Any),
            }
        }

        Expr::Lambda {
            params: _,
            body: _,
            span: _,
        } => {
            // For now, lambdas return Any
            // In full implementation, infer from body
            Ok(TypeExpr::Any)
        }

        Expr::Wait { expr, span: _ } => {
            let inner_ty = infer_expr(expr, env)?;
            // wait unwraps the async type
            Ok(inner_ty)
        }

        Expr::Try { expr, span: _ } => {
            let inner_ty = infer_expr(expr, env)?;
            Ok(TypeExpr::Maybe(Box::new(inner_ty)))
        }

        Expr::With {
            base,
            fields: _,
            span: _,
        } => {
            let base_ty = infer_expr(base, env)?;
            Ok(base_ty)
        }

        Expr::Env { name: _, span: _ } => Ok(TypeExpr::Text),
        Expr::Param { name: _, span: _ } => Ok(TypeExpr::Text),

        Expr::DbQuery {
            sql: _,
            as_type,
            span: _,
        } => Ok(as_type.clone().unwrap_or(TypeExpr::Any)),

        Expr::DbAll { table, span: _ } => {
            Ok(TypeExpr::List(Box::new(TypeExpr::Named(table.clone()))))
        }

        Expr::DbOne {
            table,
            key: _,
            span: _,
        } => Ok(TypeExpr::Maybe(Box::new(TypeExpr::Named(table.clone())))),

        Expr::DbSave {
            table,
            record: _,
            span: _,
        } => Ok(TypeExpr::Named(table.clone())),

        Expr::DbRemove {
            table: _,
            key: _,
            span: _,
        } => Ok(TypeExpr::Bool),

        _ => Ok(TypeExpr::Any),
    }
}

/// Check if source type can be assigned to target type
fn is_assignable(source: &TypeExpr, target: &TypeExpr) -> bool {
    if matches!(target, TypeExpr::Any) {
        return true;
    }

    if matches!(source, TypeExpr::Any) {
        return true;
    }

    match (source, target) {
        (TypeExpr::Num, TypeExpr::Num) => true,
        (TypeExpr::Dec, TypeExpr::Dec) => true,
        (TypeExpr::Text, TypeExpr::Text) => true,
        (TypeExpr::Bool, TypeExpr::Bool) => true,
        (TypeExpr::Nothing, TypeExpr::Nothing) => true,
        (TypeExpr::Named(a), TypeExpr::Named(b)) => a == b,
        (TypeExpr::List(a), TypeExpr::List(b)) => is_assignable(a, b),
        (TypeExpr::Map(ak, av), TypeExpr::Map(bk, bv)) => {
            is_assignable(ak, bk) && is_assignable(av, bv)
        }
        (TypeExpr::Maybe(a), TypeExpr::Maybe(b)) => is_assignable(a, b),
        (a, TypeExpr::Maybe(b)) => is_assignable(a, b),
        _ => false,
    }
}

/// Parse a builtin function signature string
fn parse_builtin_sig(sig: &str) -> Option<TypeExpr> {
    // Simple signature parser for "fn(T1, T2) -> R" format
    if sig.starts_with("fn(") {
        let params_start = 3;
        let params_end = sig.find(") -> ").unwrap_or(sig.len() - 1);
        let params_str = &sig[params_start..params_end];

        let params: Vec<TypeExpr> = params_str
            .split(',')
            .map(|s| parse_type_name(s.trim()))
            .collect();

        let ret_start = sig.find("-> ").map(|i| i + 3).unwrap_or(sig.len());
        let ret_str = &sig[ret_start..];
        let ret = parse_type_name(ret_str);

        Some(TypeExpr::Function(params, Box::new(ret)))
    } else {
        Some(parse_type_name(sig))
    }
}

/// Parse a simple type name
fn parse_type_name(name: &str) -> TypeExpr {
    match name {
        "num" => TypeExpr::Num,
        "dec" => TypeExpr::Dec,
        "text" => TypeExpr::Text,
        "bool" => TypeExpr::Bool,
        "nothing" => TypeExpr::Nothing,
        "any" => TypeExpr::Any,
        s if s.starts_with("list[") && s.ends_with("]") => {
            let inner = &s[5..s.len() - 1];
            TypeExpr::List(Box::new(parse_type_name(inner)))
        }
        s if s.starts_with("map[") && s.ends_with("]") => {
            let inner = &s[4..s.len() - 1];
            let parts: Vec<&str> = inner.split(',').collect();
            if parts.len() == 2 {
                TypeExpr::Map(
                    Box::new(parse_type_name(parts[0].trim())),
                    Box::new(parse_type_name(parts[1].trim())),
                )
            } else {
                TypeExpr::Any
            }
        }
        s if s.starts_with("maybe[") && s.ends_with("]") => {
            let inner = &s[6..s.len() - 1];
            TypeExpr::Maybe(Box::new(parse_type_name(inner)))
        }
        s => TypeExpr::Named(s.to_string()),
    }
}

fn has_static_content(nodes: &[UiNode]) -> bool {
    for node in nodes {
        match node {
            UiNode::Words(_) | UiNode::Image(_) => return true,
            UiNode::Box(b) => {
                if has_static_content(&b.children) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn validate_anim_spec(spec: Option<&AnimSpec>, span: Span) -> TypeResult<()> {
    let Some(spec) = spec else {
        return Ok(());
    };

    if let Some(physics) = &spec.physics {
        if let Some(stiffness) = physics.stiffness {
            if !stiffness.is_finite() || stiffness <= 0.0 {
                return Err(TypeError {
                    message: "Animation stiffness must be a positive number".to_string(),
                    span: SourceSpan::from(span.start..span.end),
                });
            }
        }
        if let Some(damping) = physics.damping {
            if !damping.is_finite() || damping < 0.0 {
                return Err(TypeError {
                    message: "Animation damping must be a non-negative number".to_string(),
                    span: SourceSpan::from(span.start..span.end),
                });
            }
        }
        if let Some(mass) = physics.mass {
            if !mass.is_finite() || mass <= 0.0 {
                return Err(TypeError {
                    message: "Animation mass must be a positive number".to_string(),
                    span: SourceSpan::from(span.start..span.end),
                });
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::{filter_tokens, tokenize};
    use crate::parser::parse;

    fn parse_program(source: &str) -> Program {
        let tokens = filter_tokens(tokenize(source).expect("tokenize should succeed"));
        parse(tokens, source).expect("parse should succeed")
    }

    #[test]
    fn remember_introduces_typed_binding() {
        let source = r#"think main
  remember count = 1
  give count
"#;

        let program = parse_program(source);
        let checked = check(program);

        assert!(
            checked.is_ok(),
            "type check should pass for remember binding"
        );
    }

    #[test]
    fn remember_computed_dependencies_typecheck() {
        let source = r#"think main
  remember base = 10
  remember multiplier = 2
  remember result = base * multiplier
  give result
"#;

        let program = parse_program(source);
        let checked = check(program);

        assert!(
            checked.is_ok(),
            "type check should pass for computed remember binding"
        );
    }

    #[test]
    fn remember_respects_type_annotation() {
        let source = r#"think main
  remember title: text = 1
  give title
"#;

        let program = parse_program(source);
        let err = check(program).expect_err("type check should fail for bad remember annotation");

        assert!(
            err.message.contains("Type mismatch in remember binding"),
            "unexpected error message: {}",
            err.message
        );
    }

    #[test]
    fn remember_rejects_self_dependency_cycle() {
        let source = r#"think main
  remember count = count + 1
  give count
"#;

        let program = parse_program(source);
        let err = check(program).expect_err("type check should fail for self-cycle remember");

        assert!(
            err.message.contains("Circular dependency detected"),
            "unexpected error message: {}",
            err.message
        );
    }

    #[test]
    fn fetch_stmt_introduces_target_binding() {
        let source = r#"think main
    let userId = 7
    fetch user
        from: "/api/users/{userId}"
        when: [userId]
        cache: 5min
    give user
"#;

        let program = parse_program(source);
        let checked = check(program);

        assert!(
            checked.is_ok(),
            "type check should pass for fetch statement"
        );
    }

    #[test]
    fn fetch_stmt_rejects_undefined_dependency() {
        let source = r#"think main
    fetch user
        from: "/api/users/{userId}"
        when: [userId]
    give user
"#;

        let program = parse_program(source);
        let err =
            check(program).expect_err("type check should fail for undefined fetch dependency");

        assert!(
            err.message.contains("Undefined fetch dependency"),
            "unexpected error message: {}",
            err.message
        );
    }
}
