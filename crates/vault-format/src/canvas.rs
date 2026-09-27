//! JSON Canvas 1.0 (`.canvas`, <https://jsoncanvas.org/spec/1.0/>) for saved mind-map layouts
//! in `maps/` (PLAN §6.8).
//!
//! Written in Obsidian's layout: tab indentation, one node/edge object per line. Unknown
//! fields on the canvas, nodes and edges are preserved.

use serde_json::{Map, Value};

/// A preset colour (`"1"`–`"6"`) or `#RRGGBB`.
pub fn is_valid_color(c: &str) -> bool {
    matches!(c, "1" | "2" | "3" | "4" | "5" | "6")
        || (c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|h| h.is_ascii_hexdigit()))
}

/// Errors reading a canvas.
#[derive(Debug, thiserror::Error)]
pub enum CanvasError {
    /// Not JSON.
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// JSON that is not a canvas.
    #[error("invalid canvas: {0}")]
    Invalid(String),
}

/// A problem found by [`Canvas::validate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanvasIssue {
    /// Two nodes or edges share an ID.
    DuplicateId(String),
    /// An edge points at a node that does not exist.
    DanglingEdge {
        /// Edge ID.
        edge: String,
        /// Missing node ID.
        node: String,
    },
    /// A colour is neither a preset nor `#RRGGBB`.
    BadColor(String),
    /// A file subpath does not start with `#`.
    BadSubpath(String),
    /// A width or height is not positive.
    BadSize(String),
}

/// Group background style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackgroundStyle {
    /// `cover`
    Cover,
    /// `ratio`
    Ratio,
    /// `repeat`
    Repeat,
}

impl BackgroundStyle {
    fn as_str(self) -> &'static str {
        match self {
            Self::Cover => "cover",
            Self::Ratio => "ratio",
            Self::Repeat => "repeat",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "cover" => Some(Self::Cover),
            "ratio" => Some(Self::Ratio),
            "repeat" => Some(Self::Repeat),
            _ => None,
        }
    }
}

/// The type-specific part of a node.
#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    /// Markdown text.
    Text {
        /// Text.
        text: String,
    },
    /// A vault file (a note, image, PDF…).
    File {
        /// Vault path.
        file: String,
        /// `#Heading` or `#^block`.
        subpath: Option<String>,
    },
    /// A URL.
    Link {
        /// URL.
        url: String,
    },
    /// A group box.
    Group {
        /// Label.
        label: Option<String>,
        /// Background image path.
        background: Option<String>,
        /// Background style.
        background_style: Option<BackgroundStyle>,
    },
}

/// A node.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// Unique ID.
    pub id: String,
    /// Type and its fields.
    pub kind: NodeKind,
    /// Position and size (integers).
    pub x: i64,
    /// y
    pub y: i64,
    /// width
    pub width: i64,
    /// height
    pub height: i64,
    /// Colour.
    pub color: Option<String>,
    /// Unknown fields.
    pub extra: Map<String, Value>,
}

/// Edge side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    /// top
    Top,
    /// right
    Right,
    /// bottom
    Bottom,
    /// left
    Left,
}

impl Side {
    fn as_str(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Right => "right",
            Self::Bottom => "bottom",
            Self::Left => "left",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "top" => Some(Self::Top),
            "right" => Some(Self::Right),
            "bottom" => Some(Self::Bottom),
            "left" => Some(Self::Left),
            _ => None,
        }
    }
}

/// Edge end shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum End {
    /// none
    None,
    /// arrow
    Arrow,
}

impl End {
    fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Arrow => "arrow",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "none" => Some(Self::None),
            "arrow" => Some(Self::Arrow),
            _ => None,
        }
    }
}

/// An edge.
#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    /// Unique ID.
    pub id: String,
    /// Source node.
    pub from_node: String,
    /// Source side.
    pub from_side: Option<Side>,
    /// Source end (default `none`).
    pub from_end: Option<End>,
    /// Target node.
    pub to_node: String,
    /// Target side.
    pub to_side: Option<Side>,
    /// Target end (default `arrow`).
    pub to_end: Option<End>,
    /// Colour.
    pub color: Option<String>,
    /// Label (Strata writes the relation type here).
    pub label: Option<String>,
    /// Unknown fields.
    pub extra: Map<String, Value>,
}

/// A canvas. Nodes are in z-order (first is bottom).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Canvas {
    /// Nodes.
    pub nodes: Vec<Node>,
    /// Edges.
    pub edges: Vec<Edge>,
    /// Unknown top-level fields.
    pub extra: Map<String, Value>,
}

struct Fields {
    map: Map<String, Value>,
    what: String,
}

impl Fields {
    fn new(v: Value, what: String) -> Result<Self, CanvasError> {
        match v {
            Value::Object(map) => Ok(Self { map, what }),
            _ => Err(CanvasError::Invalid(format!("{what} is not an object"))),
        }
    }

    fn opt_str(&mut self, key: &str) -> Result<Option<String>, CanvasError> {
        match self.map.remove(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(s)) => Ok(Some(s)),
            Some(_) => Err(CanvasError::Invalid(format!(
                "{}: `{key}` must be a string",
                self.what
            ))),
        }
    }

    fn str(&mut self, key: &str) -> Result<String, CanvasError> {
        self.opt_str(key)?
            .ok_or_else(|| CanvasError::Invalid(format!("{}: missing `{key}`", self.what)))
    }

    fn int(&mut self, key: &str) -> Result<i64, CanvasError> {
        match self.map.remove(key) {
            Some(Value::Number(n)) => n
                .as_i64()
                .or_else(|| {
                    // Some writers emit `100.0`; accept whole numbers only.
                    n.as_f64()
                        .filter(|f| f.fract() == 0.0)
                        .and_then(|f| format!("{f:.0}").parse::<i64>().ok())
                })
                .ok_or_else(|| {
                    CanvasError::Invalid(format!("{}: `{key}` must be an integer", self.what))
                }),
            _ => Err(CanvasError::Invalid(format!(
                "{}: missing integer `{key}`",
                self.what
            ))),
        }
    }

    fn enum_of<T>(
        &mut self,
        key: &str,
        parse: fn(&str) -> Option<T>,
    ) -> Result<Option<T>, CanvasError> {
        self.opt_str(key)?
            .map(|s| {
                parse(&s).ok_or_else(|| {
                    CanvasError::Invalid(format!("{}: bad `{key}` value `{s}`", self.what))
                })
            })
            .transpose()
    }
}

impl Canvas {
    /// Parses a `.canvas` file.
    pub fn from_json(json: &str) -> Result<Self, CanvasError> {
        let mut top = Fields::new(serde_json::from_str(json)?, "canvas".into())?;
        let nodes = match top.map.remove("nodes") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(items)) => items
                .into_iter()
                .enumerate()
                .map(|(i, v)| parse_node(v, i))
                .collect::<Result<_, _>>()?,
            Some(_) => return Err(CanvasError::Invalid("`nodes` must be an array".into())),
        };
        let edges = match top.map.remove("edges") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(items)) => items
                .into_iter()
                .enumerate()
                .map(|(i, v)| parse_edge(v, i))
                .collect::<Result<_, _>>()?,
            Some(_) => return Err(CanvasError::Invalid("`edges` must be an array".into())),
        };
        Ok(Self {
            nodes,
            edges,
            extra: top.map,
        })
    }

    /// Serialises in Obsidian's layout.
    pub fn to_json(&self) -> String {
        let mut out = String::from("{\n");
        let mut sections: Vec<String> = Vec::new();
        sections.push(format!(
            "\t\"nodes\":{}",
            array(self.nodes.iter().map(node_json))
        ));
        sections.push(format!(
            "\t\"edges\":{}",
            array(self.edges.iter().map(edge_json))
        ));
        for (k, v) in &self.extra {
            sections.push(format!("\t{}:{}", json_str(k), v));
        }
        out.push_str(&sections.join(",\n"));
        out.push_str("\n}");
        out
    }

    /// Structural problems (the file still parses; Obsidian may misrender these).
    pub fn validate(&self) -> Vec<CanvasIssue> {
        let mut issues = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for id in self
            .nodes
            .iter()
            .map(|n| &n.id)
            .chain(self.edges.iter().map(|e| &e.id))
        {
            if !seen.insert(id.as_str()) {
                issues.push(CanvasIssue::DuplicateId(id.clone()));
            }
        }
        for n in &self.nodes {
            if n.width <= 0 || n.height <= 0 {
                issues.push(CanvasIssue::BadSize(n.id.clone()));
            }
            if let Some(c) = n.color.as_ref().filter(|c| !is_valid_color(c)) {
                issues.push(CanvasIssue::BadColor(c.clone()));
            }
            if let NodeKind::File {
                subpath: Some(s), ..
            } = &n.kind
                && !s.starts_with('#')
            {
                issues.push(CanvasIssue::BadSubpath(s.clone()));
            }
        }
        for e in &self.edges {
            for node in [&e.from_node, &e.to_node] {
                if !self.nodes.iter().any(|n| &n.id == node) {
                    issues.push(CanvasIssue::DanglingEdge {
                        edge: e.id.clone(),
                        node: node.clone(),
                    });
                }
            }
            if let Some(c) = e.color.as_ref().filter(|c| !is_valid_color(c)) {
                issues.push(CanvasIssue::BadColor(c.clone()));
            }
        }
        issues
    }

    /// Vault paths referenced by file nodes.
    pub fn files(&self) -> Vec<&str> {
        self.nodes
            .iter()
            .filter_map(|n| match &n.kind {
                NodeKind::File { file, .. } => Some(file.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Renames file references after a note move. Returns how many nodes changed.
    pub fn rename_file(&mut self, old: &str, new: &str) -> usize {
        let mut n = 0;
        for node in &mut self.nodes {
            if let NodeKind::File { file, .. } = &mut node.kind
                && file == old
            {
                new.clone_into(file);
                n += 1;
            }
        }
        n
    }
}

fn parse_node(v: Value, i: usize) -> Result<Node, CanvasError> {
    let mut f = Fields::new(v, format!("node {i}"))?;
    let id = f.str("id")?;
    f.what = format!("node `{id}`");
    let ty = f.str("type")?;
    let kind = match ty.as_str() {
        "text" => NodeKind::Text {
            text: f.str("text")?,
        },
        "file" => NodeKind::File {
            file: f.str("file")?,
            subpath: f.opt_str("subpath")?,
        },
        "link" => NodeKind::Link { url: f.str("url")? },
        "group" => NodeKind::Group {
            label: f.opt_str("label")?,
            background: f.opt_str("background")?,
            background_style: f.enum_of("backgroundStyle", BackgroundStyle::parse)?,
        },
        other => {
            return Err(CanvasError::Invalid(format!(
                "node `{id}`: unknown type `{other}`"
            )));
        }
    };
    Ok(Node {
        id,
        kind,
        x: f.int("x")?,
        y: f.int("y")?,
        width: f.int("width")?,
        height: f.int("height")?,
        color: f.opt_str("color")?,
        extra: f.map,
    })
}

fn parse_edge(v: Value, i: usize) -> Result<Edge, CanvasError> {
    let mut f = Fields::new(v, format!("edge {i}"))?;
    let id = f.str("id")?;
    f.what = format!("edge `{id}`");
    Ok(Edge {
        id,
        from_node: f.str("fromNode")?,
        from_side: f.enum_of("fromSide", Side::parse)?,
        from_end: f.enum_of("fromEnd", End::parse)?,
        to_node: f.str("toNode")?,
        to_side: f.enum_of("toSide", Side::parse)?,
        to_end: f.enum_of("toEnd", End::parse)?,
        color: f.opt_str("color")?,
        label: f.opt_str("label")?,
        extra: f.map,
    })
}

fn json_str(s: &str) -> String {
    Value::String(s.to_owned()).to_string()
}

fn array(items: impl Iterator<Item = String>) -> String {
    let items: Vec<String> = items.map(|i| format!("\t\t{i}")).collect();
    if items.is_empty() {
        "[]".into()
    } else {
        format!("[\n{}\n\t]", items.join(",\n"))
    }
}

fn object(fields: Vec<(&str, Value)>, extra: &Map<String, Value>) -> String {
    let parts: Vec<String> = fields
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .chain(extra.iter().map(|(k, v)| (k.clone(), v.clone())))
        .map(|(k, v)| format!("{}:{}", json_str(&k), v))
        .collect();
    format!("{{{}}}", parts.join(","))
}

fn push_opt(fields: &mut Vec<(&'static str, Value)>, key: &'static str, v: Option<&str>) {
    if let Some(v) = v {
        fields.push((key, Value::String(v.to_owned())));
    }
}

fn node_json(n: &Node) -> String {
    let mut f: Vec<(&str, Value)> = vec![("id", Value::String(n.id.clone()))];
    match &n.kind {
        NodeKind::Text { text } => {
            f.push(("type", "text".into()));
            f.push(("text", Value::String(text.clone())));
        }
        NodeKind::File { file, subpath } => {
            f.push(("type", "file".into()));
            f.push(("file", Value::String(file.clone())));
            push_opt(&mut f, "subpath", subpath.as_deref());
        }
        NodeKind::Link { url } => {
            f.push(("type", "link".into()));
            f.push(("url", Value::String(url.clone())));
        }
        NodeKind::Group {
            label,
            background,
            background_style,
        } => {
            f.push(("type", "group".into()));
            push_opt(&mut f, "label", label.as_deref());
            push_opt(&mut f, "background", background.as_deref());
            push_opt(
                &mut f,
                "backgroundStyle",
                background_style.map(BackgroundStyle::as_str),
            );
        }
    }
    f.push(("x", n.x.into()));
    f.push(("y", n.y.into()));
    f.push(("width", n.width.into()));
    f.push(("height", n.height.into()));
    push_opt(&mut f, "color", n.color.as_deref());
    object(f, &n.extra)
}

fn edge_json(e: &Edge) -> String {
    let mut f: Vec<(&str, Value)> = vec![
        ("id", Value::String(e.id.clone())),
        ("fromNode", Value::String(e.from_node.clone())),
    ];
    push_opt(&mut f, "fromSide", e.from_side.map(Side::as_str));
    push_opt(&mut f, "fromEnd", e.from_end.map(End::as_str));
    f.push(("toNode", Value::String(e.to_node.clone())));
    push_opt(&mut f, "toSide", e.to_side.map(Side::as_str));
    push_opt(&mut f, "toEnd", e.to_end.map(End::as_str));
    push_opt(&mut f, "color", e.color.as_deref());
    push_opt(&mut f, "label", e.label.as_deref());
    object(f, &e.extra)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "{\n\t\"nodes\":[\n\t\t{\"id\":\"g\",\"type\":\"group\",\"label\":\"Pricing\",\"x\":-400,\"y\":-300,\"width\":900,\"height\":700,\"color\":\"4\"},\n\t\t{\"id\":\"a\",\"type\":\"file\",\"file\":\"notes/Pricing experiments.md\",\"subpath\":\"#Summary\",\"x\":-300,\"y\":-200,\"width\":400,\"height\":300},\n\t\t{\"id\":\"b\",\"type\":\"text\",\"text\":\"ملاحظة **مهمة**\",\"x\":200,\"y\":-200,\"width\":250,\"height\":60,\"color\":\"#ff8800\"},\n\t\t{\"id\":\"c\",\"type\":\"link\",\"url\":\"https://jsoncanvas.org\",\"x\":200,\"y\":0,\"width\":250,\"height\":100,\"styleAttributes\":{}}\n\t],\n\t\"edges\":[\n\t\t{\"id\":\"e1\",\"fromNode\":\"a\",\"fromSide\":\"right\",\"toNode\":\"b\",\"toSide\":\"left\",\"toEnd\":\"arrow\",\"label\":\"contradicts\"}\n\t]\n}";

    #[test]
    fn round_trips_obsidian_layout() {
        let c = Canvas::from_json(SAMPLE);
        assert!(c.is_ok(), "{c:?}");
        let c = c.unwrap_or_default();
        assert_eq!(c.nodes.len(), 4);
        assert_eq!(c.files(), ["notes/Pricing experiments.md"]);
        assert_eq!(c.validate(), vec![]);
        assert_eq!(c.to_json(), SAMPLE);
    }

    #[test]
    fn validation_and_errors() {
        let json = r#"{"nodes":[{"id":"a","type":"file","file":"x.md","subpath":"Head","x":0,"y":0,"width":0,"height":10,"color":"9"},{"id":"a","type":"text","text":"","x":0,"y":0,"width":1,"height":1}],"edges":[{"id":"e","fromNode":"a","toNode":"zz","color":"red"}]}"#;
        let c = Canvas::from_json(json).unwrap_or_default();
        assert_eq!(
            c.validate(),
            vec![
                CanvasIssue::DuplicateId("a".into()),
                CanvasIssue::BadSize("a".into()),
                CanvasIssue::BadColor("9".into()),
                CanvasIssue::BadSubpath("Head".into()),
                CanvasIssue::DanglingEdge {
                    edge: "e".into(),
                    node: "zz".into()
                },
                CanvasIssue::BadColor("red".into()),
            ]
        );
        for bad in [
            "[]",
            r#"{"nodes":{}}"#,
            r#"{"nodes":[{"id":"a","type":"blob","x":0,"y":0,"width":1,"height":1}]}"#,
            r#"{"nodes":[{"id":"a","type":"text","x":0,"y":0,"width":1,"height":1}]}"#,
            r#"{"nodes":[{"id":"a","type":"text","text":"","x":0.5,"y":0,"width":1,"height":1}]}"#,
            r#"{"edges":[{"id":"e","fromNode":"a","toNode":"b","toEnd":"circle"}]}"#,
            "not json",
        ] {
            assert!(Canvas::from_json(bad).is_err(), "{bad}");
        }
        assert_eq!(
            Canvas::default().to_json(),
            "{\n\t\"nodes\":[],\n\t\"edges\":[]\n}"
        );
    }

    #[test]
    fn renames_files() {
        let mut c = Canvas::from_json(SAMPLE).unwrap_or_default();
        assert_eq!(
            c.rename_file("notes/Pricing experiments.md", "archive/Pricing.md"),
            1
        );
        assert_eq!(c.files(), ["archive/Pricing.md"]);
    }
}
