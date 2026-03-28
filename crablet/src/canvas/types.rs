//! Canvas Types Module
//!
//! Defines core types for the canvas system.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Unique identifier for canvas elements
pub type ElementId = Uuid;

/// Unique identifier for canvas sessions
pub type SessionId = Uuid;

/// Canvas element (any drawable object)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasElement {
    pub id: ElementId,
    pub element_type: ElementType,
    pub bounds: Rect,
    pub style: Style,
    pub transform: Transform2D,
    pub visible: bool,
    pub locked: bool,
    pub z_index: i32,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl CanvasElement {
    /// Create a new canvas element
    pub fn new(element_type: ElementType, bounds: Rect) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            element_type,
            bounds,
            style: Style::default(),
            transform: Transform2D::identity(),
            visible: true,
            locked: false,
            z_index: 0,
            metadata: serde_json::json!({}),
            created_at: now,
            updated_at: now,
        }
    }
}

/// Type of canvas element
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ElementType {
    Rectangle(RectangleData),
    Circle(CircleData),
    Line(LineData),
    Text(TextData),
    Image(ImageData),
    Path(PathData),
    Group(GroupData),
}

/// Rectangle element data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RectangleData {
    pub corner_radius: f64,
}

/// Circle element data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircleData {
    pub is_ellipse: bool,
}

/// Line element data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineData {
    pub start_point: Point,
    pub end_point: Point,
    pub arrow_start: bool,
    pub arrow_end: bool,
}

/// Text element data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextData {
    pub content: String,
    pub font_family: String,
    pub font_size: f64,
    pub font_weight: FontWeight,
    pub text_align: TextAlign,
    pub vertical_align: VerticalAlign,
}

/// Image element data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageData {
    pub src: String,
    pub natural_width: u32,
    pub natural_height: u32,
}

/// Path element data (SVG path)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathData {
    pub d: String,
    pub fill_rule: FillRule,
}

/// Group element data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupData {
    pub children: Vec<ElementId>,
}

/// 2D Transform
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Transform2D {
    pub translate_x: f64,
    pub translate_y: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    pub rotation: f64, // degrees
}

impl Transform2D {
    /// Create identity transform
    pub fn identity() -> Self {
        Self {
            translate_x: 0.0,
            translate_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
        }
    }
}

/// 2D Point
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// 2D Rectangle
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    /// Create a new rectangle
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { x, y, width, height }
    }

    /// Check if point is inside
    pub fn contains(&self, point: &Point) -> bool {
        point.x >= self.x && point.x <= self.x + self.width &&
        point.y >= self.y && point.y <= self.y + self.height
    }
}

/// Style properties
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Style {
    pub fill: Option<Color>,
    pub stroke: Option<StrokeStyle>,
    pub shadow: Option<ShadowStyle>,
    pub opacity: f64,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            fill: Some(Color::default()),
            stroke: Some(StrokeStyle::default()),
            shadow: None,
            opacity: 1.0,
        }
    }
}

/// Color RGBA
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Default for Color {
    fn default() -> Self {
        Self { r: 0, g: 0, b: 0, a: 255 }
    }
}

impl Color {
    /// Create from hex string
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 && hex.len() != 8 {
            return None;
        }
        
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        let a = if hex.len() == 8 {
            u8::from_str_radix(&hex[6..8], 16).ok()?
        } else {
            255
        };
        
        Some(Self { r, g, b, a })
    }
}

/// Stroke style
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrokeStyle {
    pub color: Color,
    pub width: f64,
    pub line_cap: LineCap,
    pub line_join: LineJoin,
    pub dash_array: Vec<f64>,
}

impl Default for StrokeStyle {
    fn default() -> Self {
        Self {
            color: Color { r: 0, g: 0, b: 0, a: 255 },
            width: 1.0,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            dash_array: vec![],
        }
    }
}

/// Shadow style
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowStyle {
    pub color: Color,
    pub offset_x: f64,
    pub offset_y: f64,
    pub blur: f64,
}

/// Line cap
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

/// Line join
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

/// Fill rule for paths
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FillRule {
    NonZero,
    EvenOdd,
}

/// Font weight
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FontWeight {
    Normal,
    Bold,
    Bolder,
    Lighter,
    Weight(u16),
}

/// Text alignment
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

/// Vertical alignment
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VerticalAlign {
    Top,
    Middle,
    Bottom,
}

/// Viewport (visible area)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Viewport {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub zoom: f64,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
            zoom: 1.0,
        }
    }
}

impl Viewport {
    /// Convert screen coordinates to canvas coordinates
    pub fn screen_to_canvas(&self, screen_x: f64, screen_y: f64) -> Point {
        Point {
            x: (screen_x - self.x) / self.zoom,
            y: (screen_y - self.y) / self.zoom,
        }
    }

    /// Convert canvas coordinates to screen coordinates
    pub fn canvas_to_screen(&self, canvas_x: f64, canvas_y: f64) -> Point {
        Point {
            x: canvas_x * self.zoom + self.x,
            y: canvas_y * self.zoom + self.y,
        }
    }
}

/// Selection state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Selection {
    pub element_ids: Vec<ElementId>,
    pub bounds: Option<Rect>,
}

/// Canvas session (one canvas document)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasSession {
    pub id: SessionId,
    pub name: String,
    pub elements: Vec<ElementId>,
    pub layers: Vec<LayerId>,
    pub viewport: Viewport,
    pub selection: Selection,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl CanvasSession {
    /// Create a new canvas session
    pub fn new(name: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            elements: vec![],
            layers: vec![],
            viewport: Viewport::default(),
            selection: Selection { element_ids: vec![], bounds: None },
            created_at: now,
            updated_at: now,
        }
    }
}

/// Layer identifier
pub type LayerId = Uuid;

/// Canvas layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub opacity: f64,
    pub element_ids: Vec<ElementId>,
}

impl Layer {
    /// Create a new layer
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            visible: true,
            locked: false,
            opacity: 1.0,
            element_ids: vec![],
        }
    }
}