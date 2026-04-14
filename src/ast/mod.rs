#![allow(dead_code)]
use crate::lexer::Span;

/// Program root node
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub items: Vec<Item>,
    pub span: Span,
}

/// Top-level program item
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// Type definition: shape Point { x: num, y: num }
    Shape(ShapeDef),
    /// Function definition: think add needs a: num, b: num -> num
    Think(ThinkDef),
    /// Screen (client UI): screen Main { ... }
    Screen(ScreenDef),
    /// Reusable UI component: piece Button needs label: text { ... }
    Piece(PieceDef),
    /// Route definitions: routes { "/" => Home, "/about" => About }
    Routes(RoutesDef),
    /// Server definition: serve Api { ... }
    Serve(ServeDef),
    /// Database definition: database main { ... }
    Database(DatabaseDef),
    /// Background task: task cleanup every "1h" { ... }
    Task(TaskDef),
    /// Job queue: queue emails { ... }
    Queue(QueueDef),
    /// WebSocket endpoint: live chat { ... }
    Live(LiveDef),
    /// Authentication config: auth jwt { ... }
    Auth(AuthDef),
    /// Global configuration
    Config(ConfigDef),
}

/// Shape/type definition
#[derive(Debug, Clone, PartialEq)]
pub struct ShapeDef {
    pub name: String,
    pub fields: Vec<FieldDef>,
    pub span: Span,
}

/// Field definition within a shape
#[derive(Debug, Clone, PartialEq)]
pub struct FieldDef {
    pub name: String,
    pub ty: TypeExpr,
    pub default: Option<Expr>,
    pub span: Span,
}

/// Function definition
#[derive(Debug, Clone, PartialEq)]
pub struct ThinkDef {
    pub name: String,
    pub params: Vec<Param>,
    pub ret: Option<TypeExpr>,
    pub is_async: bool,
    pub body: Vec<Stmt>,
    /// UI nodes declared in the think body (makes it a render-component)
    pub ui_children: Vec<UiNode>,
    pub span: Span,
}

/// Function parameter
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: TypeExpr,
    pub span: Span,
}

/// Type expressions
#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    Num,
    Dec,
    Text,
    Bool,
    /// List[T]
    List(Box<TypeExpr>),
    /// Map[K, V]
    Map(Box<TypeExpr>, Box<TypeExpr>),
    /// Maybe[T] - null safety
    Maybe(Box<TypeExpr>),
    /// Ok[T, E] - result type for error handling
    Ok(Box<TypeExpr>, Box<TypeExpr>),
    /// Named user type
    Named(String),
    Any,
    Nothing,
    /// Function type: fn(num, num) -> num
    Function(Vec<TypeExpr>, Box<TypeExpr>),
}

/// Statements
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// Variable declaration: let x = 10
    Let {
        name: String,
        mutable: bool,
        ty: Option<TypeExpr>,
        value: Expr,
        span: Span,
    },
    /// Reactive/local state declaration: remember count = 0
    Remember {
        name: String,
        ty: Option<TypeExpr>,
        value: Expr,
        computed: bool,
        dependencies: Vec<String>,
        span: Span,
    },
    /// Reactive fetch statement
    Fetch(FetchStmt),
    /// Variable assignment: x = 20
    Assign {
        target: AssignTarget,
        value: Expr,
        span: Span,
    },
    /// Expression statement
    Expr(Expr),
    /// Return value: give result
    Give {
        value: Expr,
        span: Span,
    },
    /// Return error: fail "error message"
    Fail {
        value: Expr,
        span: Span,
    },
    /// Pattern matching: when { ... }
    When {
        arms: Vec<WhenArm>,
        otherwise: Option<Box<Stmt>>,
        span: Span,
    },
    /// For-each loop: each item in items { ... }
    Each {
        var: String,
        iter: Expr,
        body: Vec<Stmt>,
        span: Span,
    },
    /// Database transaction
    Transaction {
        body: Vec<Stmt>,
        span: Span,
    },
    /// Event handlers
    OnLoad {
        body: Vec<Stmt>,
        span: Span,
    },
    OnKey {
        key: String,
        body: Vec<Stmt>,
        span: Span,
    },
    OnTap {
        body: Vec<Stmt>,
        span: Span,
    },
    OnHover {
        body: Vec<Stmt>,
        span: Span,
    },
    OnMessage {
        channel: String,
        body: Vec<Stmt>,
        span: Span,
    },
    OnConnect {
        name: String,
        body: Vec<Stmt>,
        span: Span,
    },
    OnLeave {
        name: String,
        body: Vec<Stmt>,
        span: Span,
    },
}

/// Assignment target (left-hand side of assignment)
#[derive(Debug, Clone, PartialEq)]
pub enum AssignTarget {
    /// Simple variable: x = 10
    Simple(String, Span),
    /// Field access: obj.field = value
    Field(Box<Expr>, String, Span),
    /// Index: arr[0] = value
    Index(Box<Expr>, Box<Expr>, Span),
}

/// When arm (pattern match branch)
#[derive(Debug, Clone, PartialEq)]
pub struct WhenArm {
    pub cond: Expr,
    pub body: Vec<Stmt>,
    pub span: Span,
}

/// Expressions
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Integer literal: 42
    Num(i64, Span),
    /// Decimal literal: 3.14
    Dec(f64, Span),
    /// String literal (with optional interpolation)
    Text(Vec<TextPart>, Span),
    /// Boolean: yes/no
    Bool(bool, Span),
    /// Color literal: #7f6fe8
    Color(String, Span),
    /// Null value
    Nothing(Span),
    /// Variable reference
    Ident(String, Span),
    /// List literal: [1, 2, 3]
    List(Vec<Expr>, Span),
    /// Map literal: {"a": 1, "b": 2}
    Map(Vec<(Expr, Expr)>, Span),
    /// Struct construction: Point { x: 10, y: 20 }
    Construct {
        name: String,
        fields: Vec<(String, Expr)>,
        span: Span,
    },
    /// Field access: obj.field
    Field {
        obj: Box<Expr>,
        field: String,
        span: Span,
    },
    /// Index access: arr[0]
    Index {
        obj: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    /// Function call: add(1, 2)
    Call {
        func: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
    /// Pipe operator: value |> func |> other
    Pipe {
        left: Box<Expr>,
        right: Box<Expr>,
        span: Span,
    },
    /// Binary operation: a + b
    BinOp {
        left: Box<Expr>,
        op: BinOp,
        right: Box<Expr>,
        span: Span,
    },
    /// Unary not: not condition
    Not { expr: Box<Expr>, span: Span },
    /// Negation: -x
    Neg { expr: Box<Expr>, span: Span },
    /// Lambda/anonymous function: x => x * 2
    Lambda {
        params: Vec<String>,
        body: Box<Expr>,
        span: Span,
    },
    /// Await expression: wait async_op
    Wait { expr: Box<Expr>, span: Span },
    /// Try expression: try might_fail
    Try { expr: Box<Expr>, span: Span },
    /// With expression (struct update): base with { field: value }
    With {
        base: Box<Expr>,
        fields: Vec<(String, Expr)>,
        span: Span,
    },
    /// HTTP fetch
    Fetch(FetchExpr),
    /// Error handling: handle expr ok x => ... fail e => ...
    Handle {
        expr: Box<Expr>,
        ok_arm: Option<HandleArm>,
        fail_arm: Option<HandleArm>,
        span: Span,
    },
    /// Environment variable access: env "HOME"
    Env { name: String, span: Span },
    /// URL parameter access: param "id"
    Param { name: String, span: Span },
    /// Database query
    DbQuery {
        sql: Box<Expr>,
        as_type: Option<TypeExpr>,
        span: Span,
    },
    /// Get all records: db all Users
    DbAll { table: String, span: Span },
    /// Get one record: db one Users id
    DbOne {
        table: String,
        key: Box<Expr>,
        span: Span,
    },
    /// Save record: db save Users record
    DbSave {
        table: String,
        record: Box<Expr>,
        span: Span,
    },
    /// Remove record: db remove Users id
    DbRemove {
        table: String,
        key: Box<Expr>,
        span: Span,
    },
}

/// Text parts for interpolated strings
#[derive(Debug, Clone, PartialEq)]
pub enum TextPart {
    /// Literal text segment
    Literal(String),
    /// Interpolated expression: {expr}
    Interp(Box<Expr>),
}

/// HTTP fetch expression
#[derive(Debug, Clone, PartialEq)]
pub struct FetchExpr {
    pub method: HttpMethod,
    pub url: Box<Expr>,
    pub body: Option<Box<Expr>>,
    pub ok_arm: Option<Vec<Stmt>>,
    pub fail_arm: Option<Vec<Stmt>>,
    pub span: Span,
}

/// Reactive fetch statement
#[derive(Debug, Clone, PartialEq)]
pub struct FetchStmt {
    pub target: String,
    pub url: Expr,
    pub when_deps: Vec<String>,
    pub cache_duration: Option<String>,
    pub loading_handler: Option<String>,
    pub error_handler: Option<String>,
    pub span: Span,
}

/// HTTP methods
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Del,
}

/// Handle arm for error handling
#[derive(Debug, Clone, PartialEq)]
pub struct HandleArm {
    pub binding: Option<String>,
    pub body: Box<Expr>,
    pub span: Span,
}

/// Binary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,   // +
    Sub,   // -
    Mul,   // *
    Div,   // /
    Mod,   // %
    Eq,    // ==
    NotEq, // !=
    Gt,    // >
    Lt,    // <
    GtEq,  // >=
    LtEq,  // <=
    And,   // and
    Or,    // or
    Is,    // is (type check)
    In,    // in (membership)
}

// === Client-side AST nodes ===

/// Screen definition (main UI container)
#[derive(Debug, Clone, PartialEq)]
pub struct ScreenDef {
    pub name: String,
    pub state: Vec<StateDecl>,
    pub on_load: Option<Vec<Stmt>>,
    pub children: Vec<UiNode>,
    pub thinks: Vec<ThinkDef>,
    /// Ghost mode: compiler-assisted intent capture and hints
    pub ghost: bool,
    /// Captured natural-language intent comments within screen scope
    pub intents: Vec<String>,
    /// Stream mode: server-side render with zero JS for static content
    pub stream: Option<bool>,
    pub span: Span,
}

/// State declaration: remember count = 0
#[derive(Debug, Clone, PartialEq)]
pub struct StateDecl {
    pub name: String,
    pub ty: Option<TypeExpr>,
    pub default: Expr,
    pub span: Span,
}

/// Piece definition (reusable component)
#[derive(Debug, Clone, PartialEq)]
pub struct PieceDef {
    pub name: String,
    pub needs: Vec<Param>,
    pub children: Vec<UiNode>,
    pub thinks: Vec<ThinkDef>,
    pub span: Span,
}

/// Routes definition
#[derive(Debug, Clone, PartialEq)]
pub struct RoutesDef {
    pub routes: Vec<Route>,
    pub otherwise: Option<String>,
    pub span: Span,
}

/// Single route
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub pattern: String,
    pub screen: String,
    pub span: Span,
}

/// UI node types
#[derive(Debug, Clone, PartialEq)]
pub enum UiNode {
    /// Box container: box main { ... }
    Box(BoxNode),
    /// Text element: words "Hello"
    Words(WordsNode),
    /// Button/tappable: tap "Click me" => action
    Tap(TapNode),
    /// Input field: field placeholder => binding
    Field(FieldNode),
    /// Image: image "url"
    Image(ImageNode),
    /// Component instance: SomePiece { prop: value }
    Piece(PieceCall),
    /// For-each in UI: each item in items { ... }
    Each(EachNode),
    /// Conditional show: show when condition { ... }
    ShowWhen(ShowWhenNode),
}

/// Box container node
#[derive(Debug, Clone, PartialEq)]
pub struct BoxNode {
    pub name: Option<String>,
    pub props: BoxProps,
    pub events: Vec<EventHandler>,
    pub children: Vec<UiNode>,
    /// Lazy hydration: only hydrate when visible (IntersectionObserver)
    pub lazy: bool,
    pub span: Span,
}

/// Box layout properties
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoxProps {
    pub fill: Option<Expr>,
    pub tall: Option<Expr>,
    pub flow: Option<FlowDir>,
    pub gap: Option<Expr>,
    pub padding: Option<PaddingExpr>,
    pub center: Option<CenterDir>,
    pub layer: Option<Expr>,
    pub backdrop: Option<Expr>,
    pub push: Option<PushDir>,
    pub color: Option<ColorExpr>,
    pub radius: Option<Expr>,
    pub border: Option<BorderExpr>,
    pub shadow: Option<ShadowExpr>,
    pub blur: Option<Expr>,
    pub opacity: Option<Expr>,
    pub clip: Option<bool>,
    pub scroll: Option<bool>,
    pub cursor: Option<String>,
    pub enter: Option<AnimSpec>,
    pub leave: Option<AnimSpec>,
    pub hover: Option<AnimSpec>,
    pub press: Option<AnimSpec>,
    pub anim_loop: Option<AnimSpec>,
    pub show: Option<Expr>,
    /// Flow: snap behavior for carousels/scrolling
    pub snap: Option<bool>,
    /// Flow: gesture-driven interactions
    pub gesture: Option<GestureSpec>,
    /// Flow: primary animation (shorthand for enter with physics)
    pub animate: Option<AnimSpec>,
}

/// Words (text) node
#[derive(Debug, Clone, PartialEq)]
pub struct WordsNode {
    pub content: Expr,
    pub props: TextProps,
    pub events: Vec<EventHandler>,
    pub span: Span,
}

/// Text properties
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextProps {
    pub size: Option<Expr>,
    pub weight: Option<TextWeight>,
    pub color: Option<ColorExpr>,
    pub align: Option<TextAlign>,
    pub spacing: Option<Expr>,
    pub lines: Option<Expr>,
    pub cut: Option<Expr>,
    pub fill: Option<Expr>,
}

/// Tap (button) node
#[derive(Debug, Clone, PartialEq)]
pub struct TapNode {
    pub label: Expr,
    pub action: Vec<Stmt>,
    /// Optional guard expression that must evaluate truthy before action executes
    pub guard: Option<Expr>,
    /// Whether to show a confirmation dialog before action executes
    pub confirm: bool,
    pub props: BoxProps,
    pub span: Span,
}

/// Input field node
#[derive(Debug, Clone, PartialEq)]
pub struct FieldNode {
    pub placeholder: Expr,
    pub bind: String,
    pub secret: bool,
    pub events: Vec<EventHandler>,
    pub props: BoxProps,
    pub span: Span,
}

/// Image node
#[derive(Debug, Clone, PartialEq)]
pub struct ImageNode {
    pub src: Expr,
    pub props: BoxProps,
    pub span: Span,
}

/// Piece (component) call
#[derive(Debug, Clone, PartialEq)]
pub struct PieceCall {
    pub name: String,
    pub props: Vec<(String, Expr)>,
    pub span: Span,
}

/// Each (loop) node
#[derive(Debug, Clone, PartialEq)]
pub struct EachNode {
    pub var: String,
    pub iter: Expr,
    pub children: Vec<UiNode>,
    pub span: Span,
}

/// ShowWhen (conditional) node
#[derive(Debug, Clone, PartialEq)]
pub struct ShowWhenNode {
    pub cond: Expr,
    pub children: Vec<UiNode>,
    pub span: Span,
}

/// Event handler
#[derive(Debug, Clone, PartialEq)]
pub struct EventHandler {
    pub kind: EventKind,
    pub body: Vec<Stmt>,
    pub span: Span,
}

/// Event types
#[derive(Debug, Clone, PartialEq)]
pub enum EventKind {
    Tap,
    Hover,
    Unhover,
    Change,
    Submit,
    Key(String),
    Scroll(ScrollCond),
}

/// Scroll condition
#[derive(Debug, Clone, PartialEq)]
pub struct ScrollCond {
    pub axis: char,
    pub op: BinOp,
    pub value: i64,
}

/// Layout flow direction
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlowDir {
    Down,
    Across,
    Wrap,
    Layer,
}

/// Center direction
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CenterDir {
    Across,
    Down,
    Both,
}

/// Push direction
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PushDir {
    Right,
    Left,
    Bottom,
}

/// Text weight
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextWeight {
    Fat,
    Normal,
    Thin,
}

/// Text alignment
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

/// Color expression
#[derive(Debug, Clone, PartialEq)]
pub enum ColorExpr {
    Hex(String),
    Named(String),
    Fade(String, String),
}

/// Padding expression
#[derive(Debug, Clone, PartialEq)]
pub struct PaddingExpr {
    pub top: i64,
    pub right: i64,
    pub bottom: i64,
    pub left: i64,
}

/// Border expression
#[derive(Debug, Clone, PartialEq)]
pub struct BorderExpr {
    pub width: i64,
    pub color: String,
}

/// Shadow expression
#[derive(Debug, Clone, PartialEq)]
pub struct ShadowExpr {
    pub blur: i64,
    pub color: String,
    pub x: i64,
    pub y: i64,
}

/// Animation specification with optional physics-based simulation
#[derive(Debug, Clone, PartialEq)]
pub struct AnimSpec {
    pub name: String,
    pub duration: Option<String>,
    pub ease: Option<String>,
    /// Physics-based animation (replaces duration-based)
    pub physics: Option<PhysicsSpec>,
    /// Transform state to animate from
    pub from: Option<AnimTransform>,
    /// Transform state to animate to
    pub to: Option<AnimTransform>,
}

/// Physics simulation parameters
#[derive(Debug, Clone, PartialEq)]
pub struct PhysicsSpec {
    pub kind: PhysicsKind,
    pub stiffness: Option<f64>,
    pub damping: Option<f64>,
    pub mass: Option<f64>,
}

/// Physics simulation type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PhysicsKind {
    Spring,
    Gravity,
    Friction,
}

/// Animation transform state (from/to)
#[derive(Debug, Clone, PartialEq)]
pub struct AnimTransform {
    pub x: Option<String>,
    pub y: Option<String>,
    pub opacity: Option<f64>,
    pub scale: Option<f64>,
    pub rotate: Option<f64>,
}

/// Gesture specification for touch/pointer interactions
#[derive(Debug, Clone, PartialEq)]
pub struct GestureSpec {
    pub direction: GestureDirection,
    pub on_release: Option<Vec<GestureAction>>,
}

/// Gesture tracking direction
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GestureDirection {
    Horizontal,
    Vertical,
    Both,
}

/// Actions triggered by gesture release
#[derive(Debug, Clone, PartialEq)]
pub enum GestureAction {
    NextSlide {
        velocity_threshold: Option<f64>,
        offset_threshold: Option<String>,
    },
    SnapBack,
    Custom(Vec<Stmt>),
}

// === Server-side AST nodes ===

/// Server definition
#[derive(Debug, Clone, PartialEq)]
pub struct ServeDef {
    pub name: String,
    pub port: Option<u16>,
    pub prefix: Option<String>,
    pub routes: Vec<ServerRoute>,
    pub guards: Vec<String>,
    pub span: Span,
}

/// Server route
#[derive(Debug, Clone, PartialEq)]
pub struct ServerRoute {
    pub method: HttpMethod,
    pub pattern: String,
    pub handler: String,
    pub guards: Vec<String>,
    pub span: Span,
}

/// Database definition
#[derive(Debug, Clone, PartialEq)]
pub struct DatabaseDef {
    pub name: String,
    pub kind: DbKind,
    pub url: Expr,
    pub pool: Option<u32>,
    pub span: Span,
}

/// Database kind
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DbKind {
    Postgres,
    Sqlite,
    Mysql,
}

/// Task definition
#[derive(Debug, Clone, PartialEq)]
pub struct TaskDef {
    pub name: String,
    pub schedule: TaskSchedule,
    pub body: Vec<Stmt>,
    pub span: Span,
}

/// Task schedule
#[derive(Debug, Clone, PartialEq)]
pub enum TaskSchedule {
    Every(String),
    At(String),
    Cron(String),
}

/// Queue definition
#[derive(Debug, Clone, PartialEq)]
pub struct QueueDef {
    pub name: String,
    pub workers: Option<u32>,
    pub retry: Option<u32>,
    pub span: Span,
}

/// Live (WebSocket) definition with CRDT collaboration support
#[derive(Debug, Clone, PartialEq)]
pub struct LiveDef {
    pub name: String,
    pub path: String,
    pub on_connect: Option<(String, Vec<Stmt>)>,
    pub on_message: Option<(String, String, Vec<Stmt>)>,
    pub on_leave: Option<(String, Vec<Stmt>)>,
    /// Sync mode for CRDT-based state synchronization
    pub sync: Option<SyncMode>,
    /// Enable presence indicators (cursor tracking, user list)
    pub presence: bool,
    /// CRDT type for conflict resolution
    pub transform: Option<CrdtType>,
    pub span: Span,
}

/// Sync mode for live collaboration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SyncMode {
    Automatic,
    Manual,
}

/// CRDT (Conflict-free Replicated Data Type) variants
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CrdtType {
    /// Last-Write-Wins register
    Lww,
    /// Positive-Negative Counter
    PNCounter,
    /// Grow-only Counter
    GCounter,
    /// Multi-Value Register
    MVRegister,
}

/// Auth definition
#[derive(Debug, Clone, PartialEq)]
pub struct AuthDef {
    pub kind: AuthKind,
    pub secret: Expr,
    pub expires: Option<String>,
    pub span: Span,
}

/// Auth kind
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AuthKind {
    Jwt,
    Basic,
    OAuth,
}

/// Config definition
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigDef {
    pub name: String,
    pub version: String,
    pub web: Option<String>,
    pub server: Option<String>,
    pub span: Span,
}
