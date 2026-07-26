//! MDN「值定义语法」(value definition syntax) 的解析与分析。
//!
//! 以前的分类器只有一行判据：`syntax.contains(' ')` 就算 `Shorthand`。而 MDN
//! 的属性语法几乎都带空格（`auto | <length>` 就带），于是 490 个属性里 384 个
//! 落进「什么都收」的那一组，`align-items: #ff0000`、`animation-delay: 10px`
//! 全部编译通过。
//!
//! 这里改成真正解析一遍语法，回答一个具体问题：**哪些值可以单独构成这个属性
//! 的完整取值**。答案就是该属性允许的 Rust 值类型集合。

use std::collections::{BTreeSet, HashMap, HashSet};

use super::types::{MdnCssProperty, MdnCssSyntax};

// ==========================================
// 语法树
// ==========================================

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    /// `a | b`：任选其一
    Alt(Vec<Node>),
    /// `a || b`：至少一个，顺序任意
    AnyOf(Vec<Node>),
    /// `a && b`：全部都要，顺序任意
    AllOf(Vec<Node>),
    /// `a b`：并列，按序全都要
    Seq(Vec<Node>),
    Mult(Box<Node>, Multiplier),
    /// 字面关键字，如 `auto`
    Keyword(String),
    /// `<length>`、`<color>`
    TypeRef(String),
    /// `<'border-width'>`：引用另一个属性的语法
    PropRef(String),
    /// `rgb( … )`
    Func(String, Box<Node>),
    /// `,`、`/` 这类必须原样出现的分隔符
    Literal(String),
    /// 空（语法为空串的属性）
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Multiplier {
    /// `?`
    Optional,
    /// `*`
    ZeroOrMore,
    /// `+`
    OneOrMore,
    /// `#`
    CommaList,
    /// `{a,b}`
    Range(u32, u32),
    /// `#{a,b}`
    CommaRange(u32, u32),
    /// `!`
    Required,
}

// ==========================================
// 词法
// ==========================================

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Bar,
    DoubleBar,
    DoubleAmp,
    LBracket,
    RBracket,
    LParen,
    RParen,
    /// `<…>` 里的原文
    Type(String),
    Ident(String),
    Comma,
    Slash,
    Star,
    Plus,
    Question,
    Hash,
    Bang,
    Range(u32, u32),
}

fn tokenize(src: &str) -> Vec<Tok> {
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        match c {
            c if c.is_whitespace() => i += 1,
            '|' => {
                if chars.get(i + 1) == Some(&'|') {
                    out.push(Tok::DoubleBar);
                    i += 2;
                } else {
                    out.push(Tok::Bar);
                    i += 1;
                }
            }
            '&' => {
                // 单个 `&` 在 MDN 语法里不出现，一律按 `&&` 处理
                if chars.get(i + 1) == Some(&'&') {
                    i += 2;
                } else {
                    i += 1;
                }
                out.push(Tok::DoubleAmp);
            }
            '[' => {
                out.push(Tok::LBracket);
                i += 1;
            }
            ']' => {
                out.push(Tok::RBracket);
                i += 1;
            }
            '(' => {
                out.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                out.push(Tok::RParen);
                i += 1;
            }
            ',' => {
                out.push(Tok::Comma);
                i += 1;
            }
            '/' => {
                out.push(Tok::Slash);
                i += 1;
            }
            '*' => {
                out.push(Tok::Star);
                i += 1;
            }
            '+' => {
                out.push(Tok::Plus);
                i += 1;
            }
            '?' => {
                out.push(Tok::Question);
                i += 1;
            }
            '#' => {
                out.push(Tok::Hash);
                i += 1;
            }
            '!' => {
                out.push(Tok::Bang);
                i += 1;
            }
            '<' => {
                // `<length [0,∞]>`、`<'grid-template-rows'>`、`<calc-size()>`
                // 内部不会再出现 `>`，读到第一个 `>` 为止
                let start = i + 1;
                let mut j = start;
                while j < chars.len() && chars[j] != '>' {
                    j += 1;
                }
                out.push(Tok::Type(chars[start..j].iter().collect()));
                i = j + 1;
            }
            '{' => {
                let start = i + 1;
                let mut j = start;
                while j < chars.len() && chars[j] != '}' {
                    j += 1;
                }
                let body: String = chars[start..j].iter().collect();
                out.push(parse_range(&body));
                i = j + 1;
            }
            _ => {
                // 关键字：字母/数字/`-`/`_`/`%`，另外允许 `'`（属性引用在 `<>` 内已处理）
                let start = i;
                while i < chars.len()
                    && (chars[i].is_alphanumeric()
                        || chars[i] == '-'
                        || chars[i] == '_'
                        || chars[i] == '%'
                        || chars[i] == '.')
                {
                    i += 1;
                }
                if i == start {
                    // 无法识别的字符，跳过，避免死循环
                    i += 1;
                    continue;
                }
                out.push(Tok::Ident(chars[start..i].iter().collect()));
            }
        }
    }
    out
}

fn parse_range(body: &str) -> Tok {
    let (lo, hi) = match body.split_once(',') {
        Some((a, b)) => {
            let lo = a.trim().parse::<u32>().unwrap_or(1);
            let hi = b.trim().parse::<u32>().unwrap_or(u32::MAX);
            (lo, hi)
        }
        None => {
            let n = body.trim().parse::<u32>().unwrap_or(1);
            (n, n)
        }
    };
    Tok::Range(lo, hi)
}

// ==========================================
// 语法分析
// ==========================================

/// 优先级由弱到强：`|` < `||` < `&&` < 并列 < 乘数 < 分组
pub fn parse(src: &str) -> Node {
    let toks = tokenize(src);
    let mut p = Parser { toks, pos: 0 };
    p.parse_alt()
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn bump(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }

    fn parse_alt(&mut self) -> Node {
        let mut items = vec![self.parse_any_of()];
        while self.peek() == Some(&Tok::Bar) {
            self.pos += 1;
            items.push(self.parse_any_of());
        }
        if items.len() == 1 {
            items.pop().unwrap()
        } else {
            Node::Alt(items)
        }
    }

    fn parse_any_of(&mut self) -> Node {
        let mut items = vec![self.parse_all_of()];
        while self.peek() == Some(&Tok::DoubleBar) {
            self.pos += 1;
            items.push(self.parse_all_of());
        }
        if items.len() == 1 {
            items.pop().unwrap()
        } else {
            Node::AnyOf(items)
        }
    }

    fn parse_all_of(&mut self) -> Node {
        let mut items = vec![self.parse_seq()];
        while self.peek() == Some(&Tok::DoubleAmp) {
            self.pos += 1;
            items.push(self.parse_seq());
        }
        if items.len() == 1 {
            items.pop().unwrap()
        } else {
            Node::AllOf(items)
        }
    }

    fn parse_seq(&mut self) -> Node {
        let mut items = Vec::new();
        while self.starts_term() {
            items.push(self.parse_term());
        }
        match items.len() {
            0 => Node::Empty,
            1 => items.pop().unwrap(),
            _ => Node::Seq(items),
        }
    }

    fn starts_term(&self) -> bool {
        matches!(
            self.peek(),
            Some(
                Tok::LBracket
                    | Tok::Type(_)
                    | Tok::Ident(_)
                    | Tok::Comma
                    | Tok::Slash
                    | Tok::Star
                    | Tok::Plus
            )
        ) && !self.at_stray_multiplier()
    }

    /// `*` / `+` 出现在项的开头时是字面量而不是乘数——MDN 里没有这种写法，
    /// 但数据总有例外，别让它把并列解析卡死。
    fn at_stray_multiplier(&self) -> bool {
        false
    }

    fn parse_term(&mut self) -> Node {
        let mut node = self.parse_primary();
        loop {
            match self.peek() {
                Some(Tok::Question) => {
                    self.pos += 1;
                    node = Node::Mult(Box::new(node), Multiplier::Optional);
                }
                Some(Tok::Star) => {
                    self.pos += 1;
                    node = Node::Mult(Box::new(node), Multiplier::ZeroOrMore);
                }
                Some(Tok::Plus) => {
                    self.pos += 1;
                    node = Node::Mult(Box::new(node), Multiplier::OneOrMore);
                }
                Some(Tok::Bang) => {
                    self.pos += 1;
                    node = Node::Mult(Box::new(node), Multiplier::Required);
                }
                Some(Tok::Hash) => {
                    self.pos += 1;
                    if let Some(Tok::Range(lo, hi)) = self.peek().cloned() {
                        self.pos += 1;
                        node = Node::Mult(Box::new(node), Multiplier::CommaRange(lo, hi));
                    } else {
                        node = Node::Mult(Box::new(node), Multiplier::CommaList);
                    }
                }
                Some(Tok::Range(lo, hi)) => {
                    let (lo, hi) = (*lo, *hi);
                    self.pos += 1;
                    node = Node::Mult(Box::new(node), Multiplier::Range(lo, hi));
                }
                _ => break,
            }
        }
        node
    }

    fn parse_primary(&mut self) -> Node {
        match self.bump() {
            Some(Tok::LBracket) => {
                let inner = self.parse_alt();
                if self.peek() == Some(&Tok::RBracket) {
                    self.pos += 1;
                }
                inner
            }
            Some(Tok::Type(raw)) => {
                let raw = raw.trim();
                if let Some(stripped) = raw.strip_prefix('\'') {
                    Node::PropRef(stripped.trim_end_matches('\'').to_string())
                } else {
                    // `<length [0,∞]>` 的取值范围与类型无关，丢掉
                    let name = raw.split_whitespace().next().unwrap_or(raw);
                    Node::TypeRef(name.to_string())
                }
            }
            Some(Tok::Ident(name)) => {
                if self.peek() == Some(&Tok::LParen) {
                    self.pos += 1;
                    let args = self.parse_alt();
                    if self.peek() == Some(&Tok::RParen) {
                        self.pos += 1;
                    }
                    Node::Func(name, Box::new(args))
                } else {
                    Node::Keyword(name)
                }
            }
            Some(Tok::Comma) => Node::Literal(",".into()),
            Some(Tok::Slash) => Node::Literal("/".into()),
            Some(Tok::Star) => Node::Literal("*".into()),
            Some(Tok::Plus) => Node::Literal("+".into()),
            _ => Node::Empty,
        }
    }
}

// ==========================================
// 值类别
// ==========================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Length,
    Percentage,
    Number,
    Integer,
    Color,
    Angle,
    Time,
    /// `<flex>`：网格轨道的 `fr`
    Flex,
    Url,
    /// `<string>` / `<custom-ident>` / `<*-name>`：只能用裸字符串表达
    Textual,
    /// 函数式取值（`rgb()`、`fit-content()`）：Rust 侧没有对应类型，
    /// 需要 `css_unsafe(...)`
    Func,
    /// 解析不出来的引用：保守放行裸字符串
    Opaque,
}

/// 某个属性单独一个值能取到的东西。
#[derive(Debug, Clone, Default)]
pub struct Analysis {
    pub singles: BTreeSet<Kind>,
    pub keywords: BTreeSet<String>,
    /// 该位置可以由多个分量拼成（`<length>{1,4}`、`a && b`、`<x>#`）
    pub multi: bool,
    /// 该位置整体可以缺省
    pub optional: bool,
}

impl Analysis {
    fn of_kind(k: Kind) -> Self {
        Self {
            singles: BTreeSet::from([k]),
            ..Default::default()
        }
    }

    fn merge_from(&mut self, other: &Analysis) {
        self.singles.extend(other.singles.iter().copied());
        self.keywords.extend(other.keywords.iter().cloned());
    }
}

/// 已知的基础类型：命中就到此为止，不再往下展开。
///
/// 尤其重要的是 `<color>`——展开它会把 148 个具名颜色 + 31 个系统颜色灌进
/// 每一个接受颜色的属性的关键字枚举里，`keywords_gen.rs` 的 9 105 行有很大
/// 一部分就是这么来的。
fn primitive_kinds(name: &str) -> Option<&'static [Kind]> {
    Some(match name {
        "length" => &[Kind::Length],
        "percentage" => &[Kind::Percentage],
        "length-percentage" => &[Kind::Length, Kind::Percentage],
        "number" => &[Kind::Number],
        "integer" => &[Kind::Integer],
        "number-percentage" => &[Kind::Number, Kind::Percentage],
        "alpha-value" => &[Kind::Number, Kind::Percentage],
        "angle" => &[Kind::Angle],
        "angle-percentage" => &[Kind::Angle, Kind::Percentage],
        "time" => &[Kind::Time],
        "time-percentage" => &[Kind::Time, Kind::Percentage],
        // `<flex>` 在 MDN 的 syntaxes.json 里没有定义，不特判就会落到
        // `Opaque` → 只能写裸字符串，`fr` 单位便无处可用
        "flex" => &[Kind::Flex],
        "color"
        | "color-base"
        | "absolute-color-base"
        | "named-color"
        | "system-color"
        | "deprecated-system-color"
        | "hex-color"
        | "absolute-color-function"
        | "currentcolor" => &[Kind::Color],
        "url" | "src" | "image" | "url-token" | "url-set" => &[Kind::Url],
        "string"
        | "custom-ident"
        | "dashed-ident"
        | "ident"
        | "custom-property-name"
        | "keyframes-name"
        | "counter-name"
        | "counter-style-name"
        | "container-name"
        | "timeline-name"
        | "view-transition-name"
        | "family-name"
        | "feature-tag-value"
        | "palette-identifier"
        | "position-area"
        | "anchor-name"
        | "dashed-function" => &[Kind::Textual],
        _ => return None,
    })
}

pub struct Resolver<'a> {
    syntaxes: &'a HashMap<String, MdnCssSyntax>,
    props: &'a HashMap<String, MdnCssProperty>,
}

impl<'a> Resolver<'a> {
    pub fn new(
        syntaxes: &'a HashMap<String, MdnCssSyntax>,
        props: &'a HashMap<String, MdnCssProperty>,
    ) -> Self {
        Self { syntaxes, props }
    }

    /// 分析一条属性语法。
    pub fn analyze_syntax(&self, syntax: &str) -> Analysis {
        let node = parse(syntax);
        let mut visiting = HashSet::new();
        self.analyze(&node, &mut visiting)
    }

    fn analyze(&self, node: &Node, visiting: &mut HashSet<String>) -> Analysis {
        match node {
            Node::Empty => Analysis::default(),
            // 分隔符不是取值分量。MDN 的数据里分隔符时常没有被括进它所属的
            // 可选分组——`background: <bg-layer>#? , <final-bg-layer>` 里那个
            // 逗号就是这样。把它当成必填分量会让 `background: red` 这种单值
            // 形式凭空消失，进而丢掉整个 `<color>` 能力。
            Node::Literal(_) => Analysis {
                optional: true,
                ..Default::default()
            },
            Node::Keyword(k) => Analysis {
                keywords: BTreeSet::from([k.clone()]),
                ..Default::default()
            },
            Node::Func(..) => Analysis::of_kind(Kind::Func),
            Node::TypeRef(name) => self.analyze_type_ref(name, visiting),
            Node::PropRef(name) => {
                let key = format!("'{name}'");
                if !visiting.insert(key.clone()) {
                    return Analysis::of_kind(Kind::Opaque);
                }
                let out = match self.props.get(name) {
                    Some(p) => {
                        let node = parse(&p.syntax);
                        self.analyze(&node, visiting)
                    }
                    None => Analysis::of_kind(Kind::Opaque),
                };
                visiting.remove(&key);
                out
            }
            Node::Alt(items) => {
                let mut out = Analysis::default();
                for it in items {
                    let a = self.analyze(it, visiting);
                    out.merge_from(&a);
                    out.multi |= a.multi;
                    out.optional |= a.optional;
                }
                out
            }
            Node::AnyOf(items) => {
                // `a || b`：任一分量单独出现都合法
                let mut out = Analysis::default();
                for it in items {
                    let a = self.analyze(it, visiting);
                    out.merge_from(&a);
                    out.multi |= a.multi;
                }
                out.multi |= items.len() > 1;
                out
            }
            Node::AllOf(items) | Node::Seq(items) => {
                let analyses: Vec<Analysis> =
                    items.iter().map(|i| self.analyze(i, visiting)).collect();
                let required: Vec<&Analysis> = analyses.iter().filter(|a| !a.optional).collect();
                let mut out = Analysis::default();
                match required.len() {
                    // 全都可省略：任一分量都可以单独构成整个取值
                    0 => {
                        for a in &analyses {
                            out.merge_from(a);
                        }
                        out.optional = true;
                    }
                    // 只有一个必填项：它单独出现就是合法的完整取值
                    1 => out.merge_from(required[0]),
                    // 两个以上必填项：无法由单个值构成
                    _ => {}
                }
                out.multi = items.len() > 1 || analyses.iter().any(|a| a.multi);
                out
            }
            Node::Mult(inner, m) => {
                let a = self.analyze(inner, visiting);
                let mut out = Analysis {
                    singles: a.singles.clone(),
                    keywords: a.keywords.clone(),
                    multi: a.multi,
                    optional: a.optional,
                };
                match m {
                    Multiplier::Optional => out.optional = true,
                    Multiplier::ZeroOrMore => {
                        out.optional = true;
                        out.multi = true;
                    }
                    Multiplier::OneOrMore | Multiplier::CommaList => out.multi = true,
                    Multiplier::Range(lo, hi) | Multiplier::CommaRange(lo, hi) => {
                        if *lo > 1 {
                            out.singles.clear();
                            out.keywords.clear();
                        }
                        out.optional = *lo == 0;
                        out.multi |= *hi > 1;
                    }
                    Multiplier::Required => out.optional = false,
                }
                out
            }
        }
    }

    fn analyze_type_ref(&self, name: &str, visiting: &mut HashSet<String>) -> Analysis {
        if let Some(kinds) = primitive_kinds(name) {
            let mut out = Analysis::default();
            out.singles.extend(kinds.iter().copied());
            return out;
        }
        // `<calc-size()>`、`<light-dark()>`：函数式取值
        if name.ends_with("()") {
            return Analysis::of_kind(Kind::Func);
        }
        if !visiting.insert(name.to_string()) {
            return Analysis::of_kind(Kind::Opaque);
        }
        let out = match self.syntaxes.get(name) {
            Some(s) => {
                let node = parse(&s.syntax);
                self.analyze(&node, visiting)
            }
            None => Analysis::of_kind(Kind::Opaque),
        };
        visiting.remove(name);
        out
    }

    /// 只收关键字，穿透 `<color>` 这类基础类型——专供 `ColorKeyword` 使用。
    pub fn harvest_keywords(&self, syntax: &str) -> BTreeSet<String> {
        let node = parse(syntax);
        let mut out = BTreeSet::new();
        let mut visiting = HashSet::new();
        self.collect_keywords(&node, &mut out, &mut visiting);
        out
    }

    fn collect_keywords(
        &self,
        node: &Node,
        out: &mut BTreeSet<String>,
        visiting: &mut HashSet<String>,
    ) {
        match node {
            Node::Keyword(k) => {
                out.insert(k.clone());
            }
            Node::Alt(items) | Node::AnyOf(items) | Node::AllOf(items) | Node::Seq(items) => {
                for i in items {
                    self.collect_keywords(i, out, visiting);
                }
            }
            Node::Mult(inner, _) => self.collect_keywords(inner, out, visiting),
            Node::TypeRef(name) => {
                if name.ends_with("()") || !visiting.insert(name.to_string()) {
                    return;
                }
                if let Some(s) = self.syntaxes.get(name) {
                    let inner = parse(&s.syntax);
                    self.collect_keywords(&inner, out, visiting);
                }
                visiting.remove(name);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn syntaxes(pairs: &[(&str, &str)]) -> HashMap<String, MdnCssSyntax> {
        pairs
            .iter()
            .map(|(k, v)| {
                (
                    k.to_string(),
                    MdnCssSyntax {
                        syntax: v.to_string(),
                    },
                )
            })
            .collect()
    }

    fn analyze(syntax: &str, extra: &[(&str, &str)]) -> Analysis {
        let sx = syntaxes(extra);
        let props = HashMap::new();
        Resolver::new(&sx, &props).analyze_syntax(syntax)
    }

    /// 报告 P1-1 的核心：`auto | <length>` 带空格，但它不是 shorthand，
    /// 也绝不该接受颜色
    #[test]
    fn alternation_is_not_a_shorthand() {
        let a = analyze("auto | <length-percentage>", &[]);
        assert!(a.singles.contains(&Kind::Length));
        assert!(a.singles.contains(&Kind::Percentage));
        assert!(!a.singles.contains(&Kind::Color));
        assert!(!a.multi, "单值选择不是多分量");
        assert!(a.keywords.contains("auto"));
    }

    /// `<length>{1,4}` 单个长度也合法，同时是多分量
    #[test]
    fn repetition_keeps_the_single_form_and_marks_multi() {
        let a = analyze("<length-percentage>{1,4}", &[]);
        assert!(a.singles.contains(&Kind::Length));
        assert!(a.multi);
    }

    /// `{2,4}` 起步就是两个，单值形式不存在
    #[test]
    fn repetition_starting_above_one_has_no_single_form() {
        let a = analyze("<length>{2,4}", &[]);
        assert!(a.singles.is_empty());
        assert!(a.multi);
    }

    /// 并列里只有一个必填项时，那一项可以单独构成完整取值
    #[test]
    fn sequence_with_one_required_component_has_a_single_form() {
        let a = analyze(
            "<overflow-position>? <self-position>",
            &[
                ("overflow-position", "unsafe | safe"),
                ("self-position", "center | start | end"),
            ],
        );
        assert!(a.keywords.contains("center"));
        assert!(!a.keywords.contains("safe"), "可选前缀不能单独成值");
        assert!(a.multi);
    }

    /// 两个必填项并列 → 没有单值形式
    #[test]
    fn sequence_with_two_required_components_has_no_single_form() {
        let a = analyze("<length> <color>", &[]);
        assert!(a.singles.is_empty());
        assert!(a.multi);
    }

    /// `a && b` 全都要，同样没有单值形式
    #[test]
    fn all_of_requires_every_component() {
        let a = analyze("<integer> && <custom-ident>", &[]);
        assert!(a.singles.is_empty());
    }

    /// `a || b` 任一单独出现都合法
    #[test]
    fn any_of_admits_each_component_alone() {
        let a = analyze("<length> || <color>", &[]);
        assert!(a.singles.contains(&Kind::Length));
        assert!(a.singles.contains(&Kind::Color));
        assert!(a.multi);
    }

    /// `<color>` 是终点，不展开成一百多个具名颜色关键字
    #[test]
    fn color_does_not_leak_named_color_keywords() {
        let a = analyze("<color>", &[("color", "<named-color> | currentcolor")]);
        assert_eq!(a.singles, BTreeSet::from([Kind::Color]));
        assert!(a.keywords.is_empty());
    }

    /// 但 `ColorKeyword` 自己需要穿透
    #[test]
    fn harvest_keywords_walks_through_type_refs() {
        let sx = syntaxes(&[("named-color", "red | blue")]);
        let props = HashMap::new();
        let kws = Resolver::new(&sx, &props).harvest_keywords("<named-color> | currentcolor");
        assert!(kws.contains("red"));
        assert!(kws.contains("currentcolor"));
    }

    /// 递归引用不能把生成器转死
    #[test]
    fn cyclic_references_terminate() {
        let a = analyze("<a>", &[("a", "<b> | x"), ("b", "<a> | y")]);
        assert!(a.keywords.contains("x"));
        assert!(a.keywords.contains("y"));
    }

    /// 函数式取值不算「可以写裸字符串」
    #[test]
    fn functions_are_their_own_kind() {
        let a = analyze("fit-content( <length-percentage> )", &[]);
        assert_eq!(a.singles, BTreeSet::from([Kind::Func]));
    }

    /// `background` 的 MDN 语法把逗号留在了可选分组之外：
    /// `<bg-layer>#? , <final-bg-layer>`。把逗号算成必填分量会让
    /// `background: red` 这种单值形式消失，`<color>` 能力也跟着丢掉。
    #[test]
    fn dangling_separators_do_not_kill_the_single_form() {
        let a = analyze("<a>#? , <b>", &[("a", "x"), ("b", "<color>")]);
        assert!(a.singles.contains(&Kind::Color), "{a:?}");
    }

    /// 但真正并列的两个必填分量还是没有单值形式
    #[test]
    fn separators_do_not_excuse_two_required_components() {
        let a = analyze("<length> / <color>", &[]);
        assert!(a.singles.is_empty(), "{a:?}");
    }

    #[test]
    fn optional_only_sequence_lets_each_part_stand_alone() {
        let a = analyze("<length>? <color>?", &[]);
        assert!(a.singles.contains(&Kind::Length));
        assert!(a.singles.contains(&Kind::Color));
        assert!(a.optional);
    }
}
