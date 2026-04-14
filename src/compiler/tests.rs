use super::web::{generate_html_bootstrap, generate_wasm_source};
use crate::ast::*;
use crate::lexer::Span;

fn mk_counter_program() -> Program {
    Program {
        items: vec![Item::Screen(ScreenDef {
            name: "Counter".to_string(),
            state: vec![StateDecl {
                name: "count".to_string(),
                ty: None,
                default: Expr::Num(0, Span::new(0, 0)),
                span: Span::new(0, 0),
            }],
            on_load: None,
            children: vec![
                UiNode::Words(WordsNode {
                    content: Expr::Text(
                        vec![TextPart::Literal("Hello".to_string())],
                        Span::new(0, 0),
                    ),
                    props: TextProps::default(),
                    events: Vec::new(),
                    span: Span::new(0, 0),
                }),
                UiNode::Tap(TapNode {
                    label: Expr::Text(vec![TextPart::Literal("+".to_string())], Span::new(0, 0)),
                    action: vec![Stmt::Assign {
                        target: AssignTarget::Simple("count".to_string(), Span::new(0, 0)),
                        value: Expr::BinOp {
                            left: Box::new(Expr::Ident("count".to_string(), Span::new(0, 0))),
                            op: BinOp::Add,
                            right: Box::new(Expr::Num(1, Span::new(0, 0))),
                            span: Span::new(0, 0),
                        },
                        span: Span::new(0, 0),
                    }],
                    guard: None,
                    confirm: false,
                    props: BoxProps::default(),
                    span: Span::new(0, 0),
                }),
            ],
            thinks: Vec::new(),
            ghost: false,
            intents: Vec::new(),
            stream: None,
            span: Span::new(0, 0),
        })],
        span: Span::new(0, 0),
    }
}

fn mk_spring_program() -> Program {
    Program {
        items: vec![Item::Screen(ScreenDef {
            name: "Flow".to_string(),
            state: Vec::new(),
            on_load: None,
            children: vec![UiNode::Box(BoxNode {
                name: Some("cta".to_string()),
                props: BoxProps {
                    animate: Some(AnimSpec {
                        name: "spring".to_string(),
                        duration: None,
                        ease: None,
                        physics: Some(PhysicsSpec {
                            kind: PhysicsKind::Spring,
                            stiffness: Some(120.0),
                            damping: Some(14.0),
                            mass: Some(1.0),
                        }),
                        from: None,
                        to: None,
                    }),
                    ..Default::default()
                },
                events: Vec::new(),
                children: vec![],
                lazy: false,
                span: Span::new(0, 0),
            })],
            thinks: Vec::new(),
            ghost: false,
            intents: Vec::new(),
            stream: None,
            span: Span::new(0, 0),
        })],
        span: Span::new(0, 0),
    }
}

fn mk_safe_tap_program() -> Program {
    Program {
        items: vec![Item::Screen(ScreenDef {
            name: "Safe".to_string(),
            state: Vec::new(),
            on_load: None,
            children: vec![UiNode::Tap(TapNode {
                label: Expr::Text(
                    vec![TextPart::Literal("Delete".to_string())],
                    Span::new(0, 0),
                ),
                action: Vec::new(),
                guard: Some(Expr::Ident("canDelete".to_string(), Span::new(0, 0))),
                confirm: true,
                props: BoxProps::default(),
                span: Span::new(0, 0),
            })],
            thinks: Vec::new(),
            ghost: false,
            intents: Vec::new(),
            stream: None,
            span: Span::new(0, 0),
        })],
        span: Span::new(0, 0),
    }
}

// ─── HTML bootstrap tests ─────────────────────────────────────────────────────

#[test]
fn test_bootstrap_has_canvas_not_dom() {
    let program = mk_counter_program();
    let html = generate_html_bootstrap(&program);

    // WASM canvas approach — canvas is the rendering surface, no app-framework divs
    assert!(html.contains("<canvas"), "must have canvas element");
    assert!(html.contains("app.wasm"), "must fetch the WASM binary");
    assert!(
        !html.contains("<p>"),
        "must not generate DOM paragraph tags"
    );
    // A single diagnostic error overlay div is allowed; assert no framework content divs
    assert!(
        !html.contains("<div id=\"app\"") && !html.contains("<div id=\"root\""),
        "must not generate app-framework root divs"
    );
}

#[test]
fn test_bootstrap_wires_canvas_api_to_wasm() {
    let program = mk_counter_program();
    let html = generate_html_bootstrap(&program);

    // JS bridge must wire canvas 2D context methods as WASM imports
    assert!(html.contains("ved_fill_rect"), "must wire fill_rect");
    assert!(html.contains("ved_fill_text"), "must wire fill_text");
    assert!(html.contains("ved_set_fill"), "must wire set_fill");
    assert!(
        html.contains("ved_request_frame"),
        "must wire request_frame"
    );
    assert!(
        html.contains("WebAssembly.instantiate"),
        "must use WebAssembly.instantiate to load the WASM module"
    );
}

#[test]
fn test_bootstrap_exports_tap_and_resize() {
    let program = mk_counter_program();
    let html = generate_html_bootstrap(&program);

    // WASM exports used by the JS bridge
    assert!(
        html.contains("ved_tap") || html.contains("click"),
        "must wire tap events"
    );
    assert!(
        html.contains("ved_resize") || html.contains("resize"),
        "must wire resize"
    );
    assert!(
        html.contains("ved_frame") || html.contains("requestAnimationFrame"),
        "must wire frame"
    );
}

// ─── WASM source (generated Rust) tests ──────────────────────────────────────

#[test]
fn test_wasm_source_has_no_std() {
    let program = mk_counter_program();
    let src = generate_wasm_source(&program);

    assert!(
        src.contains("#![no_std]"),
        "generated WASM must be no_std — no stdlib overhead"
    );
    assert!(
        src.contains("extern crate alloc"),
        "must use alloc crate for collections"
    );
}

#[test]
fn test_wasm_source_generates_state_for_remember() {
    let program = mk_counter_program();
    let src = generate_wasm_source(&program);

    // `remember count = 0` on screen Counter → counter_count field in AppState
    assert!(
        src.contains("counter_count"),
        "must generate state field for remember decl"
    );
    assert!(src.contains("AppState"), "must have AppState struct");
}

#[test]
fn test_wasm_source_generates_render_function() {
    let program = mk_counter_program();
    let src = generate_wasm_source(&program);

    assert!(
        src.contains("render_counter"),
        "must generate render_counter fn for screen Counter"
    );
    assert!(src.contains("ved_clear"), "render must clear canvas");
}

#[test]
fn test_wasm_source_generates_tap_dispatch() {
    let program = mk_counter_program();
    let src = generate_wasm_source(&program);

    // tap "+" registers a hit-test area and action
    assert!(
        src.contains("dispatch"),
        "must have dispatch fn for tap actions"
    );
    assert!(src.contains("ved_init"), "must export ved_init");
    assert!(src.contains("ved_tap"), "must export ved_tap");
}

#[test]
fn test_wasm_source_embeds_spring_physics() {
    let program = mk_spring_program();
    let src = generate_wasm_source(&program);

    // Spring physics struct must be in the generated WASM source
    assert!(
        src.contains("Spring") || src.contains("stiffness"),
        "must embed spring physics"
    );
}

#[test]
fn test_wasm_source_imports_canvas_api() {
    let program = mk_counter_program();
    let src = generate_wasm_source(&program);

    // extern "C" imports for browser canvas API
    assert!(
        src.contains("extern"),
        "must have extern block for JS imports"
    );
    assert!(
        src.contains("ved_fill_rect") || src.contains("fill_rect"),
        "must import fill_rect"
    );
}

#[test]
fn test_wasm_source_guard_renders_tap() {
    let program = mk_safe_tap_program();
    let src = generate_wasm_source(&program);

    // tap with a guard — it should still render the button
    assert!(
        src.contains("Delete") || src.contains("v_text"),
        "tap label must appear in render"
    );
}

#[test]
fn test_wasm_exports_are_no_mangle() {
    let program = mk_counter_program();
    let src = generate_wasm_source(&program);

    // All WASM exports need #[no_mangle] so the JS bridge can call them by name
    assert!(src.contains("#[no_mangle]"), "exports must be #[no_mangle]");
}
