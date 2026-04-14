use logos::Logos;
use std::fmt;

/// Token types for the VED language
/// Uses logos for efficient lexer generation
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\r]+")] // Skip whitespace but NOT newlines (significant)
#[logos(error = LexError)]
pub enum Token {
    // === Significant Whitespace ===
    /// Newline - tracked for Python-style indentation
    #[regex(r"\n")]
    Newline,

    /// Indentation level (measured in spaces, must be multiple of 2)
    /// This is handled by the lexer preprocessor, not logos directly
    Indent(usize),

    /// Dedent marker
    Dedent,

    /// End of indentation block
    #[allow(dead_code)]
    EndOfBlock,

    // === Comments ===
    /// Line comment: -- comment text
    #[regex(r"--[^\n]*")]
    Comment,

    // === Shared Keywords ===
    #[token("shape")]
    KwShape,
    #[token("think")]
    KwThink,
    #[token("let")]
    KwLet,
    #[token("mut")]
    KwMut,
    #[token("give")]
    KwGive,
    #[token("fail")]
    KwFail,
    #[token("wait")]
    KwWait,
    #[token("try")]
    KwTry,
    #[token("when")]
    KwWhen,
    #[token("otherwise")]
    KwOtherwise,
    #[token("and")]
    KwAnd,
    #[token("or")]
    KwOr,
    #[token("not")]
    KwNot,
    #[token("is")]
    KwIs,
    #[token("in")]
    KwIn,
    #[token("with")]
    KwWith,
    #[token("nothing")]
    KwNothing,
    #[token("async")]
    KwAsync,
    #[token("on")]
    KwOn,
    #[token("ok")]
    KwOk,
    #[token("maybe")]
    KwMaybe,
    #[token("each")]
    KwEach,
    #[token("keep")]
    KwKeep,
    #[token("fold")]
    KwFold,
    #[token("take")]
    KwTake,
    #[token("sort")]
    KwSort,

    // === Client-side Keywords ===
    #[token("screen")]
    KwScreen,
    #[token("piece")]
    KwPiece,
    #[token("box")]
    KwBox,
    #[token("words")]
    KwWords,
    #[token("tap")]
    KwTap,
    #[token("field")]
    KwField,
    #[token("image")]
    KwImage,
    #[token("remember")]
    KwRemember,
    #[token("fetch")]
    KwFetch,
    #[token("cache")]
    KwCache,
    #[token("loading")]
    KwLoading,
    #[token("error")]
    KwError,
    #[token("load")]
    KwLoad,
    #[token("routes")]
    KwRoutes,
    #[token("connect")]
    KwConnect,
    #[token("go")]
    KwGo,
    #[token("back")]
    KwBack,
    #[token("needs")]
    KwNeeds,
    #[token("param")]
    KwParam,
    #[token("show")]
    KwShow,
    #[token("hide")]
    KwHide,
    #[token("enter")]
    KwEnter,
    #[token("leave")]
    KwLeave,
    #[token("hover")]
    KwHover,
    #[token("press")]
    KwPress,
    #[token("loop")]
    KwLoop,
    #[token("move")]
    KwMove,
    #[token("ease")]
    KwEase,
    #[token("duration")]
    KwDuration,

    // === Ghost AI-Native Keywords ===
    #[token("ghost")]
    KwGhost,

    // === Stream Keywords ===
    #[token("stream")]
    KwStream,
    #[token("lazy")]
    KwLazy,

    // === Flow / Animation Keywords ===
    #[token("animate")]
    KwAnimate,
    #[token("spring")]
    KwSpring,
    #[token("physics")]
    KwPhysics,
    #[token("stiffness")]
    KwStiffness,
    #[token("damping")]
    KwDamping,
    #[token("mass")]
    KwMass,
    #[token("snap")]
    KwSnap,
    #[token("pan")]
    KwPan,
    #[token("from")]
    KwFrom,
    #[token("to")]
    KwTo,
    #[token("scale")]
    KwScale,
    #[token("rotate")]
    KwRotate,
    #[token("velocity")]
    KwVelocity,
    #[token("offset")]
    KwOffset,
    #[token("release")]
    KwRelease,

    // === Live Collaboration Keywords ===
    #[token("sync")]
    KwSync,
    #[token("presence")]
    KwPresence,
    #[token("transform")]
    KwTransform,
    #[token("automatic")]
    KwAutomatic,
    #[token("manual")]
    KwManual,
    #[token("lww")]
    KwLWW,
    #[token("pncounter")]
    KwPNCounter,
    #[token("gcounter")]
    KwGCounter,
    #[token("mvregister")]
    KwMVRegister,

    // === Annotations ===
    #[token("@")]
    At,

    // === Layout / Style Properties ===
    #[token("fill")]
    KwFill,
    #[token("tall")]
    KwTall,
    #[token("flow")]
    KwFlow,
    #[token("gap")]
    KwGap,
    #[token("padding")]
    KwPadding,
    #[token("center")]
    KwCenter,
    #[token("backdrop")]
    KwBackdrop,
    /// Push keyword (layout direction) - single unique variant
    #[token("push")]
    KwPush,
    #[token("color")]
    KwColor,
    #[token("radius")]
    KwRadius,
    #[token("border")]
    KwBorder,
    #[token("shadow")]
    KwShadow,
    #[token("blur")]
    KwBlur,
    #[token("opacity")]
    KwOpacity,
    #[token("clip")]
    KwClip,
    #[token("scroll")]
    KwScroll,
    #[token("cursor")]
    KwCursor,
    #[token("size")]
    KwSize,
    #[token("weight")]
    KwWeight,
    #[token("align")]
    KwAlign,
    #[token("spacing")]
    KwSpacing,
    #[token("lines")]
    KwLines,
    #[token("cut")]
    KwCut,

    // === Server-side Keywords ===
    #[token("serve")]
    KwServe,
    #[token("database")]
    KwDatabase,
    #[token("task")]
    KwTask,
    #[token("queue")]
    KwQueue,
    #[token("live")]
    KwLive,
    #[token("guard")]
    KwGuard,
    #[token("confirm")]
    KwConfirm,
    #[token("auth")]
    KwAuth,
    #[token("reply")]
    KwReply,
    #[token("transaction")]
    KwTransaction,
    #[token("every")]
    KwEvery,
    #[token("workers")]
    KwWorkers,
    #[token("retry")]
    KwRetry,
    #[token("port")]
    KwPort,
    #[token("prefix")]
    KwPrefix,
    #[token("path")]
    KwPath,
    #[token("secret")]
    KwSecret,
    #[token("expires")]
    KwExpires,
    #[token("kind")]
    KwKind,
    #[token("url")]
    KwUrl,
    #[token("pool")]
    KwPool,
    #[token("as")]
    KwAs,
    #[token("broadcast")]
    KwBroadcast,
    /// Server-side push (WebSocket) - unified with KwPush
    /// Context determines meaning: layout vs WebSocket

    // === HTTP Methods ===
    #[token("GET")]
    HttpGet,
    #[token("POST")]
    HttpPost,
    #[token("PUT")]
    HttpPut,
    #[token("PATCH")]
    HttpPatch,
    #[token("DEL")]
    HttpDel,

    // === Literals ===
    #[regex(r"-?[0-9]+\.[0-9]+", |lex| lex.slice().parse::<f64>().ok())]
    LitDec(Option<f64>),

    #[regex(r"-?[0-9]+", |lex| lex.slice().parse::<i64>().ok())]
    LitNum(Option<i64>),

    /// String literal - handles both interpolation parsing and regular strings
    #[regex(r#""([^"\\]|\\.)*""#, |lex| lex.slice().to_string())]
    LitStr(String),

    /// Raw string literal (no interpolation)
    #[regex(r##"#"[^"]*"#"##, |lex| {
        let s = lex.slice();
        s[2..s.len()-2].to_string()
    })]
    LitStrRaw(String),

    #[token("yes")]
    LitYes,
    #[token("no")]
    LitNo,

    // === Type Keywords ===
    #[token("num")]
    TyNum,
    #[token("dec")]
    TyDec,
    #[token("text")]
    TyText,
    #[token("bool")]
    TyBool,
    #[token("list")]
    TyList,
    #[token("map")]
    TyMap,
    #[token("any")]
    TyAny,

    // === Identifiers ===
    /// Identifier: letter or underscore followed by alphanumeric or underscore
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Ident(String),

    // === Operators ===
    #[token("|>")]
    PipeOp,
    #[token("=>")]
    FatArrow,
    #[token("->")]
    ThinArrow,
    #[token("?")]
    Question,
    #[token(":")]
    Colon,
    #[token(",")]
    Comma,
    #[token(".")]
    Dot,
    #[token("==")]
    EqEq,
    #[token("=")]
    Eq,
    #[token("!=")]
    NotEq,
    #[token(">=")]
    GtEq,
    #[token("<=")]
    LtEq,
    #[token(">")]
    Gt,
    #[token("<")]
    Lt,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,

    // === Value Keywords ===
    #[token("whole")]
    ValWhole,
    #[token("half")]
    ValHalf,
    #[token("down")]
    ValDown,
    #[token("up")]
    ValUp,
    #[token("across")]
    ValAcross,
    #[token("wrap")]
    ValWrap,
    #[token("layer")]
    ValLayer,
    #[token("both")]
    ValBoth,
    #[token("right")]
    ValRight,
    #[token("left")]
    ValLeft,
    #[token("bottom")]
    ValBottom,
    #[token("fat")]
    ValFat,
    #[token("thin")]
    ValThin,
    #[token("normal")]
    ValNormal,
    #[token("white")]
    ValWhite,
    #[token("black")]
    ValBlack,
    #[token("red")]
    ValRed,
    #[token("green")]
    ValGreen,
    #[token("blue")]
    ValBlue,
    #[token("pointer")]
    ValPointer,
    #[token("linear")]
    ValLinear,
    #[token("add")]
    ValAdd,
    #[token("horizontal")]
    ValHorizontal,
    #[token("vertical")]
    ValVertical,

    // === Color/Size/Time Literals ===
    #[regex(r"#[0-9a-fA-F]{3,8}", |lex| lex.slice().to_string())]
    LitColor(String),

    #[regex(r"[0-9]+[m][s]", |lex| lex.slice().to_string())]
    LitMs(String),

    #[regex(r"[0-9]+[s]", |lex| lex.slice().to_string())]
    LitSecs(String),

    #[regex(r"[0-9]+[h]", |lex| lex.slice().to_string())]
    LitHours(String),

    #[regex(r"[0-9]+[%]", |lex| lex.slice().to_string())]
    LitPercent(String),

    // === Special ===
    /// End of file marker
    #[regex(r"", logos::skip)]
    Eof,
}

/// Lexer error type
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LexError {
    pub message: String,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for LexError {}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Newline => write!(f, "newline"),
            Token::KwShape => write!(f, "shape"),
            Token::KwThink => write!(f, "think"),
            Token::KwLet => write!(f, "let"),
            Token::KwMut => write!(f, "mut"),
            Token::KwGive => write!(f, "give"),
            Token::KwFail => write!(f, "fail"),
            Token::KwWhen => write!(f, "when"),
            Token::KwOtherwise => write!(f, "otherwise"),
            Token::KwAnd => write!(f, "and"),
            Token::KwOr => write!(f, "or"),
            Token::KwNot => write!(f, "not"),
            Token::KwScreen => write!(f, "screen"),
            Token::KwServe => write!(f, "serve"),
            Token::KwDatabase => write!(f, "database"),
            Token::KwEach => write!(f, "each"),
            Token::FatArrow => write!(f, "=>"),
            Token::ThinArrow => write!(f, "->"),
            Token::PipeOp => write!(f, "|>"),
            Token::Eq => write!(f, "="),
            Token::Colon => write!(f, ":"),
            Token::Comma => write!(f, ","),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::LBrace => write!(f, "{{"),
            Token::RBrace => write!(f, "}}"),
            Token::LBracket => write!(f, "["),
            Token::RBracket => write!(f, "]"),
            Token::Ident(s) => write!(f, "identifier({})", s),
            Token::LitStr(s) => write!(f, "string({})", s),
            Token::LitNum(Some(n)) => write!(f, "number({})", n),
            Token::LitNum(None) => write!(f, "number(invalid)"),
            Token::LitDec(Some(d)) => write!(f, "decimal({})", d),
            Token::LitDec(None) => write!(f, "decimal(invalid)"),
            Token::LitStrRaw(s) => write!(f, "raw_string({})", s),
            Token::Indent(n) => write!(f, "indent({})", n),
            Token::Dedent => write!(f, "dedent"),
            Token::EndOfBlock => write!(f, "end_block"),
            Token::Eof => write!(f, "eof"),
            Token::Comment => write!(f, "comment"),
            Token::KwLoad => write!(f, "load"),
            Token::KwConnect => write!(f, "connect"),
            Token::KwGo => write!(f, "go"),
            Token::KwBack => write!(f, "back"),
            Token::KwNeeds => write!(f, "needs"),
            Token::KwParam => write!(f, "param"),
            Token::KwShow => write!(f, "show"),
            Token::KwHide => write!(f, "hide"),
            Token::KwEnter => write!(f, "enter"),
            Token::KwLeave => write!(f, "leave"),
            Token::KwHover => write!(f, "hover"),
            Token::KwPress => write!(f, "press"),
            Token::KwLoop => write!(f, "loop"),
            Token::KwMove => write!(f, "move"),
            Token::KwEase => write!(f, "ease"),
            Token::KwDuration => write!(f, "duration"),
            Token::KwFill => write!(f, "fill"),
            Token::KwTall => write!(f, "tall"),
            Token::KwFlow => write!(f, "flow"),
            Token::KwGap => write!(f, "gap"),
            Token::KwPadding => write!(f, "padding"),
            Token::KwCenter => write!(f, "center"),
            Token::KwBackdrop => write!(f, "backdrop"),
            Token::KwPush => write!(f, "push"),
            Token::KwColor => write!(f, "color"),
            Token::KwRadius => write!(f, "radius"),
            Token::KwBorder => write!(f, "border"),
            Token::KwShadow => write!(f, "shadow"),
            Token::KwBlur => write!(f, "blur"),
            Token::KwOpacity => write!(f, "opacity"),
            Token::KwClip => write!(f, "clip"),
            Token::KwScroll => write!(f, "scroll"),
            Token::KwCursor => write!(f, "cursor"),
            Token::KwSize => write!(f, "size"),
            Token::KwWeight => write!(f, "weight"),
            Token::KwAlign => write!(f, "align"),
            Token::KwSpacing => write!(f, "spacing"),
            Token::KwLines => write!(f, "lines"),
            Token::KwCut => write!(f, "cut"),
            Token::KwPort => write!(f, "port"),
            Token::KwPrefix => write!(f, "prefix"),
            Token::KwPath => write!(f, "path"),
            Token::KwSecret => write!(f, "secret"),
            Token::KwExpires => write!(f, "expires"),
            Token::KwKind => write!(f, "kind"),
            Token::KwUrl => write!(f, "url"),
            Token::KwPool => write!(f, "pool"),
            Token::KwAs => write!(f, "as"),
            Token::KwBroadcast => write!(f, "broadcast"),
            Token::KwTransaction => write!(f, "transaction"),
            Token::KwEvery => write!(f, "every"),
            Token::KwWorkers => write!(f, "workers"),
            Token::KwRetry => write!(f, "retry"),
            Token::KwGuard => write!(f, "guard"),
            Token::KwConfirm => write!(f, "confirm"),
            Token::KwReply => write!(f, "reply"),
            Token::KwRemember => write!(f, "remember"),
            Token::KwFetch => write!(f, "fetch"),
            Token::KwCache => write!(f, "cache"),
            Token::KwLoading => write!(f, "loading"),
            Token::KwError => write!(f, "error"),
            Token::KwPiece => write!(f, "piece"),
            Token::KwBox => write!(f, "box"),
            Token::KwWords => write!(f, "words"),
            Token::KwTap => write!(f, "tap"),
            Token::KwField => write!(f, "field"),
            Token::KwImage => write!(f, "image"),
            Token::KwRoutes => write!(f, "routes"),
            Token::KwIs => write!(f, "is"),
            Token::KwIn => write!(f, "in"),
            Token::KwWith => write!(f, "with"),
            Token::KwNothing => write!(f, "nothing"),
            Token::KwAsync => write!(f, "async"),
            Token::KwOn => write!(f, "on"),
            Token::KwOk => write!(f, "ok"),
            Token::KwMaybe => write!(f, "maybe"),
            Token::KwKeep => write!(f, "keep"),
            Token::KwFold => write!(f, "fold"),
            Token::KwTake => write!(f, "take"),
            Token::KwSort => write!(f, "sort"),
            Token::KwWait => write!(f, "wait"),
            Token::KwTry => write!(f, "try"),
            Token::KwAuth => write!(f, "auth"),
            Token::KwTask => write!(f, "task"),
            Token::KwQueue => write!(f, "queue"),
            Token::KwLive => write!(f, "live"),
            Token::Plus => write!(f, "+"),
            Token::Minus => write!(f, "-"),
            Token::Star => write!(f, "*"),
            Token::Slash => write!(f, "/"),
            Token::Percent => write!(f, "%"),
            Token::NotEq => write!(f, "!="),
            Token::Lt => write!(f, "<"),
            Token::Gt => write!(f, ">"),
            Token::LtEq => write!(f, "<="),
            Token::GtEq => write!(f, ">="),
            Token::Dot => write!(f, "."),
            Token::Question => write!(f, "?"),
            Token::LitYes => write!(f, "yes"),
            Token::LitNo => write!(f, "no"),
            Token::LitColor(c) => write!(f, "color({})", c),
            Token::LitMs(s) => write!(f, "ms({})", s),
            Token::LitSecs(s) => write!(f, "secs({})", s),
            Token::LitHours(s) => write!(f, "hours({})", s),
            Token::LitPercent(s) => write!(f, "percent({})", s),
            Token::ValWhole => write!(f, "whole"),
            Token::ValHalf => write!(f, "half"),
            Token::ValDown => write!(f, "down"),
            Token::ValUp => write!(f, "up"),
            Token::ValAcross => write!(f, "across"),
            Token::ValWrap => write!(f, "wrap"),
            Token::ValLayer => write!(f, "layer"),
            Token::ValBoth => write!(f, "both"),
            Token::ValRight => write!(f, "right"),
            Token::ValLeft => write!(f, "left"),
            Token::ValBottom => write!(f, "bottom"),
            Token::ValFat => write!(f, "fat"),
            Token::ValThin => write!(f, "thin"),
            Token::ValNormal => write!(f, "normal"),
            Token::ValWhite => write!(f, "white"),
            Token::ValBlack => write!(f, "black"),
            Token::ValRed => write!(f, "red"),
            Token::ValGreen => write!(f, "green"),
            Token::ValBlue => write!(f, "blue"),
            Token::ValPointer => write!(f, "pointer"),
            Token::ValLinear => write!(f, "linear"),
            Token::ValAdd => write!(f, "add"),
            Token::ValHorizontal => write!(f, "horizontal"),
            Token::ValVertical => write!(f, "vertical"),
            Token::HttpGet => write!(f, "GET"),
            Token::HttpPost => write!(f, "POST"),
            Token::HttpPut => write!(f, "PUT"),
            Token::HttpPatch => write!(f, "PATCH"),
            Token::HttpDel => write!(f, "DEL"),
            Token::TyNum => write!(f, "num"),
            Token::TyDec => write!(f, "dec"),
            Token::TyText => write!(f, "text"),
            Token::TyBool => write!(f, "bool"),
            Token::TyList => write!(f, "list"),
            Token::TyMap => write!(f, "map"),
            Token::TyAny => write!(f, "any"),
            // Stream / Flow / Live keywords
            Token::KwGhost => write!(f, "ghost"),
            Token::KwStream => write!(f, "stream"),
            Token::KwLazy => write!(f, "lazy"),
            Token::KwAnimate => write!(f, "animate"),
            Token::KwSpring => write!(f, "spring"),
            Token::KwPhysics => write!(f, "physics"),
            Token::KwStiffness => write!(f, "stiffness"),
            Token::KwDamping => write!(f, "damping"),
            Token::KwMass => write!(f, "mass"),
            Token::KwSnap => write!(f, "snap"),
            Token::KwPan => write!(f, "pan"),
            Token::KwFrom => write!(f, "from"),
            Token::KwTo => write!(f, "to"),
            Token::KwScale => write!(f, "scale"),
            Token::KwRotate => write!(f, "rotate"),
            Token::KwVelocity => write!(f, "velocity"),
            Token::KwOffset => write!(f, "offset"),
            Token::KwRelease => write!(f, "release"),
            Token::KwSync => write!(f, "sync"),
            Token::KwPresence => write!(f, "presence"),
            Token::KwTransform => write!(f, "transform"),
            Token::KwAutomatic => write!(f, "automatic"),
            Token::KwManual => write!(f, "manual"),
            Token::KwLWW => write!(f, "lww"),
            Token::KwPNCounter => write!(f, "pncounter"),
            Token::KwGCounter => write!(f, "gcounter"),
            Token::KwMVRegister => write!(f, "mvregister"),
            Token::At => write!(f, "@"),
            _ => write!(f, "unknown"),
        }
    }
}

impl Token {
    /// Check if this token is an identifier
    pub fn is_ident(&self) -> bool {
        matches!(self, Token::Ident(_))
    }

    /// Get identifier name if this is an identifier token
    pub fn ident_name(&self) -> Option<&str> {
        match self {
            Token::Ident(s) => Some(s),
            _ => None,
        }
    }

    #[allow(dead_code)]
    /// Check if token is a literal
    pub fn is_literal(&self) -> bool {
        matches!(
            self,
            Token::LitNum(_) | Token::LitDec(_) | Token::LitStr(_) | Token::LitStrRaw(_)
        )
    }

    #[allow(dead_code)]
    /// Check if token is a comparison operator
    pub fn is_comparison(&self) -> bool {
        matches!(
            self,
            Token::Eq | Token::NotEq | Token::Lt | Token::Gt | Token::LtEq | Token::GtEq
        )
    }

    /// Check if token can start an expression
    pub fn is_expr_start(&self) -> bool {
        matches!(
            self,
            Token::LitNum(_)
                | Token::LitDec(_)
                | Token::LitStr(_)
                | Token::LitStrRaw(_)
                | Token::Ident(_)
                | Token::LParen
                | Token::LBracket
                | Token::LBrace
                | Token::KwNothing
                | Token::LitYes
                | Token::LitNo
                | Token::KwNot
                | Token::Minus
                | Token::LitColor(_)
        )
    }

    /// Check if token can start a statement
    pub fn is_stmt_start(&self) -> bool {
        matches!(
            self,
            Token::KwLet
                | Token::KwGive
                | Token::KwFail
                | Token::KwWhen
                | Token::KwEach
                | Token::KwTransaction
                | Token::Ident(_)
                | Token::KwAsync
                | Token::KwOn
                | Token::KwFetch
        )
    }
}
