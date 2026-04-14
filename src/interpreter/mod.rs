use crate::ast::*;
use crate::lexer::Span;
use miette::{Diagnostic, SourceSpan};
use reqwest::blocking::Client;
use reqwest::header::CONTENT_TYPE;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use thiserror::Error;

/// Runtime error
#[derive(Error, Debug, Diagnostic)]
#[error("Runtime error: {message}")]
#[diagnostic(code(ved::runtime))]
pub struct RuntimeError {
    pub message: String,
    #[label("here")]
    pub span: SourceSpan,
}

/// Interpreter result
pub type RuntimeResult<T> = Result<T, RuntimeError>;

/// Runtime value types
#[derive(Debug, Clone)]
pub enum Value {
    Num(i64),
    Dec(f64),
    Text(String),
    Bool(bool),
    List(Vec<Value>),
    Map(HashMap<String, Value>),
    /// Struct instance: (type_name, fields)
    Struct(String, HashMap<String, Value>),
    /// Function closure
    Lambda(Vec<String>, Box<Expr>, Env),
    Nothing,
}

impl Value {
    /// Convert value to boolean for conditionals
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Nothing => false,
            Value::Num(n) => *n != 0,
            Value::Text(s) => !s.is_empty(),
            Value::List(l) => !l.is_empty(),
            Value::Map(m) => !m.is_empty(),
            _ => true,
        }
    }

    /// Get type name for error messages
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Num(_) => "num",
            Value::Dec(_) => "dec",
            Value::Text(_) => "text",
            Value::Bool(_) => "bool",
            Value::List(_) => "list",
            Value::Map(_) => "map",
            Value::Struct(_name, _) => "struct",
            Value::Lambda(_, _, _) => "function",
            Value::Nothing => "nothing",
        }
    }
}

/// Compare two values for equality
fn value_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Num(a), Value::Num(b)) => a == b,
        (Value::Dec(a), Value::Dec(b)) => (a - b).abs() < f64::EPSILON,
        (Value::Text(a), Value::Text(b)) => a == b,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Nothing, Value::Nothing) => true,
        (Value::List(a), Value::List(b)) => {
            if a.len() != b.len() {
                return false;
            }
            a.iter().zip(b.iter()).all(|(x, y)| value_eq(x, y))
        }
        (Value::Map(a), Value::Map(b)) => {
            if a.len() != b.len() {
                return false;
            }
            a.iter()
                .all(|(k, v)| b.get(k).map(|bv| value_eq(v, bv)).unwrap_or(false))
        }
        (Value::Struct(a_name, a_fields), Value::Struct(b_name, b_fields)) => {
            a_name == b_name
                && a_fields.len() == b_fields.len()
                && a_fields
                    .iter()
                    .all(|(k, v)| b_fields.get(k).map(|bv| value_eq(v, bv)).unwrap_or(false))
        }
        _ => false,
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Num(n) => write!(f, "{}", n),
            Value::Dec(d) => write!(f, "{}", d),
            Value::Text(s) => write!(f, "{}", s),
            Value::Bool(b) => write!(f, "{}", b),
            Value::List(l) => {
                write!(f, "[")?;
                for (i, v) in l.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            Value::Map(m) => {
                write!(f, "{{")?;
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, "}}")
            }
            Value::Struct(name, _fields) => {
                write!(f, "{} {{ ... }}", name)
            }
            Value::Lambda(_, _, _) => write!(f, "<function>"),
            Value::Nothing => write!(f, "nothing"),
        }
    }
}

/// Execution environment
#[derive(Debug, Clone)]
pub struct Env {
    vars: HashMap<String, Value>,
    fetch_cache: HashMap<String, (Value, Instant)>,
    fetch_watchers: HashMap<String, Vec<FetchStmt>>,
    parent: Option<Box<Env>>,
}

impl Env {
    /// Create global environment
    pub fn new() -> Self {
        let mut vars = HashMap::new();

        // Insert builtin functions
        vars.insert(
            "print".to_string(),
            Value::Lambda(
                vec!["text".to_string()],
                Box::new(Expr::Nothing(Span::empty())),
                Self {
                    vars: HashMap::new(),
                    fetch_cache: HashMap::new(),
                    fetch_watchers: HashMap::new(),
                    parent: None,
                },
            ),
        );

        Self {
            vars,
            fetch_cache: HashMap::new(),
            fetch_watchers: HashMap::new(),
            parent: None,
        }
    }

    /// Create child scope
    pub fn child(&self) -> Self {
        Self {
            vars: HashMap::new(),
            fetch_cache: self.fetch_cache.clone(),
            fetch_watchers: self.fetch_watchers.clone(),
            parent: Some(Box::new(self.clone())),
        }
    }

    /// Look up variable
    pub fn get(&self, name: &str) -> Option<Value> {
        self.vars
            .get(name)
            .cloned()
            .or_else(|| self.parent.as_ref().and_then(|p| p.get(name)))
    }

    /// Define variable
    pub fn set(&mut self, name: String, val: Value) {
        self.vars.insert(name, val);
    }

    /// Assign to existing variable (searches parent scopes)
    pub fn assign(&mut self, name: &str, val: Value) -> RuntimeResult<()> {
        if self.vars.contains_key(name) {
            self.vars.insert(name.to_string(), val);
            Ok(())
        } else if let Some(parent) = self.parent.as_mut() {
            parent.assign(name, val)
        } else {
            Err(RuntimeError {
                message: format!("Undefined variable: {}", name),
                span: SourceSpan::from(0..0),
            })
        }
    }

    pub fn get_cached_fetch(&self, url: &str) -> Option<(Value, Instant)> {
        self.fetch_cache
            .get(url)
            .cloned()
            .or_else(|| self.parent.as_ref().and_then(|p| p.get_cached_fetch(url)))
    }

    pub fn set_cached_fetch(&mut self, url: String, val: Value) {
        self.fetch_cache.insert(url, (val, Instant::now()));
    }

    pub fn register_fetch_watcher(&mut self, fetch: &FetchStmt) {
        for dep in &fetch.when_deps {
            self.fetch_watchers
                .entry(dep.clone())
                .or_default()
                .push(fetch.clone());
        }
    }

    pub fn get_fetch_watchers(&self, dep: &str) -> Vec<FetchStmt> {
        let mut watchers = self.fetch_watchers.get(dep).cloned().unwrap_or_default();
        if let Some(parent) = self.parent.as_ref() {
            watchers.extend(parent.get_fetch_watchers(dep));
        }
        watchers
    }
}

impl Default for Env {
    fn default() -> Self {
        Self::new()
    }
}

/// Run a program
pub fn run(program: &Program) -> RuntimeResult<Value> {
    let mut env = Env::new();

    // Define shapes as constructors
    for item in &program.items {
        if let Item::Shape(s) = item {
            let name = s.name.clone();
            // Store shape definition as a special value
            env.set(name.clone(), Value::Text(format!("<shape {}>", name)));
        }
    }

    // Execute main function if exists
    for item in &program.items {
        if let Item::Think(t) = item {
            if t.name == "main" {
                let mut last_val = Value::Nothing;
                for stmt in &t.body {
                    last_val = eval_stmt(stmt, &mut env)?;
                }
                return Ok(last_val);
            }
        }
    }

    Ok(Value::Nothing)
}

/// Evaluate a statement
fn eval_stmt(stmt: &Stmt, env: &mut Env) -> RuntimeResult<Value> {
    match stmt {
        Stmt::Let { name, value, .. } => {
            let val = eval_expr(value, env)?;
            set_var_and_trigger(env, name.clone(), val)?;
            Ok(Value::Nothing)
        }

        Stmt::Remember { name, value, .. } => {
            let val = eval_expr(value, env)?;
            set_var_and_trigger(env, name.clone(), val)?;
            Ok(Value::Nothing)
        }

        Stmt::Fetch(fetch) => {
            env.register_fetch_watcher(fetch);
            eval_fetch_stmt(fetch, env)
        }

        Stmt::Assign {
            target,
            value,
            span: _,
        } => {
            let val = eval_expr(value, env)?;

            match target {
                AssignTarget::Simple(name, _) => {
                    assign_var_and_trigger(env, name, val)?;
                }
                AssignTarget::Field(obj, _field, _) => {
                    let _obj_val = eval_expr(obj, env)?;
                    // Field assignment would modify struct here
                }
                AssignTarget::Index(obj, index, _) => {
                    let _obj_val = eval_expr(obj, env)?;
                    let _idx_val = eval_expr(index, env)?;
                    // Index assignment would modify list/map here
                }
            }

            Ok(Value::Nothing)
        }

        Stmt::Expr(expr) => eval_expr(expr, env),

        Stmt::Give { value, .. } => eval_expr(value, env),

        Stmt::Fail { value, span } => {
            let val = eval_expr(value, env)?;
            Err(RuntimeError {
                message: format!("Failed: {}", val),
                span: SourceSpan::from(span.start..span.end),
            })
        }

        Stmt::When {
            arms,
            otherwise,
            span: _,
        } => {
            for arm in arms {
                let cond = eval_expr(&arm.cond, env)?;
                if cond.is_truthy() {
                    let mut local = env.child();
                    let mut last = Value::Nothing;
                    for stmt in &arm.body {
                        last = eval_stmt(stmt, &mut local)?;
                    }
                    return Ok(last);
                }
            }

            if let Some(other) = otherwise {
                let mut local = env.child();
                eval_stmt(other, &mut local)
            } else {
                Ok(Value::Nothing)
            }
        }

        Stmt::Each {
            var,
            iter,
            body,
            span,
        } => {
            let iter_val = eval_expr(iter, env)?;

            match iter_val {
                Value::List(items) => {
                    let mut last = Value::Nothing;
                    for item in items {
                        let mut local = env.child();
                        local.set(var.clone(), item);
                        for stmt in body {
                            last = eval_stmt(stmt, &mut local)?;
                        }
                    }
                    Ok(last)
                }
                Value::Map(map) => {
                    let mut last = Value::Nothing;
                    for (key, _val) in map {
                        let mut local = env.child();
                        local.set(var.clone(), Value::Text(key));
                        for stmt in body {
                            last = eval_stmt(stmt, &mut local)?;
                        }
                    }
                    Ok(last)
                }
                _ => Err(RuntimeError {
                    message: format!("Cannot iterate over {}", iter_val.type_name()),
                    span: SourceSpan::from(span.start..span.end),
                }),
            }
        }

        Stmt::Transaction { body, .. } => {
            let mut local = env.child();
            let mut last = Value::Nothing;
            for stmt in body {
                last = eval_stmt(stmt, &mut local)?;
            }
            Ok(last)
        }

        _ => Ok(Value::Nothing),
    }
}

fn eval_fetch_stmt(fetch: &FetchStmt, env: &mut Env) -> RuntimeResult<Value> {
    let url = eval_expr(&fetch.url, env)?;
    let url = match url {
        Value::Text(s) => s,
        other => {
            return Err(RuntimeError {
                message: format!("Fetch URL must evaluate to text, got {}", other.type_name()),
                span: SourceSpan::from(fetch.span.start..fetch.span.end),
            });
        }
    };

    if let Some(cache_duration) = fetch
        .cache_duration
        .as_deref()
        .and_then(parse_cache_duration)
    {
        if let Some((cached, fetched_at)) = env.get_cached_fetch(&url) {
            if fetched_at.elapsed() <= cache_duration {
                env.set(fetch.target.clone(), cached.clone());
                return Ok(Value::Nothing);
            }
        }
    }

    if let Some(handler) = fetch.loading_handler.as_deref() {
        let _ = try_call_handler(handler, env);
    }

    match fetch_url_as_value(&url) {
        Ok(value) => {
            env.set(fetch.target.clone(), value.clone());
            env.set_cached_fetch(url, value);
            Ok(Value::Nothing)
        }
        Err(err) => {
            env.set("error".to_string(), Value::Text(err.message.clone()));
            if let Some(handler) = fetch.error_handler.as_deref() {
                let _ = try_call_handler(handler, env);
            }
            Err(err)
        }
    }
}

fn set_var_and_trigger(env: &mut Env, name: String, val: Value) -> RuntimeResult<()> {
    let changed = env
        .get(&name)
        .map(|current| !value_eq(&current, &val))
        .unwrap_or(true);

    env.set(name.clone(), val);

    if changed {
        trigger_fetch_watchers_for(&name, env)?;
    }

    Ok(())
}

fn assign_var_and_trigger(env: &mut Env, name: &str, val: Value) -> RuntimeResult<()> {
    let changed = env
        .get(name)
        .map(|current| !value_eq(&current, &val))
        .unwrap_or(true);

    env.assign(name, val)?;

    if changed {
        trigger_fetch_watchers_for(name, env)?;
    }

    Ok(())
}

fn trigger_fetch_watchers_for(dep: &str, env: &mut Env) -> RuntimeResult<()> {
    let watchers = env.get_fetch_watchers(dep);
    for fetch in watchers {
        eval_fetch_stmt(&fetch, env)?;
    }
    Ok(())
}

fn try_call_handler(handler: &str, env: &mut Env) -> RuntimeResult<Value> {
    if env.get(handler).is_none() {
        return Ok(Value::Nothing);
    }

    let call = Expr::Call {
        func: Box::new(Expr::Ident(handler.to_string(), Span::empty())),
        args: Vec::new(),
        span: Span::empty(),
    };

    eval_expr(&call, env)
}

fn fetch_url_as_value(url: &str) -> RuntimeResult<Value> {
    if let Some(mock_payload) = url.strip_prefix("mock://") {
        let mut out = HashMap::new();
        out.insert("ok".to_string(), Value::Bool(true));
        out.insert("data".to_string(), Value::Text(mock_payload.to_string()));
        return Ok(Value::Map(out));
    }

    let client = Client::new();
    let response = client.get(url).send().map_err(|e| RuntimeError {
        message: format!("Fetch failed for '{}': {}", url, e),
        span: SourceSpan::from(0..0),
    })?;

    let status = response.status();
    if !status.is_success() {
        return Err(RuntimeError {
            message: format!("Fetch failed for '{}': HTTP {}", url, status),
            span: SourceSpan::from(0..0),
        });
    }

    let is_json = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .map(|ct| ct.contains("application/json"))
        .unwrap_or(false);

    let body = response.text().map_err(|e| RuntimeError {
        message: format!("Failed reading response body for '{}': {}", url, e),
        span: SourceSpan::from(0..0),
    })?;

    if is_json {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
            return Ok(json_to_value(json));
        }
    }

    Ok(Value::Text(body))
}

fn json_to_value(json: serde_json::Value) -> Value {
    match json {
        serde_json::Value::Null => Value::Nothing,
        serde_json::Value::Bool(b) => Value::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Num(i)
            } else if let Some(f) = n.as_f64() {
                Value::Dec(f)
            } else {
                Value::Nothing
            }
        }
        serde_json::Value::String(s) => Value::Text(s),
        serde_json::Value::Array(items) => {
            Value::List(items.into_iter().map(json_to_value).collect())
        }
        serde_json::Value::Object(map) => {
            let mut out = HashMap::new();
            for (k, v) in map {
                out.insert(k, json_to_value(v));
            }
            Value::Map(out)
        }
    }
}

fn value_to_json(val: &Value) -> String {
    match val {
        Value::Nothing => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Num(n) => n.to_string(),
        Value::Dec(d) => {
            if d.is_finite() {
                format!("{}", d)
            } else {
                "null".to_string()
            }
        }
        Value::Text(s) => {
            let escaped = s
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\r', "\\r")
                .replace('\t', "\\t");
            format!("\"{}\"", escaped)
        }
        Value::List(items) => {
            let parts: Vec<String> = items.iter().map(value_to_json).collect();
            format!("[{}]", parts.join(","))
        }
        Value::Map(m) => {
            let parts: Vec<String> = m
                .iter()
                .map(|(k, v)| format!("\"{}\":{}", k.replace('"', "\\\""), value_to_json(v)))
                .collect();
            format!("{{{}}}", parts.join(","))
        }
        Value::Struct(_, fields) => {
            let parts: Vec<String> = fields
                .iter()
                .map(|(k, v)| format!("\"{}\":{}", k.replace('"', "\\\""), value_to_json(v)))
                .collect();
            format!("{{{}}}", parts.join(","))
        }
        Value::Lambda(_, _, _) => "\"<function>\"".to_string(),
    }
}

fn parse_cache_duration(raw: &str) -> Option<Duration> {
    let trimmed = raw.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        return None;
    }

    let (num_part, unit_part) = split_duration_parts(&trimmed)?;
    let amount = num_part.parse::<u64>().ok()?;

    match unit_part {
        "ms" => Some(Duration::from_millis(amount)),
        "s" | "sec" | "secs" | "second" | "seconds" => Some(Duration::from_secs(amount)),
        "m" | "min" | "mins" | "minute" | "minutes" => Some(Duration::from_secs(amount * 60)),
        "h" | "hr" | "hrs" | "hour" | "hours" => Some(Duration::from_secs(amount * 60 * 60)),
        _ => None,
    }
}

fn split_duration_parts(raw: &str) -> Option<(&str, &str)> {
    let idx = raw.find(|c: char| !c.is_ascii_digit())?;
    let (num, unit) = raw.split_at(idx);
    if num.is_empty() || unit.is_empty() {
        return None;
    }
    Some((num, unit))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cache_duration_units() {
        assert_eq!(
            parse_cache_duration("500ms"),
            Some(Duration::from_millis(500))
        );
        assert_eq!(parse_cache_duration("5s"), Some(Duration::from_secs(5)));
        assert_eq!(parse_cache_duration("5min"), Some(Duration::from_secs(300)));
        assert_eq!(parse_cache_duration("1h"), Some(Duration::from_secs(3600)));
        assert_eq!(parse_cache_duration("bad"), None);
    }

    #[test]
    fn fetch_mock_sets_target_and_cache() {
        let mut env = Env::new();
        let fetch = FetchStmt {
            target: "user".to_string(),
            url: Expr::Text(
                vec![TextPart::Literal("mock://alice".to_string())],
                Span::empty(),
            ),
            when_deps: vec![],
            cache_duration: Some("5min".to_string()),
            loading_handler: None,
            error_handler: None,
            span: Span::empty(),
        };

        let result = eval_fetch_stmt(&fetch, &mut env);
        assert!(result.is_ok());

        let stored = env.get("user").expect("user should be set");
        match stored {
            Value::Map(map) => {
                assert!(matches!(map.get("ok"), Some(Value::Bool(true))));
                assert!(matches!(map.get("data"), Some(Value::Text(v)) if v == "alice"));
            }
            _ => panic!("expected map value from mock fetch"),
        }
    }

    #[test]
    fn fetch_reexecutes_when_dependency_changes() {
        let mut env = Env::new();

        let dep_name = "user_id".to_string();
        let target_name = "user".to_string();

        let dep_init = Stmt::Let {
            name: dep_name.clone(),
            ty: None,
            value: Expr::Text(vec![TextPart::Literal("alice".to_string())], Span::empty()),
            span: Span::empty(),
            mutable: false,
        };

        let fetch_stmt = Stmt::Fetch(FetchStmt {
            target: target_name.clone(),
            url: Expr::Text(
                vec![
                    TextPart::Literal("mock://".to_string()),
                    TextPart::Interp(Box::new(Expr::Ident(dep_name.clone(), Span::empty()))),
                ],
                Span::empty(),
            ),
            when_deps: vec![dep_name.clone()],
            cache_duration: None,
            loading_handler: None,
            error_handler: None,
            span: Span::empty(),
        });

        let dep_update = Stmt::Assign {
            target: AssignTarget::Simple(dep_name.clone(), Span::empty()),
            value: Expr::Text(vec![TextPart::Literal("bob".to_string())], Span::empty()),
            span: Span::empty(),
        };

        assert!(eval_stmt(&dep_init, &mut env).is_ok());
        assert!(eval_stmt(&fetch_stmt, &mut env).is_ok());

        match env.get(&target_name) {
            Some(Value::Map(map)) => {
                assert!(matches!(map.get("data"), Some(Value::Text(v)) if v == "alice"));
            }
            _ => panic!("expected initial fetch result"),
        }

        assert!(eval_stmt(&dep_update, &mut env).is_ok());

        match env.get(&target_name) {
            Some(Value::Map(map)) => {
                assert!(matches!(map.get("data"), Some(Value::Text(v)) if v == "bob"));
            }
            _ => panic!("expected updated fetch result"),
        }
    }
}

/// Evaluate an expression
pub fn eval_expr(expr: &Expr, env: &mut Env) -> RuntimeResult<Value> {
    match expr {
        Expr::Num(n, _) => Ok(Value::Num(*n)),
        Expr::Dec(d, _) => Ok(Value::Dec(*d)),
        Expr::Text(parts, _span) => {
            let mut result = String::new();
            for part in parts {
                match part {
                    TextPart::Literal(s) => result.push_str(s),
                    TextPart::Interp(e) => {
                        let val = eval_expr(e, env)?;
                        result.push_str(&val.to_string());
                    }
                }
            }
            Ok(Value::Text(result))
        }
        Expr::Bool(b, _) => Ok(Value::Bool(*b)),
        Expr::Nothing(_) => Ok(Value::Nothing),
        Expr::Color(c, _) => Ok(Value::Text(c.clone())),

        Expr::Ident(name, span) => env.get(name).ok_or_else(|| RuntimeError {
            message: format!("Undefined variable: {}", name),
            span: SourceSpan::from(span.start..span.end),
        }),

        Expr::List(elems, _span) => {
            let mut vals = Vec::new();
            for e in elems {
                vals.push(eval_expr(e, env)?);
            }
            Ok(Value::List(vals))
        }

        Expr::Map(entries, _span) => {
            let mut map = HashMap::new();
            for (k, v) in entries {
                let key_val = eval_expr(k, env)?;
                let val_val = eval_expr(v, env)?;

                // Use text representation as key
                let key_str = match key_val {
                    Value::Text(s) => s,
                    other => other.to_string(),
                };

                map.insert(key_str, val_val);
            }
            Ok(Value::Map(map))
        }

        Expr::Construct {
            name,
            fields,
            span: _,
        } => {
            let mut field_values = HashMap::new();
            for (field_name, expr) in fields {
                let val = eval_expr(expr, env)?;
                field_values.insert(field_name.clone(), val);
            }
            Ok(Value::Struct(name.clone(), field_values))
        }

        Expr::Field { obj, field, span } => {
            let obj_val = eval_expr(obj, env)?;
            match obj_val {
                Value::Struct(_, fields) | Value::Map(fields) => {
                    fields.get(field).cloned().ok_or_else(|| RuntimeError {
                        message: format!("Field '{}' not found", field),
                        span: SourceSpan::from(span.start..span.end),
                    })
                }
                Value::Text(s) => {
                    // String methods
                    match field.as_str() {
                        "len" => Ok(Value::Num(s.len() as i64)),
                        _ => Err(RuntimeError {
                            message: format!("Unknown field '{}' on text", field),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    }
                }
                _ => Err(RuntimeError {
                    message: format!("Cannot access field '{}' on {}", field, obj_val.type_name()),
                    span: SourceSpan::from(span.start..span.end),
                }),
            }
        }

        Expr::Index { obj, index, span } => {
            let obj_val = eval_expr(obj, env)?;
            let idx_val = eval_expr(index, env)?;

            match (obj_val, idx_val) {
                (Value::List(list), Value::Num(n)) => {
                    let idx = n as usize;
                    list.get(idx).cloned().ok_or_else(|| RuntimeError {
                        message: format!("Index {} out of bounds", idx),
                        span: SourceSpan::from(span.start..span.end),
                    })
                }
                (Value::Map(map), Value::Text(key)) => {
                    map.get(&key).cloned().ok_or_else(|| RuntimeError {
                        message: format!("Key '{}' not found", key),
                        span: SourceSpan::from(span.start..span.end),
                    })
                }
                (Value::Text(s), Value::Num(n)) => {
                    let idx = n as usize;
                    s.chars()
                        .nth(idx)
                        .map(|c| Value::Text(c.to_string()))
                        .ok_or_else(|| RuntimeError {
                            message: format!("Index {} out of bounds", idx),
                            span: SourceSpan::from(span.start..span.end),
                        })
                }
                _ => Err(RuntimeError {
                    message: "Invalid index operation".to_string(),
                    span: SourceSpan::from(span.start..span.end),
                }),
            }
        }

        Expr::Call { func, args, span } => {
            // Evaluate arguments
            let arg_vals: Vec<Value> = args
                .iter()
                .map(|a| eval_expr(a, env))
                .collect::<RuntimeResult<_>>()?;

            if let Expr::Ident(name, _) = func.as_ref() {
                match name.as_str() {
                    "print" => {
                        for (i, arg) in arg_vals.iter().enumerate() {
                            if i > 0 {
                                print!(" ");
                            }
                            print!("{}", arg);
                        }
                        Ok(Value::Nothing)
                    }
                    "println" => {
                        for (i, arg) in arg_vals.iter().enumerate() {
                            if i > 0 {
                                print!(" ");
                            }
                            print!("{}", arg);
                        }
                        println!();
                        Ok(Value::Nothing)
                    }
                    "len" => match arg_vals.first() {
                        Some(Value::Text(s)) => Ok(Value::Num(s.len() as i64)),
                        Some(Value::List(l)) => Ok(Value::Num(l.len() as i64)),
                        Some(Value::Map(m)) => Ok(Value::Num(m.len() as i64)),
                        Some(v) => Err(RuntimeError {
                            message: format!(
                                "len() requires text, list, or map, got {}",
                                v.type_name()
                            ),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                        None => Err(RuntimeError {
                            message: "len() requires an argument".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "upper" => match arg_vals.first() {
                        Some(Value::Text(s)) => Ok(Value::Text(s.to_uppercase())),
                        Some(v) => Err(RuntimeError {
                            message: format!("upper() requires text, got {}", v.type_name()),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                        None => Err(RuntimeError {
                            message: "upper() requires an argument".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "lower" => match arg_vals.first() {
                        Some(Value::Text(s)) => Ok(Value::Text(s.to_lowercase())),
                        Some(v) => Err(RuntimeError {
                            message: format!("lower() requires text, got {}", v.type_name()),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                        None => Err(RuntimeError {
                            message: "lower() requires an argument".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "trim" => match arg_vals.first() {
                        Some(Value::Text(s)) => Ok(Value::Text(s.trim().to_string())),
                        Some(v) => Err(RuntimeError {
                            message: format!("trim() requires text, got {}", v.type_name()),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                        None => Err(RuntimeError {
                            message: "trim() requires an argument".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "first" => match arg_vals.first() {
                        Some(Value::List(l)) => Ok(l.first().cloned().unwrap_or(Value::Nothing)),
                        Some(v) => Err(RuntimeError {
                            message: format!("first() requires list, got {}", v.type_name()),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                        None => Err(RuntimeError {
                            message: "first() requires an argument".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "last" => match arg_vals.first() {
                        Some(Value::List(l)) => Ok(l.last().cloned().unwrap_or(Value::Nothing)),
                        Some(v) => Err(RuntimeError {
                            message: format!("last() requires list, got {}", v.type_name()),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                        None => Err(RuntimeError {
                            message: "last() requires an argument".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "floor" => match arg_vals.first() {
                        Some(Value::Dec(d)) => Ok(Value::Num(*d as i64)),
                        Some(v) => Err(RuntimeError {
                            message: format!("floor() requires dec, got {}", v.type_name()),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                        None => Err(RuntimeError {
                            message: "floor() requires an argument".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "ceil" => match arg_vals.first() {
                        Some(Value::Dec(d)) => Ok(Value::Num(d.ceil() as i64)),
                        Some(v) => Err(RuntimeError {
                            message: format!("ceil() requires dec, got {}", v.type_name()),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                        None => Err(RuntimeError {
                            message: "ceil() requires an argument".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "round" => match arg_vals.first() {
                        Some(Value::Dec(d)) => Ok(Value::Num(d.round() as i64)),
                        Some(v) => Err(RuntimeError {
                            message: format!("round() requires dec, got {}", v.type_name()),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                        None => Err(RuntimeError {
                            message: "round() requires an argument".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "abs" => match arg_vals.first() {
                        Some(Value::Num(n)) => Ok(Value::Num(n.abs())),
                        Some(Value::Dec(d)) => Ok(Value::Dec(d.abs())),
                        Some(v) => Err(RuntimeError {
                            message: format!("abs() requires num or dec, got {}", v.type_name()),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                        None => Err(RuntimeError {
                            message: "abs() requires an argument".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "min" => {
                        if arg_vals.len() < 2 {
                            return Err(RuntimeError {
                                message: "min() requires at least 2 arguments".to_string(),
                                span: SourceSpan::from(span.start..span.end),
                            });
                        }
                        let mut min = arg_vals[0].clone();
                        for val in &arg_vals[1..] {
                            match (&min, val) {
                                (Value::Num(a), Value::Num(b)) => {
                                    if b < a {
                                        min = val.clone();
                                    }
                                }
                                (Value::Dec(a), Value::Dec(b)) => {
                                    if b < a {
                                        min = val.clone();
                                    }
                                }
                                _ => {}
                            }
                        }
                        Ok(min)
                    }
                    "max" => {
                        if arg_vals.len() < 2 {
                            return Err(RuntimeError {
                                message: "max() requires at least 2 arguments".to_string(),
                                span: SourceSpan::from(span.start..span.end),
                            });
                        }
                        let mut max = arg_vals[0].clone();
                        for val in &arg_vals[1..] {
                            match (&max, val) {
                                (Value::Num(a), Value::Num(b)) => {
                                    if b > a {
                                        max = val.clone();
                                    }
                                }
                                (Value::Dec(a), Value::Dec(b)) => {
                                    if b > a {
                                        max = val.clone();
                                    }
                                }
                                _ => {}
                            }
                        }
                        Ok(max)
                    }
                    // === String operations ===
                    "split" => match (arg_vals.first(), arg_vals.get(1)) {
                        (Some(Value::Text(s)), Some(Value::Text(sep))) => Ok(Value::List(
                            s.split(sep.as_str())
                                .map(|p| Value::Text(p.to_string()))
                                .collect(),
                        )),
                        _ => Err(RuntimeError {
                            message: "split(text, text) -> list[text]".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "join" => match (arg_vals.first(), arg_vals.get(1)) {
                        (Some(Value::List(l)), Some(Value::Text(sep))) => {
                            let parts: Vec<String> = l.iter().map(|v| v.to_string()).collect();
                            Ok(Value::Text(parts.join(sep)))
                        }
                        _ => Err(RuntimeError {
                            message: "join(list, text) -> text".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "replace" => match (arg_vals.first(), arg_vals.get(1), arg_vals.get(2)) {
                        (Some(Value::Text(s)), Some(Value::Text(from)), Some(Value::Text(to))) => {
                            Ok(Value::Text(s.replace(from.as_str(), to.as_str())))
                        }
                        _ => Err(RuntimeError {
                            message: "replace(text, text, text) -> text".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "contains" => match (arg_vals.first(), arg_vals.get(1)) {
                        (Some(Value::Text(s)), Some(Value::Text(needle))) => {
                            Ok(Value::Bool(s.contains(needle.as_str())))
                        }
                        (Some(Value::List(l)), Some(v)) => {
                            Ok(Value::Bool(l.iter().any(|item| value_eq(item, v))))
                        }
                        _ => Err(RuntimeError {
                            message: "contains(text|list, text|any) -> bool".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "starts_with" => match (arg_vals.first(), arg_vals.get(1)) {
                        (Some(Value::Text(s)), Some(Value::Text(prefix))) => {
                            Ok(Value::Bool(s.starts_with(prefix.as_str())))
                        }
                        _ => Err(RuntimeError {
                            message: "starts_with(text, text) -> bool".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "ends_with" => match (arg_vals.first(), arg_vals.get(1)) {
                        (Some(Value::Text(s)), Some(Value::Text(suffix))) => {
                            Ok(Value::Bool(s.ends_with(suffix.as_str())))
                        }
                        _ => Err(RuntimeError {
                            message: "ends_with(text, text) -> bool".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "slice" => match (arg_vals.first(), arg_vals.get(1), arg_vals.get(2)) {
                        (Some(Value::Text(s)), Some(Value::Num(start)), Some(Value::Num(end))) => {
                            let chars: Vec<char> = s.chars().collect();
                            let s_idx = (*start as usize).min(chars.len());
                            let e_idx = (*end as usize).min(chars.len());
                            Ok(Value::Text(chars[s_idx..e_idx].iter().collect()))
                        }
                        (Some(Value::List(l)), Some(Value::Num(start)), Some(Value::Num(end))) => {
                            let s_idx = (*start as usize).min(l.len());
                            let e_idx = (*end as usize).min(l.len());
                            Ok(Value::List(l[s_idx..e_idx].to_vec()))
                        }
                        _ => Err(RuntimeError {
                            message: "slice(text|list, num, num)".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    // === Math ===
                    "sqrt" => match arg_vals.first() {
                        Some(Value::Dec(d)) => Ok(Value::Dec(d.sqrt())),
                        Some(Value::Num(n)) => Ok(Value::Dec((*n as f64).sqrt())),
                        _ => Err(RuntimeError {
                            message: "sqrt(num|dec) -> dec".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "pow" => match (arg_vals.first(), arg_vals.get(1)) {
                        (Some(Value::Num(base)), Some(Value::Num(exp))) => {
                            Ok(Value::Num(base.wrapping_pow(*exp as u32)))
                        }
                        (Some(Value::Dec(base)), Some(Value::Dec(exp))) => {
                            Ok(Value::Dec(base.powf(*exp)))
                        }
                        (Some(Value::Num(base)), Some(Value::Dec(exp))) => {
                            Ok(Value::Dec((*base as f64).powf(*exp)))
                        }
                        _ => Err(RuntimeError {
                            message: "pow(num, num) -> num".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "log" => match (arg_vals.first(), arg_vals.get(1)) {
                        (Some(Value::Dec(v)), Some(Value::Dec(base))) => {
                            Ok(Value::Dec(v.log(*base)))
                        }
                        (Some(Value::Num(v)), Some(Value::Num(base))) => {
                            Ok(Value::Dec((*v as f64).log(*base as f64)))
                        }
                        _ => Err(RuntimeError {
                            message: "log(dec, dec) -> dec".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "sin" => match arg_vals.first() {
                        Some(Value::Dec(d)) => Ok(Value::Dec(d.sin())),
                        Some(Value::Num(n)) => Ok(Value::Dec((*n as f64).sin())),
                        _ => Err(RuntimeError {
                            message: "sin(dec) -> dec".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "cos" => match arg_vals.first() {
                        Some(Value::Dec(d)) => Ok(Value::Dec(d.cos())),
                        Some(Value::Num(n)) => Ok(Value::Dec((*n as f64).cos())),
                        _ => Err(RuntimeError {
                            message: "cos(dec) -> dec".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "tan" => match arg_vals.first() {
                        Some(Value::Dec(d)) => Ok(Value::Dec(d.tan())),
                        Some(Value::Num(n)) => Ok(Value::Dec((*n as f64).tan())),
                        _ => Err(RuntimeError {
                            message: "tan(dec) -> dec".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "clamp" => match (arg_vals.first(), arg_vals.get(1), arg_vals.get(2)) {
                        (Some(Value::Num(v)), Some(Value::Num(lo)), Some(Value::Num(hi))) => {
                            Ok(Value::Num((*v).max(*lo).min(*hi)))
                        }
                        (Some(Value::Dec(v)), Some(Value::Dec(lo)), Some(Value::Dec(hi))) => {
                            Ok(Value::Dec(v.clamp(*lo, *hi)))
                        }
                        _ => Err(RuntimeError {
                            message: "clamp(num, num, num) -> num".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    // === List operations ===
                    "rest" => match arg_vals.first() {
                        Some(Value::List(l)) => Ok(Value::List(if l.is_empty() {
                            vec![]
                        } else {
                            l[1..].to_vec()
                        })),
                        _ => Err(RuntimeError {
                            message: "rest(list) -> list".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "sort" => match arg_vals.first() {
                        Some(Value::List(l)) => {
                            let mut sorted = l.clone();
                            sorted.sort_by(|a, b| match (a, b) {
                                (Value::Num(x), Value::Num(y)) => x.cmp(y),
                                (Value::Dec(x), Value::Dec(y)) => {
                                    x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal)
                                }
                                (Value::Text(x), Value::Text(y)) => x.cmp(y),
                                _ => std::cmp::Ordering::Equal,
                            });
                            Ok(Value::List(sorted))
                        }
                        _ => Err(RuntimeError {
                            message: "sort(list) -> list".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "reverse" => match arg_vals.first() {
                        Some(Value::List(l)) => {
                            let mut rev = l.clone();
                            rev.reverse();
                            Ok(Value::List(rev))
                        }
                        _ => Err(RuntimeError {
                            message: "reverse(list) -> list".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "take" => match (arg_vals.first(), arg_vals.get(1)) {
                        (Some(Value::List(l)), Some(Value::Num(n))) => {
                            Ok(Value::List(l.iter().take(*n as usize).cloned().collect()))
                        }
                        _ => Err(RuntimeError {
                            message: "take(list, num) -> list".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "drop" => match (arg_vals.first(), arg_vals.get(1)) {
                        (Some(Value::List(l)), Some(Value::Num(n))) => {
                            Ok(Value::List(l.iter().skip(*n as usize).cloned().collect()))
                        }
                        _ => Err(RuntimeError {
                            message: "drop(list, num) -> list".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "append" => match (arg_vals.first(), arg_vals.get(1)) {
                        (Some(Value::List(l)), Some(v)) => {
                            let mut new_list = l.clone();
                            new_list.push(v.clone());
                            Ok(Value::List(new_list))
                        }
                        _ => Err(RuntimeError {
                            message: "append(list, T) -> list".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "prepend" => match (arg_vals.first(), arg_vals.get(1)) {
                        (Some(Value::List(l)), Some(v)) => {
                            let mut new_list = vec![v.clone()];
                            new_list.extend(l.iter().cloned());
                            Ok(Value::List(new_list))
                        }
                        _ => Err(RuntimeError {
                            message: "prepend(list, T) -> list".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "concat" => match (arg_vals.first(), arg_vals.get(1)) {
                        (Some(Value::List(a)), Some(Value::List(b))) => {
                            let mut result = a.clone();
                            result.extend(b.iter().cloned());
                            Ok(Value::List(result))
                        }
                        _ => Err(RuntimeError {
                            message: "concat(list, list) -> list".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "unique" => match arg_vals.first() {
                        Some(Value::List(l)) => {
                            let mut seen: Vec<Value> = Vec::new();
                            for item in l {
                                if !seen.iter().any(|s| value_eq(s, item)) {
                                    seen.push(item.clone());
                                }
                            }
                            Ok(Value::List(seen))
                        }
                        _ => Err(RuntimeError {
                            message: "unique(list) -> list".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    // === Higher-order list functions ===
                    "each" => {
                        let items = match arg_vals.first() {
                            Some(Value::List(l)) => l.clone(),
                            _ => {
                                return Err(RuntimeError {
                                    message: "each(list, fn) -> list: first arg must be list"
                                        .to_string(),
                                    span: SourceSpan::from(span.start..span.end),
                                })
                            }
                        };
                        let func = match arg_vals.get(1) {
                            Some(v) => v.clone(),
                            None => {
                                return Err(RuntimeError {
                                    message: "each(list, fn) -> list: missing function argument"
                                        .to_string(),
                                    span: SourceSpan::from(span.start..span.end),
                                })
                            }
                        };
                        let mut result = Vec::with_capacity(items.len());
                        for item in items {
                            match &func {
                                Value::Lambda(params, body, closure_env) => {
                                    let mut local = closure_env.child();
                                    if let Some(p) = params.first() {
                                        local.set(p.clone(), item);
                                    }
                                    result.push(eval_expr(body, &mut local)?);
                                }
                                _ => {
                                    return Err(RuntimeError {
                                        message: "each: second argument must be a function"
                                            .to_string(),
                                        span: SourceSpan::from(span.start..span.end),
                                    })
                                }
                            }
                        }
                        Ok(Value::List(result))
                    }
                    "keep" => {
                        let items = match arg_vals.first() {
                            Some(Value::List(l)) => l.clone(),
                            _ => {
                                return Err(RuntimeError {
                                    message: "keep(list, fn) -> list: first arg must be list"
                                        .to_string(),
                                    span: SourceSpan::from(span.start..span.end),
                                })
                            }
                        };
                        let func = match arg_vals.get(1) {
                            Some(v) => v.clone(),
                            None => {
                                return Err(RuntimeError {
                                    message: "keep(list, fn): missing function argument"
                                        .to_string(),
                                    span: SourceSpan::from(span.start..span.end),
                                })
                            }
                        };
                        let mut result = Vec::new();
                        for item in items {
                            let keep = match &func {
                                Value::Lambda(params, body, closure_env) => {
                                    let mut local = closure_env.child();
                                    if let Some(p) = params.first() {
                                        local.set(p.clone(), item.clone());
                                    }
                                    eval_expr(body, &mut local)?.is_truthy()
                                }
                                _ => {
                                    return Err(RuntimeError {
                                        message: "keep: second argument must be a function"
                                            .to_string(),
                                        span: SourceSpan::from(span.start..span.end),
                                    })
                                }
                            };
                            if keep {
                                result.push(item);
                            }
                        }
                        Ok(Value::List(result))
                    }
                    "find" => {
                        let items = match arg_vals.first() {
                            Some(Value::List(l)) => l.clone(),
                            _ => {
                                return Err(RuntimeError {
                                    message: "find(list, fn) -> maybe[T]: first arg must be list"
                                        .to_string(),
                                    span: SourceSpan::from(span.start..span.end),
                                })
                            }
                        };
                        let func = match arg_vals.get(1) {
                            Some(v) => v.clone(),
                            None => {
                                return Err(RuntimeError {
                                    message: "find(list, fn): missing function argument"
                                        .to_string(),
                                    span: SourceSpan::from(span.start..span.end),
                                })
                            }
                        };
                        for item in items {
                            let matched = match &func {
                                Value::Lambda(params, body, closure_env) => {
                                    let mut local = closure_env.child();
                                    if let Some(p) = params.first() {
                                        local.set(p.clone(), item.clone());
                                    }
                                    eval_expr(body, &mut local)?.is_truthy()
                                }
                                _ => {
                                    return Err(RuntimeError {
                                        message: "find: second argument must be a function"
                                            .to_string(),
                                        span: SourceSpan::from(span.start..span.end),
                                    })
                                }
                            };
                            if matched {
                                return Ok(item);
                            }
                        }
                        Ok(Value::Nothing)
                    }
                    "any" => {
                        let items = match arg_vals.first() {
                            Some(Value::List(l)) => l.clone(),
                            _ => {
                                return Err(RuntimeError {
                                    message: "any(list, fn) -> bool: first arg must be list"
                                        .to_string(),
                                    span: SourceSpan::from(span.start..span.end),
                                })
                            }
                        };
                        let func = match arg_vals.get(1) {
                            Some(v) => v.clone(),
                            None => {
                                return Err(RuntimeError {
                                    message: "any(list, fn): missing function argument".to_string(),
                                    span: SourceSpan::from(span.start..span.end),
                                })
                            }
                        };
                        for item in items {
                            let matched = match &func {
                                Value::Lambda(params, body, closure_env) => {
                                    let mut local = closure_env.child();
                                    if let Some(p) = params.first() {
                                        local.set(p.clone(), item);
                                    }
                                    eval_expr(body, &mut local)?.is_truthy()
                                }
                                _ => {
                                    return Err(RuntimeError {
                                        message: "any: second argument must be a function"
                                            .to_string(),
                                        span: SourceSpan::from(span.start..span.end),
                                    })
                                }
                            };
                            if matched {
                                return Ok(Value::Bool(true));
                            }
                        }
                        Ok(Value::Bool(false))
                    }
                    "all" => {
                        let items = match arg_vals.first() {
                            Some(Value::List(l)) => l.clone(),
                            _ => {
                                return Err(RuntimeError {
                                    message: "all(list, fn) -> bool: first arg must be list"
                                        .to_string(),
                                    span: SourceSpan::from(span.start..span.end),
                                })
                            }
                        };
                        let func = match arg_vals.get(1) {
                            Some(v) => v.clone(),
                            None => {
                                return Err(RuntimeError {
                                    message: "all(list, fn): missing function argument".to_string(),
                                    span: SourceSpan::from(span.start..span.end),
                                })
                            }
                        };
                        for item in items {
                            let matched = match &func {
                                Value::Lambda(params, body, closure_env) => {
                                    let mut local = closure_env.child();
                                    if let Some(p) = params.first() {
                                        local.set(p.clone(), item);
                                    }
                                    eval_expr(body, &mut local)?.is_truthy()
                                }
                                _ => {
                                    return Err(RuntimeError {
                                        message: "all: second argument must be a function"
                                            .to_string(),
                                        span: SourceSpan::from(span.start..span.end),
                                    })
                                }
                            };
                            if !matched {
                                return Ok(Value::Bool(false));
                            }
                        }
                        Ok(Value::Bool(true))
                    }
                    "fold" => {
                        let items = match arg_vals.first() {
                            Some(Value::List(l)) => l.clone(),
                            _ => {
                                return Err(RuntimeError {
                                    message: "fold(list, init, fn) -> T: first arg must be list"
                                        .to_string(),
                                    span: SourceSpan::from(span.start..span.end),
                                })
                            }
                        };
                        let mut acc = match arg_vals.get(1) {
                            Some(v) => v.clone(),
                            None => {
                                return Err(RuntimeError {
                                    message: "fold: missing initial value".to_string(),
                                    span: SourceSpan::from(span.start..span.end),
                                })
                            }
                        };
                        let func = match arg_vals.get(2) {
                            Some(v) => v.clone(),
                            None => {
                                return Err(RuntimeError {
                                    message: "fold: missing function argument".to_string(),
                                    span: SourceSpan::from(span.start..span.end),
                                })
                            }
                        };
                        for item in items {
                            match &func {
                                Value::Lambda(params, body, closure_env) => {
                                    let mut local = closure_env.child();
                                    if let Some(p0) = params.first() {
                                        local.set(p0.clone(), acc.clone());
                                    }
                                    if let Some(p1) = params.get(1) {
                                        local.set(p1.clone(), item);
                                    }
                                    acc = eval_expr(body, &mut local)?;
                                }
                                _ => {
                                    return Err(RuntimeError {
                                        message: "fold: third argument must be a function"
                                            .to_string(),
                                        span: SourceSpan::from(span.start..span.end),
                                    })
                                }
                            }
                        }
                        Ok(acc)
                    }
                    // === Map operations ===
                    "keys" => match arg_vals.first() {
                        Some(Value::Map(m)) => Ok(Value::List(
                            m.keys().map(|k| Value::Text(k.clone())).collect(),
                        )),
                        Some(Value::Struct(_, m)) => Ok(Value::List(
                            m.keys().map(|k| Value::Text(k.clone())).collect(),
                        )),
                        _ => Err(RuntimeError {
                            message: "keys(map) -> list[text]".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "values" => match arg_vals.first() {
                        Some(Value::Map(m)) => Ok(Value::List(m.values().cloned().collect())),
                        Some(Value::Struct(_, m)) => Ok(Value::List(m.values().cloned().collect())),
                        _ => Err(RuntimeError {
                            message: "values(map) -> list".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "has" => match (arg_vals.first(), arg_vals.get(1)) {
                        (Some(Value::Map(m)), Some(Value::Text(k))) => {
                            Ok(Value::Bool(m.contains_key(k)))
                        }
                        (Some(Value::Struct(_, m)), Some(Value::Text(k))) => {
                            Ok(Value::Bool(m.contains_key(k)))
                        }
                        _ => Err(RuntimeError {
                            message: "has(map, text) -> bool".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "get" => match (arg_vals.first(), arg_vals.get(1)) {
                        (Some(Value::Map(m)), Some(Value::Text(k))) => {
                            Ok(m.get(k).cloned().unwrap_or(Value::Nothing))
                        }
                        (Some(Value::Struct(_, m)), Some(Value::Text(k))) => {
                            Ok(m.get(k).cloned().unwrap_or(Value::Nothing))
                        }
                        _ => Err(RuntimeError {
                            message: "get(map, text) -> maybe[T]".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "set" => match (arg_vals.first(), arg_vals.get(1), arg_vals.get(2)) {
                        (Some(Value::Map(m)), Some(Value::Text(k)), Some(v)) => {
                            let mut new_map = m.clone();
                            new_map.insert(k.clone(), v.clone());
                            Ok(Value::Map(new_map))
                        }
                        _ => Err(RuntimeError {
                            message: "set(map, text, V) -> map".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "remove" => match (arg_vals.first(), arg_vals.get(1)) {
                        (Some(Value::Map(m)), Some(Value::Text(k))) => {
                            let mut new_map = m.clone();
                            new_map.remove(k);
                            Ok(Value::Map(new_map))
                        }
                        _ => Err(RuntimeError {
                            message: "remove(map, text) -> map".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "merge" => match (arg_vals.first(), arg_vals.get(1)) {
                        (Some(Value::Map(a)), Some(Value::Map(b))) => {
                            let mut result = a.clone();
                            result.extend(b.iter().map(|(k, v)| (k.clone(), v.clone())));
                            Ok(Value::Map(result))
                        }
                        _ => Err(RuntimeError {
                            message: "merge(map, map) -> map".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    // === Type checks ===
                    "is_num" => Ok(Value::Bool(matches!(arg_vals.first(), Some(Value::Num(_))))),
                    "is_text" => Ok(Value::Bool(matches!(
                        arg_vals.first(),
                        Some(Value::Text(_))
                    ))),
                    "is_list" => Ok(Value::Bool(matches!(
                        arg_vals.first(),
                        Some(Value::List(_))
                    ))),
                    "is_map" => Ok(Value::Bool(matches!(
                        arg_vals.first(),
                        Some(Value::Map(_)) | Some(Value::Struct(_, _))
                    ))),
                    "is_nothing" => Ok(Value::Bool(matches!(
                        arg_vals.first(),
                        Some(Value::Nothing) | None
                    ))),
                    "type_of" => Ok(Value::Text(
                        arg_vals
                            .first()
                            .map(|v| v.type_name())
                            .unwrap_or("nothing")
                            .to_string(),
                    )),
                    // === Time ===
                    "now" => Ok(Value::Num(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64,
                    )),
                    "today" => {
                        // Compute ISO date from Unix timestamp without chrono
                        let secs = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        let days_since_epoch = secs / 86400;
                        // Epoch is 1970-01-01
                        let mut year = 1970u64;
                        let mut days_left = days_since_epoch;
                        loop {
                            let days_in_year =
                                if year % 400 == 0 || (year % 4 == 0 && year % 100 != 0) {
                                    366
                                } else {
                                    365
                                };
                            if days_left < days_in_year {
                                break;
                            }
                            days_left -= days_in_year;
                            year += 1;
                        }
                        let leap = year % 400 == 0 || (year % 4 == 0 && year % 100 != 0);
                        let month_days: [u64; 12] = [
                            31,
                            if leap { 29 } else { 28 },
                            31,
                            30,
                            31,
                            30,
                            31,
                            31,
                            30,
                            31,
                            30,
                            31,
                        ];
                        let mut month = 1u64;
                        for &md in &month_days {
                            if days_left < md {
                                break;
                            }
                            days_left -= md;
                            month += 1;
                        }
                        Ok(Value::Text(format!(
                            "{:04}-{:02}-{:02}",
                            year,
                            month,
                            days_left + 1
                        )))
                    }
                    "sleep" => match arg_vals.first() {
                        Some(Value::Num(ms)) => {
                            std::thread::sleep(std::time::Duration::from_millis(*ms as u64));
                            Ok(Value::Nothing)
                        }
                        Some(Value::Dec(ms)) => {
                            std::thread::sleep(std::time::Duration::from_millis(*ms as u64));
                            Ok(Value::Nothing)
                        }
                        _ => Err(RuntimeError {
                            message: "sleep(num) -> nothing".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "format_time" => match (arg_vals.first(), arg_vals.get(1)) {
                        (Some(Value::Num(ts)), _) => {
                            // Return RFC 3339-ish string without external deps
                            let s = *ts as u64;
                            let secs_in_day = s % 86400;
                            let h = secs_in_day / 3600;
                            let m = (secs_in_day % 3600) / 60;
                            let sec = secs_in_day % 60;
                            Ok(Value::Text(format!(
                                "{}T{:02}:{:02}:{:02}Z",
                                s / 86400,
                                h,
                                m,
                                sec
                            )))
                        }
                        _ => Ok(Value::Text(String::new())),
                    },
                    "parse_time" => Ok(Value::Num(0)), // stub — requires date parsing library
                    // === JSON ===
                    "to_json" => match arg_vals.first() {
                        Some(v) => Ok(Value::Text(value_to_json(v))),
                        None => Ok(Value::Text("null".to_string())),
                    },
                    "from_json" => match arg_vals.first() {
                        Some(Value::Text(s)) => serde_json::from_str::<serde_json::Value>(s)
                            .map(json_to_value)
                            .map_err(|e| RuntimeError {
                                message: format!("JSON parse error: {}", e),
                                span: SourceSpan::from(span.start..span.end),
                            }),
                        _ => Err(RuntimeError {
                            message: "from_json(text) -> any".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    // === HTTP ===
                    "fetch_post" => match (arg_vals.first(), arg_vals.get(1)) {
                        (Some(Value::Text(url)), Some(body)) => {
                            let url = url.clone();
                            let body_json = value_to_json(body);
                            let client = Client::new();
                            let response = client
                                .post(&url)
                                .header(CONTENT_TYPE, "application/json")
                                .body(body_json)
                                .send()
                                .map_err(|e| RuntimeError {
                                    message: format!("fetch_post failed: {}", e),
                                    span: SourceSpan::from(span.start..span.end),
                                })?;
                            let text = response.text().map_err(|e| RuntimeError {
                                message: format!("fetch_post body error: {}", e),
                                span: SourceSpan::from(span.start..span.end),
                            })?;
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                                Ok(json_to_value(json))
                            } else {
                                Ok(Value::Text(text))
                            }
                        }
                        _ => Err(RuntimeError {
                            message: "fetch_post(text, any) -> any".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "encode_url" => match arg_vals.first() {
                        Some(Value::Text(s)) => {
                            let encoded: String = s
                                .chars()
                                .flat_map(|c| {
                                    if c.is_ascii_alphanumeric() || "-_.~".contains(c) {
                                        vec![c]
                                    } else {
                                        format!("%{:02X}", c as u32).chars().collect()
                                    }
                                })
                                .collect();
                            Ok(Value::Text(encoded))
                        }
                        _ => Err(RuntimeError {
                            message: "encode_url(text) -> text".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "decode_url" => match arg_vals.first() {
                        Some(Value::Text(s)) => {
                            let mut result = String::with_capacity(s.len());
                            let mut chars = s.chars().peekable();
                            while let Some(c) = chars.next() {
                                if c == '%' {
                                    let h1 = chars.next().unwrap_or('0');
                                    let h2 = chars.next().unwrap_or('0');
                                    if let Ok(byte) =
                                        u8::from_str_radix(&format!("{}{}", h1, h2), 16)
                                    {
                                        result.push(byte as char);
                                    } else {
                                        result.push('%');
                                        result.push(h1);
                                        result.push(h2);
                                    }
                                } else if c == '+' {
                                    result.push(' ');
                                } else {
                                    result.push(c);
                                }
                            }
                            Ok(Value::Text(result))
                        }
                        _ => Err(RuntimeError {
                            message: "decode_url(text) -> text".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    // === Crypto / random ===
                    "random" => {
                        use std::collections::hash_map::DefaultHasher;
                        use std::hash::{Hash, Hasher};
                        let mut h = DefaultHasher::new();
                        std::time::SystemTime::now().hash(&mut h);
                        // Thread id adds additional entropy
                        std::thread::current().id().hash(&mut h);
                        Ok(Value::Dec((h.finish() as f64) / (u64::MAX as f64)))
                    }
                    "random_int" => match (arg_vals.first(), arg_vals.get(1)) {
                        (Some(Value::Num(lo)), Some(Value::Num(hi))) => {
                            use std::collections::hash_map::DefaultHasher;
                            use std::hash::{Hash, Hasher};
                            let mut h = DefaultHasher::new();
                            std::time::SystemTime::now().hash(&mut h);
                            std::thread::current().id().hash(&mut h);
                            let range = (*hi - *lo).max(1) as u64;
                            Ok(Value::Num(*lo + (h.finish() % range) as i64))
                        }
                        _ => Err(RuntimeError {
                            message: "random_int(num, num) -> num".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "uuid" => {
                        use std::collections::hash_map::DefaultHasher;
                        use std::hash::{Hash, Hasher};
                        let mut h1 = DefaultHasher::new();
                        std::time::SystemTime::now().hash(&mut h1);
                        std::thread::current().id().hash(&mut h1);
                        let lo = h1.finish();
                        let mut h2 = DefaultHasher::new();
                        lo.hash(&mut h2);
                        (lo ^ 0xdeadbeef_cafebabe).hash(&mut h2);
                        let hi = h2.finish();
                        Ok(Value::Text(format!(
                            "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
                            (lo >> 32) as u32,
                            (lo >> 16) as u16,
                            lo as u16 & 0x0fff,
                            (hi >> 48) as u16 & 0x3fff | 0x8000,
                            hi & 0x0000_ffff_ffff_ffff
                        )))
                    }
                    "hash" => match arg_vals.first() {
                        Some(Value::Text(s)) => {
                            use std::collections::hash_map::DefaultHasher;
                            use std::hash::{Hash, Hasher};
                            let mut h = DefaultHasher::new();
                            s.hash(&mut h);
                            Ok(Value::Text(format!("{:016x}", h.finish())))
                        }
                        Some(v) => {
                            use std::collections::hash_map::DefaultHasher;
                            use std::hash::{Hash, Hasher};
                            let mut h = DefaultHasher::new();
                            v.to_string().hash(&mut h);
                            Ok(Value::Text(format!("{:016x}", h.finish())))
                        }
                        None => Err(RuntimeError {
                            message: "hash(text) -> text".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "hash_file" => match arg_vals.first() {
                        Some(Value::Text(path)) => {
                            let contents =
                                std::fs::read_to_string(path).map_err(|e| RuntimeError {
                                    message: format!("hash_file: {}", e),
                                    span: SourceSpan::from(span.start..span.end),
                                })?;
                            use std::collections::hash_map::DefaultHasher;
                            use std::hash::{Hash, Hasher};
                            let mut h = DefaultHasher::new();
                            contents.hash(&mut h);
                            Ok(Value::Text(format!("{:016x}", h.finish())))
                        }
                        _ => Err(RuntimeError {
                            message: "hash_file(text) -> text".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    // === Environment ===
                    "env" => match arg_vals.first() {
                        Some(Value::Text(k)) => {
                            Ok(std::env::var(k).map(Value::Text).unwrap_or(Value::Nothing))
                        }
                        _ => Err(RuntimeError {
                            message: "env(text) -> maybe[text]".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "env_or" => match (arg_vals.first(), arg_vals.get(1)) {
                        (Some(Value::Text(k)), Some(Value::Text(default))) => Ok(Value::Text(
                            std::env::var(k).unwrap_or_else(|_| default.clone()),
                        )),
                        _ => Err(RuntimeError {
                            message: "env_or(text, text) -> text".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "env_set" => match (arg_vals.first(), arg_vals.get(1)) {
                        (Some(Value::Text(k)), Some(Value::Text(v))) => {
                            std::env::set_var(k, v);
                            Ok(Value::Nothing)
                        }
                        _ => Err(RuntimeError {
                            message: "env_set(text, text) -> nothing".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    // === Filesystem ===
                    "read_file" => match arg_vals.first() {
                        Some(Value::Text(path)) => std::fs::read_to_string(path)
                            .map(Value::Text)
                            .map_err(|e| RuntimeError {
                                message: format!("read_file: {}", e),
                                span: SourceSpan::from(span.start..span.end),
                            }),
                        _ => Err(RuntimeError {
                            message: "read_file(text) -> text".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "write_file" => match (arg_vals.first(), arg_vals.get(1)) {
                        (Some(Value::Text(path)), Some(Value::Text(content))) => {
                            std::fs::write(path, content).map_err(|e| RuntimeError {
                                message: format!("write_file: {}", e),
                                span: SourceSpan::from(span.start..span.end),
                            })?;
                            Ok(Value::Nothing)
                        }
                        _ => Err(RuntimeError {
                            message: "write_file(text, text) -> nothing".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "file_exists" => match arg_vals.first() {
                        Some(Value::Text(path)) => {
                            Ok(Value::Bool(std::path::Path::new(path).exists()))
                        }
                        _ => Err(RuntimeError {
                            message: "file_exists(text) -> bool".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    "list_dir" => match arg_vals.first() {
                        Some(Value::Text(path)) => {
                            let entries = std::fs::read_dir(path)
                                .map_err(|e| RuntimeError {
                                    message: format!("list_dir: {}", e),
                                    span: SourceSpan::from(span.start..span.end),
                                })?
                                .filter_map(|e| e.ok())
                                .map(|e| Value::Text(e.file_name().to_string_lossy().to_string()))
                                .collect();
                            Ok(Value::List(entries))
                        }
                        _ => Err(RuntimeError {
                            message: "list_dir(text) -> list[text]".to_string(),
                            span: SourceSpan::from(span.start..span.end),
                        }),
                    },
                    // === I/O ===
                    "input" => {
                        if let Some(Value::Text(prompt)) = arg_vals.first() {
                            print!("{}", prompt);
                            use std::io::Write;
                            std::io::stdout().flush().ok();
                        }
                        let mut line = String::new();
                        std::io::stdin()
                            .read_line(&mut line)
                            .map_err(|e| RuntimeError {
                                message: format!("input: {}", e),
                                span: SourceSpan::from(span.start..span.end),
                            })?;
                        Ok(Value::Text(
                            line.trim_end_matches('\n')
                                .trim_end_matches('\r')
                                .to_string(),
                        ))
                    }
                    "exit" => {
                        let code = match arg_vals.first() {
                            Some(Value::Num(n)) => *n as i32,
                            _ => 0,
                        };
                        std::process::exit(code);
                    }
                    _ => {
                        // Look up in environment for user-defined functions
                        let func_val = env.get(name).ok_or_else(|| RuntimeError {
                            message: format!("Unknown function: {}", name),
                            span: SourceSpan::from(span.start..span.end),
                        })?;

                        match func_val {
                            Value::Lambda(params, body, closure_env) => {
                                let mut local = closure_env.child();
                                for (i, param) in params.iter().enumerate() {
                                    let val = arg_vals.get(i).cloned().unwrap_or(Value::Nothing);
                                    local.set(param.clone(), val);
                                }
                                eval_expr(&body, &mut local)
                            }
                            _ => Err(RuntimeError {
                                message: format!("'{}' is not a function", name),
                                span: SourceSpan::from(span.start..span.end),
                            }),
                        }
                    }
                }
            } else {
                // Complex function expression
                let func_val = eval_expr(func, env)?;
                match func_val {
                    Value::Lambda(params, body, closure_env) => {
                        let mut local = closure_env.child();
                        for (i, param) in params.iter().enumerate() {
                            let val = arg_vals.get(i).cloned().unwrap_or(Value::Nothing);
                            local.set(param.clone(), val);
                        }
                        eval_expr(&body, &mut local)
                    }
                    _ => Err(RuntimeError {
                        message: "Value is not callable".to_string(),
                        span: SourceSpan::from(span.start..span.end),
                    }),
                }
            }
        }

        Expr::Pipe { left, right, span } => {
            let left_val = eval_expr(left, env)?;

            // Pipe: left |> right
            // If right is a function, call it with left as argument
            // If right is a function call, insert left as first argument
            match right.as_ref() {
                Expr::Call { func, args, .. } => {
                    // Insert left value as first argument
                    let mut new_args = vec![Expr::Ident("".to_string(), Span::empty())];
                    for arg in args {
                        new_args.push(arg.clone());
                    }

                    let _new_call = Expr::Call {
                        func: func.clone(),
                        args: new_args.into_iter().skip(1).collect(),
                        span: *span,
                    };

                    // Actually: call the function with left_val
                    if let Expr::Ident(name, _) = func.as_ref() {
                        let arg_vals = [left_val];
                        // Reuse call logic
                        match name.as_str() {
                            "upper" | "lower" | "trim" | "first" | "last" | "len" | "abs"
                            | "floor" | "ceil" | "round" => {
                                // Re-evaluate with the piped value
                                let mut temp_env = env.child();
                                temp_env.set("$pipe".to_string(), arg_vals[0].clone());
                                eval_expr(right, env)
                            }
                            _ => eval_expr(right, env),
                        }
                    } else {
                        eval_expr(right, env)
                    }
                }
                Expr::Ident(name, _) => {
                    // Call the function with left as argument
                    let call = Expr::Call {
                        func: Box::new(Expr::Ident(name.clone(), Span::empty())),
                        args: vec![Expr::Ident("$pipe".to_string(), Span::empty())],
                        span: *span,
                    };

                    // Set up environment with piped value
                    let mut pipe_env = env.child();
                    pipe_env.set("$pipe".to_string(), left_val);
                    eval_expr(&call, &mut pipe_env)
                }
                _ => eval_expr(right, env),
            }
        }

        Expr::BinOp {
            left,
            op,
            right,
            span,
        } => {
            let l = eval_expr(left, env)?;
            let r = eval_expr(right, env)?;

            match op {
                BinOp::Add => match (&l, &r) {
                    (Value::Num(a), Value::Num(b)) => Ok(Value::Num(a + b)),
                    (Value::Dec(a), Value::Dec(b)) => Ok(Value::Dec(a + b)),
                    (Value::Text(a), Value::Text(b)) => Ok(Value::Text(format!("{}{}", a, b))),
                    _ => Err(RuntimeError {
                        message: format!("Cannot add {} and {}", l.type_name(), r.type_name()),
                        span: SourceSpan::from(span.start..span.end),
                    }),
                },
                BinOp::Sub => match (&l, &r) {
                    (Value::Num(a), Value::Num(b)) => Ok(Value::Num(a - b)),
                    (Value::Dec(a), Value::Dec(b)) => Ok(Value::Dec(a - b)),
                    _ => Err(RuntimeError {
                        message: format!(
                            "Cannot subtract {} from {}",
                            r.type_name(),
                            l.type_name()
                        ),
                        span: SourceSpan::from(span.start..span.end),
                    }),
                },
                BinOp::Mul => match (&l, &r) {
                    (Value::Num(a), Value::Num(b)) => Ok(Value::Num(a * b)),
                    (Value::Dec(a), Value::Dec(b)) => Ok(Value::Dec(a * b)),
                    _ => Err(RuntimeError {
                        message: format!("Cannot multiply {} and {}", l.type_name(), r.type_name()),
                        span: SourceSpan::from(span.start..span.end),
                    }),
                },
                BinOp::Div => match (&l, &r) {
                    (Value::Num(a), Value::Num(b)) => {
                        if *b == 0 {
                            return Err(RuntimeError {
                                message: "Division by zero".to_string(),
                                span: SourceSpan::from(span.start..span.end),
                            });
                        }
                        Ok(Value::Num(a / b))
                    }
                    (Value::Dec(a), Value::Dec(b)) => {
                        if *b == 0.0 {
                            return Err(RuntimeError {
                                message: "Division by zero".to_string(),
                                span: SourceSpan::from(span.start..span.end),
                            });
                        }
                        Ok(Value::Dec(a / b))
                    }
                    _ => Err(RuntimeError {
                        message: format!("Cannot divide {} by {}", l.type_name(), r.type_name()),
                        span: SourceSpan::from(span.start..span.end),
                    }),
                },
                BinOp::Mod => match (&l, &r) {
                    (Value::Num(a), Value::Num(b)) => {
                        if *b == 0 {
                            return Err(RuntimeError {
                                message: "Modulo by zero".to_string(),
                                span: SourceSpan::from(span.start..span.end),
                            });
                        }
                        Ok(Value::Num(a % b))
                    }
                    _ => Err(RuntimeError {
                        message: format!("Cannot modulo {} by {}", l.type_name(), r.type_name()),
                        span: SourceSpan::from(span.start..span.end),
                    }),
                },
                BinOp::Eq => Ok(Value::Bool(value_eq(&l, &r))),
                BinOp::NotEq => Ok(Value::Bool(!value_eq(&l, &r))),
                BinOp::Lt => match (&l, &r) {
                    (Value::Num(a), Value::Num(b)) => Ok(Value::Bool(a < b)),
                    (Value::Dec(a), Value::Dec(b)) => Ok(Value::Bool(a < b)),
                    (Value::Text(a), Value::Text(b)) => Ok(Value::Bool(a < b)),
                    _ => Err(RuntimeError {
                        message: format!("Cannot compare {} and {}", l.type_name(), r.type_name()),
                        span: SourceSpan::from(span.start..span.end),
                    }),
                },
                BinOp::Gt => match (&l, &r) {
                    (Value::Num(a), Value::Num(b)) => Ok(Value::Bool(a > b)),
                    (Value::Dec(a), Value::Dec(b)) => Ok(Value::Bool(a > b)),
                    (Value::Text(a), Value::Text(b)) => Ok(Value::Bool(a > b)),
                    _ => Err(RuntimeError {
                        message: format!("Cannot compare {} and {}", l.type_name(), r.type_name()),
                        span: SourceSpan::from(span.start..span.end),
                    }),
                },
                BinOp::LtEq => match (&l, &r) {
                    (Value::Num(a), Value::Num(b)) => Ok(Value::Bool(a <= b)),
                    (Value::Dec(a), Value::Dec(b)) => Ok(Value::Bool(a <= b)),
                    (Value::Text(a), Value::Text(b)) => Ok(Value::Bool(a <= b)),
                    _ => Err(RuntimeError {
                        message: format!("Cannot compare {} and {}", l.type_name(), r.type_name()),
                        span: SourceSpan::from(span.start..span.end),
                    }),
                },
                BinOp::GtEq => match (&l, &r) {
                    (Value::Num(a), Value::Num(b)) => Ok(Value::Bool(a >= b)),
                    (Value::Dec(a), Value::Dec(b)) => Ok(Value::Bool(a >= b)),
                    (Value::Text(a), Value::Text(b)) => Ok(Value::Bool(a >= b)),
                    _ => Err(RuntimeError {
                        message: format!("Cannot compare {} and {}", l.type_name(), r.type_name()),
                        span: SourceSpan::from(span.start..span.end),
                    }),
                },
                BinOp::And => Ok(Value::Bool(l.is_truthy() && r.is_truthy())),
                BinOp::Or => Ok(Value::Bool(l.is_truthy() || r.is_truthy())),
                _ => Err(RuntimeError {
                    message: "Unknown binary operator".to_string(),
                    span: SourceSpan::from(span.start..span.end),
                }),
            }
        }

        Expr::Not { expr, span: _ } => {
            let val = eval_expr(expr, env)?;
            Ok(Value::Bool(!val.is_truthy()))
        }

        Expr::Neg { expr, span } => {
            let val = eval_expr(expr, env)?;
            match val {
                Value::Num(n) => Ok(Value::Num(-n)),
                Value::Dec(d) => Ok(Value::Dec(-d)),
                _ => Err(RuntimeError {
                    message: format!("Cannot negate {}", val.type_name()),
                    span: SourceSpan::from(span.start..span.end),
                }),
            }
        }

        Expr::Lambda {
            params,
            body,
            span: _,
        } => {
            // Create closure with current environment
            Ok(Value::Lambda(params.clone(), body.clone(), env.clone()))
        }

        Expr::Wait { expr, span: _ } => {
            // For interpreted mode, just evaluate synchronously
            eval_expr(expr, env)
        }

        Expr::Try { expr, span: _ } => match eval_expr(expr, env) {
            Ok(val) => Ok(val),
            Err(_) => Ok(Value::Nothing),
        },

        Expr::With { base, fields, span } => {
            let base_val = eval_expr(base, env)?;

            match base_val {
                Value::Struct(_, mut base_fields) | Value::Map(mut base_fields) => {
                    for (field_name, expr) in fields {
                        let val = eval_expr(expr, env)?;
                        base_fields.insert(field_name.clone(), val);
                    }
                    Ok(Value::Struct("".to_string(), base_fields))
                }
                _ => Err(RuntimeError {
                    message: format!("Cannot use 'with' on {}", base_val.type_name()),
                    span: SourceSpan::from(span.start..span.end),
                }),
            }
        }

        Expr::Env { name, span: _ } => {
            // In interpreted mode, return empty string or look up from actual env
            std::env::var(name).map(Value::Text).or(Ok(Value::Nothing))
        }

        Expr::Param { name, span: _ } => {
            // URL params not available in interpreted mode
            Ok(Value::Text(format!("<param {}>", name)))
        }

        _ => Ok(Value::Nothing),
    }
}
