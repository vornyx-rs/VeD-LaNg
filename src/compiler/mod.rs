pub mod fullstack;
pub mod native;
pub mod server;
pub mod web;

#[cfg(test)]
mod tests;

use crate::ast::*;
use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

/// Compiler error
#[derive(Error, Debug, Diagnostic)]
#[error("Compile error: {message}")]
#[diagnostic(code(ved::compile))]
pub struct CompileError {
    pub message: String,
    #[label("here")]
    pub span: SourceSpan,
}

/// Compilation result
pub type CompileResult<T> = Result<T, CompileError>;

/// Target architecture
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(dead_code)]
pub enum Architecture {
    Wasm32,
    X86_64,
    AArch64,
    Arm32,
}

/// Optimization level
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(dead_code)]
pub enum OptLevel {
    Debug,
    Release,
    Size,
    Aggressive,
}

// ─── Shared Rust codegen ────────────────────────────────────────────────────

/// Emit a VED expression as a Rust expression string.
pub fn emit_expr(expr: &Expr) -> String {
    match expr {
        Expr::Num(n, _) => n.to_string(),
        Expr::Dec(d, _) => format!("{}_f64", d),
        Expr::Bool(b, _) => b.to_string(),
        Expr::Nothing(_) => "()".to_string(),
        Expr::Color(c, _) => format!("String::from({:?})", c),
        Expr::Ident(s, _) => rust_ident(s),

        Expr::Text(parts, _) => {
            let all_literal = parts.iter().all(|p| matches!(p, TextPart::Literal(_)));
            if all_literal {
                let s: String = parts
                    .iter()
                    .map(|p| match p {
                        TextPart::Literal(s) => s.clone(),
                        _ => unreachable!(),
                    })
                    .collect();
                format!("String::from({:?})", s)
            } else {
                // format! macro with interpolated parts
                let mut fmt_str = String::new();
                let mut args: Vec<String> = Vec::new();
                for part in parts {
                    match part {
                        TextPart::Literal(s) => {
                            fmt_str.push_str(&s.replace('{', "{{").replace('}', "}}"));
                        }
                        TextPart::Interp(e) => {
                            fmt_str.push_str("{}");
                            args.push(emit_expr(e));
                        }
                    }
                }
                if args.is_empty() {
                    format!("String::from({:?})", fmt_str)
                } else {
                    format!("format!({:?}, {})", fmt_str, args.join(", "))
                }
            }
        }

        Expr::List(elems, _) => {
            let items: Vec<String> = elems.iter().map(emit_expr).collect();
            format!("vec![{}]", items.join(", "))
        }

        Expr::Map(entries, _) => {
            if entries.is_empty() {
                "std::collections::HashMap::new()".to_string()
            } else {
                let inserts: Vec<String> = entries
                    .iter()
                    .map(|(k, v)| format!("__m.insert({}, {});", emit_expr(k), emit_expr(v)))
                    .collect();
                format!(
                    "{{ let mut __m = std::collections::HashMap::new(); {} __m }}",
                    inserts.join(" ")
                )
            }
        }

        Expr::Construct { name, fields, .. } => {
            let field_strs: Vec<String> = fields
                .iter()
                .map(|(k, v)| format!("{}: {}", rust_ident(k), emit_expr(v)))
                .collect();
            format!("{} {{ {} }}", rust_ident(name), field_strs.join(", "))
        }

        Expr::Field { obj, field, .. } => {
            format!("{}.{}", emit_expr(obj), rust_ident(field))
        }

        Expr::Index { obj, index, .. } => {
            format!("{}[({}) as usize]", emit_expr(obj), emit_expr(index))
        }

        Expr::Call { func, args, .. } => {
            let arg_strs: Vec<String> = args.iter().map(emit_expr).collect();
            // Map VED builtins to Rust equivalents
            if let Expr::Ident(name, _) = func.as_ref() {
                match name.as_str() {
                    "print" => return format!("print!(\"{{:?}}\", {})", arg_strs.join(", ")),
                    "println" => return format!("println!(\"{{:?}}\", {})", arg_strs.join(", ")),
                    "len" => {
                        if let Some(a) = arg_strs.first() {
                            return format!("{}.len() as i64", a);
                        }
                    }
                    "upper" => {
                        if let Some(a) = arg_strs.first() {
                            return format!("{}.to_uppercase()", a);
                        }
                    }
                    "lower" => {
                        if let Some(a) = arg_strs.first() {
                            return format!("{}.to_lowercase()", a);
                        }
                    }
                    "trim" => {
                        if let Some(a) = arg_strs.first() {
                            return format!("{}.trim().to_string()", a);
                        }
                    }
                    "to_string" | "to_text" => {
                        if let Some(a) = arg_strs.first() {
                            return format!("{}.to_string()", a);
                        }
                    }
                    _ => {}
                }
            }
            format!("{}({})", emit_expr(func), arg_strs.join(", "))
        }

        Expr::Pipe { left, right, .. } => {
            // left |> right  ⟹  right(left)  or  right_fn(left, extra_args...)
            match right.as_ref() {
                Expr::Ident(name, _) => format!("{}({})", rust_ident(name), emit_expr(left)),
                Expr::Call { func, args, .. } => {
                    let mut all_args = vec![emit_expr(left)];
                    all_args.extend(args.iter().map(emit_expr));
                    format!("{}({})", emit_expr(func), all_args.join(", "))
                }
                _ => {
                    format!(
                        "{{ let __pipe = {}; {} }}",
                        emit_expr(left),
                        emit_expr(right)
                    )
                }
            }
        }

        Expr::BinOp {
            left, op, right, ..
        } => {
            let l = emit_expr(left);
            let r = emit_expr(right);
            let op_str = match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                BinOp::Div => "/",
                BinOp::Mod => "%",
                BinOp::Eq => "==",
                BinOp::NotEq => "!=",
                BinOp::Lt => "<",
                BinOp::Gt => ">",
                BinOp::LtEq => "<=",
                BinOp::GtEq => ">=",
                BinOp::And => "&&",
                BinOp::Or => "||",
                BinOp::Is => "==", // type-check becomes equality in generated Rust
                BinOp::In => "/* in */", // handled specially below
            };
            if matches!(op, BinOp::In) {
                format!("{}.contains(&{})", r, l)
            } else {
                format!("({} {} {})", l, op_str, r)
            }
        }

        Expr::Not { expr, .. } => format!("(!{})", emit_expr(expr)),
        Expr::Neg { expr, .. } => format!("(-{})", emit_expr(expr)),

        Expr::Lambda { params, body, .. } => {
            format!("|{}| {}", params.join(", "), emit_expr(body))
        }

        Expr::Wait { expr, .. } => format!("{}.await", emit_expr(expr)),

        Expr::Try { expr, .. } => format!("{}?", emit_expr(expr)),

        Expr::With { base, fields, .. } => {
            let field_strs: Vec<String> = fields
                .iter()
                .map(|(k, v)| format!("{}: {}", rust_ident(k), emit_expr(v)))
                .collect();
            format!(
                "{{ let mut __w = {}; {} __w }}",
                emit_expr(base),
                field_strs
                    .iter()
                    .map(|f| format!("__w.{};", f))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        }

        Expr::Env { name, .. } => {
            format!("std::env::var({:?}).unwrap_or_default()", name)
        }

        Expr::Param { name, .. } => {
            // In server context, params come from axum Path extractor
            format!("{}_param", rust_ident(name))
        }

        Expr::DbAll { table, .. } => {
            format!(
                "sqlx::query_as!({}Row, \"SELECT * FROM {}\").fetch_all(&pool).await?",
                table, table
            )
        }

        Expr::DbOne { table, key, .. } => {
            format!(
                "sqlx::query_as!({}Row, \"SELECT * FROM {} WHERE id = ?\", {}).fetch_one(&pool).await?",
                table, table, emit_expr(key)
            )
        }

        Expr::DbSave { table, record, .. } => {
            format!(
                "/* db save {} */ {{ let _rec = {}; () }}",
                table,
                emit_expr(record)
            )
        }

        Expr::DbRemove { table, key, .. } => {
            format!(
                "sqlx::query!(\"DELETE FROM {} WHERE id = ?\", {}).execute(&pool).await?",
                table,
                emit_expr(key)
            )
        }

        Expr::DbQuery { sql, .. } => {
            format!("sqlx::query!({}).fetch_all(&pool).await?", emit_expr(sql))
        }

        Expr::Handle {
            expr,
            ok_arm,
            fail_arm,
            ..
        } => {
            let inner = emit_expr(expr);
            let ok_branch = ok_arm
                .as_ref()
                .map(|arm| {
                    let binding = arm.binding.as_deref().unwrap_or("_ok");
                    format!("Ok({}) => {}", rust_ident(binding), emit_expr(&arm.body))
                })
                .unwrap_or_else(|| "Ok(_) => ()".to_string());
            let fail_branch = fail_arm
                .as_ref()
                .map(|arm| {
                    let binding = arm.binding.as_deref().unwrap_or("_err");
                    format!("Err({}) => {}", rust_ident(binding), emit_expr(&arm.body))
                })
                .unwrap_or_else(|| "Err(_) => ()".to_string());
            format!("match {} {{ {}, {} }}", inner, ok_branch, fail_branch)
        }

        Expr::Fetch(f) => {
            let method = match f.method {
                HttpMethod::Get => "get",
                HttpMethod::Post => "post",
                HttpMethod::Put => "put",
                HttpMethod::Patch => "patch",
                HttpMethod::Del => "delete",
            };
            let url = emit_expr(&f.url);
            if let Some(body) = &f.body {
                format!(
                    "reqwest::Client::new().{}({}).json(&{}).send().await",
                    method,
                    url,
                    emit_expr(body)
                )
            } else {
                format!("reqwest::Client::new().{}({}).send().await", method, url)
            }
        }

        #[allow(unreachable_patterns)]
        _ => "/* unhandled */()".to_string(),
    }
}

/// Emit a VED statement as one or more Rust statement strings at the given indent level.
pub fn emit_stmt(stmt: &Stmt, indent: usize) -> String {
    let pad = "    ".repeat(indent);
    match stmt {
        Stmt::Let {
            name,
            value,
            mutable,
            ..
        } => {
            let kw = if *mutable { "let mut" } else { "let" };
            format!("{}{} {} = {};", pad, kw, rust_ident(name), emit_expr(value))
        }

        Stmt::Remember { name, value, .. } => {
            format!(
                "{}let mut {} = {};",
                pad,
                rust_ident(name),
                emit_expr(value)
            )
        }

        Stmt::Assign { target, value, .. } => {
            let lhs = match target {
                AssignTarget::Simple(name, _) => rust_ident(name),
                AssignTarget::Field(obj, field, _) => {
                    format!("{}.{}", emit_expr(obj), rust_ident(field))
                }
                AssignTarget::Index(obj, idx, _) => {
                    format!("{}[({}) as usize]", emit_expr(obj), emit_expr(idx))
                }
            };
            format!("{}{} = {};", pad, lhs, emit_expr(value))
        }

        Stmt::Expr(expr) => format!("{}{};", pad, emit_expr(expr)),

        Stmt::Give { value, .. } => format!("{}return {};", pad, emit_expr(value)),

        Stmt::Fail { value, .. } => {
            format!(
                "{}return Err(anyhow::anyhow!(\"{{}}\", {}));",
                pad,
                emit_expr(value)
            )
        }

        Stmt::When {
            arms, otherwise, ..
        } => {
            let mut out = String::new();
            for (i, arm) in arms.iter().enumerate() {
                let kw = if i == 0 { "if" } else { "} else if" };
                out.push_str(&format!("{}{} {} {{\n", pad, kw, emit_expr(&arm.cond)));
                for s in &arm.body {
                    out.push_str(&emit_stmt(s, indent + 1));
                    out.push('\n');
                }
            }
            if let Some(other) = otherwise {
                out.push_str(&format!("{}}} else {{\n", pad));
                out.push_str(&emit_stmt(other, indent + 1));
                out.push('\n');
            }
            out.push_str(&format!("{}}}", pad));
            out
        }

        Stmt::Each {
            var, iter, body, ..
        } => {
            let mut out = format!("{}for {} in {} {{\n", pad, rust_ident(var), emit_expr(iter));
            for s in body {
                out.push_str(&emit_stmt(s, indent + 1));
                out.push('\n');
            }
            out.push_str(&format!("{}}}", pad));
            out
        }

        Stmt::Transaction { body, .. } => {
            let mut out = format!("{}// transaction\n", pad);
            for s in body {
                out.push_str(&emit_stmt(s, indent));
                out.push('\n');
            }
            out
        }

        Stmt::Fetch(f) => {
            // Reactive fetch: in Rust native context emit as a blocking HTTP call
            let url = emit_expr(&f.url);
            format!(
                "{}let {} = reqwest::blocking::get({}).map(|r| r.text().unwrap_or_default()).unwrap_or_default();",
                pad,
                rust_ident(&f.target),
                url
            )
        }

        // Event handlers: skip in non-UI targets
        Stmt::OnLoad { .. }
        | Stmt::OnKey { .. }
        | Stmt::OnTap { .. }
        | Stmt::OnHover { .. }
        | Stmt::OnMessage { .. }
        | Stmt::OnConnect { .. }
        | Stmt::OnLeave { .. } => format!("{}// event handler (skipped in this target)", pad),
    }
}

/// Convert a VED identifier to a valid Rust identifier.
/// Appends `_` to Rust reserved words; passes others through unchanged.
pub fn rust_ident(name: &str) -> String {
    const RUST_KEYWORDS: &[&str] = &[
        "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
        "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
        "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe",
        "use", "where", "while", "async", "await", "dyn", "abstract", "become", "box", "do",
        "final", "macro", "override", "priv", "try", "typeof", "unsized", "virtual", "yield",
    ];
    if RUST_KEYWORDS.contains(&name) {
        format!("{}_", name)
    } else {
        // Replace hyphens with underscores
        name.replace('-', "_")
    }
}
