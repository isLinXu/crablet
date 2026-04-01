//! Desktop Automation
//!
//! Provides cross-platform desktop automation for mouse, keyboard, clipboard, and screen operations.
//! Every operation passes through the RpaSafetyLayer for security validation.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde::de::{self, Visitor};
use serde::{Deserializer, Serializer};
use tracing::{debug, info, warn};

use crate::rpa::{RpaError, RpaResult};
use crate::rpa::safety::RpaSafetyLayer;

/// Desktop automation engine
pub struct DesktopAutomation {
    #[cfg(feature = "auto-working")]
    enigo: enigo::Enigo,
    safety: RpaSafetyLayer,
}

impl DesktopAutomation {
    /// Create a new desktop automation instance with default safety config
    pub fn new() -> RpaResult<Self> {
        Self::with_safety(RpaSafetyLayer::new())
    }

    /// Create a new desktop automation instance with custom safety config
    pub fn with_safety(safety: RpaSafetyLayer) -> RpaResult<Self> {
        #[cfg(feature = "auto-working")]
        {
            let enigo = enigo::Enigo::new(&enigo::Settings::default())
                .map_err(|e| RpaError::DesktopError(format!("Failed to initialize enigo: {}", e)))?;

            Ok(Self { enigo, safety })
        }

        #[cfg(not(feature = "auto-working"))]
        {
            Ok(Self { safety })
        }
    }

    /// Get a reference to the safety layer
    pub fn safety(&self) -> &RpaSafetyLayer {
        &self.safety
    }

    /// Execute a desktop workflow with safety checks
    pub async fn execute_workflow(&mut self, workflow: &DesktopWorkflow) -> RpaResult<WorkflowExecutionResult> {
        info!("Starting desktop workflow: {}", workflow.name);

        let start = std::time::Instant::now();
        let mut variables: HashMap<String, String> = workflow.variables.clone();
        let mut screenshots: Vec<String> = vec![];

        for (i, step) in workflow.steps.iter().enumerate() {
            debug!("Executing step {}: {:?}", i + 1, step);

            // Safety check before every step
            let decision = self.safety.check_step(step, None, Some(&workflow.name)).await;
            match decision {
                crate::rpa::safety::RpaSafetyDecision::Allow => {}
                crate::rpa::safety::RpaSafetyDecision::Block(reason) => {
                    warn!("Step {} blocked by safety layer: {}", i + 1, reason);
                    return Err(RpaError::DesktopError(format!(
                        "Step {} blocked: {}", i + 1, reason
                    )));
                }
                crate::rpa::safety::RpaSafetyDecision::RequireConfirmation(reason) => {
                    warn!("Step {} requires confirmation: {}", i + 1, reason);
                    // In non-interactive mode, we skip the step with a warning
                    continue;
                }
                crate::rpa::safety::RpaSafetyDecision::Warn(reason) => {
                    warn!("Step {} warning: {}", i + 1, reason);
                }
            }

            match step {
                DesktopStep::MouseMove { x, y } => {
                    debug!("Moving mouse to ({}, {})", x, y);

                    #[cfg(feature = "auto-working")]
                    {
                        use enigo::Mouse;
                        self.enigo.move_mouse(*x, *y, enigo::Coordinate::Abs)
                            .map_err(|e| RpaError::DesktopError(e.to_string()))?;
                    }
                }
                DesktopStep::MouseClick { button } => {
                    debug!("Clicking mouse: {:?}", button);

                    #[cfg(feature = "auto-working")]
                    {
                        use enigo::{Mouse, Button};
                        let btn = match button {
                            MouseButton::Left => Button::Left,
                            MouseButton::Right => Button::Right,
                            MouseButton::Middle => Button::Middle,
                        };

                        self.enigo.button(btn, enigo::Direction::Click)
                            .map_err(|e| RpaError::DesktopError(e.to_string()))?;
                    }
                }
                DesktopStep::MouseDrag { from, to } => {
                    debug!("Dragging from ({}, {}) to ({}, {})", from.x, from.y, to.x, to.y);

                    #[cfg(feature = "auto-working")]
                    {
                        use enigo::Mouse;

                        self.enigo.move_mouse(from.x, from.y, enigo::Coordinate::Abs)
                            .map_err(|e| RpaError::DesktopError(e.to_string()))?;

                        self.enigo.button(enigo::Button::Left, enigo::Direction::Press)
                            .map_err(|e| RpaError::DesktopError(e.to_string()))?;

                        tokio::time::sleep(Duration::from_millis(50)).await;

                        self.enigo.move_mouse(to.x, to.y, enigo::Coordinate::Abs)
                            .map_err(|e| RpaError::DesktopError(e.to_string()))?;

                        self.enigo.button(enigo::Button::Left, enigo::Direction::Release)
                            .map_err(|e| RpaError::DesktopError(e.to_string()))?;
                    }
                }
                DesktopStep::KeyboardType { text } => {
                    let resolved_text = self.resolve_variables(text, &variables);
                    debug!("Typing: {}", &resolved_text[..resolved_text.len().min(80)]);

                    #[cfg(feature = "auto-working")]
                    {
                        use enigo::Keyboard;
                        self.enigo.text(&resolved_text)
                            .map_err(|e| RpaError::DesktopError(e.to_string()))?;
                    }
                }
                DesktopStep::KeyboardHotkey { keys } => {
                    debug!("Pressing hotkey: {:?}", keys);

                    #[cfg(feature = "auto-working")]
                    {
                        use enigo::Keyboard;

                        for key in keys {
                            let enigo_key = Self::convert_key(key);
                            self.enigo.key(enigo_key, enigo::Direction::Press)
                                .map_err(|e| RpaError::DesktopError(e.to_string()))?;
                        }

                        tokio::time::sleep(Duration::from_millis(50)).await;

                        for key in keys.iter().rev() {
                            let enigo_key = Self::convert_key(key);
                            self.enigo.key(enigo_key, enigo::Direction::Release)
                                .map_err(|e| RpaError::DesktopError(e.to_string()))?;
                        }
                    }
                }
                DesktopStep::Screenshot { region, path } => {
                    let resolved_path = self.resolve_variables(path, &variables);
                    debug!("Taking screenshot: {} (region: {:?})", resolved_path, region);

                    match Self::capture_screenshot(&resolved_path, *region).await {
                        Ok(saved_path) => {
                            info!("Screenshot saved to: {}", saved_path);
                            screenshots.push(saved_path.clone());
                            variables.insert("last_screenshot".to_string(), saved_path);
                        }
                        Err(e) => {
                            warn!("Screenshot failed: {}", e);
                            return Err(e);
                        }
                    }
                }
                DesktopStep::Wait { seconds } => {
                    debug!("Waiting {} seconds", seconds);
                    tokio::time::sleep(Duration::from_secs(*seconds)).await;
                }
                DesktopStep::ClipboardSet { content } => {
                    let resolved_content = self.resolve_variables(content, &variables);
                    debug!("Setting clipboard: {} bytes", resolved_content.len());

                    match Self::set_clipboard(&resolved_content) {
                        Ok(()) => {
                            debug!("Clipboard updated successfully");
                        }
                        Err(e) => {
                            warn!("Clipboard set failed: {}", e);
                            return Err(e);
                        }
                    }
                }
                DesktopStep::ClipboardGet { variable } => {
                    debug!("Getting clipboard to variable: {}", variable);

                    match Self::get_clipboard() {
                        Ok(content) => {
                            debug!("Clipboard content: {} bytes", content.len());
                            variables.insert(variable.clone(), content);
                        }
                        Err(e) => {
                            warn!("Clipboard get failed: {}", e);
                            return Err(e);
                        }
                    }
                }
                DesktopStep::FindAndClick { image, confidence } => {
                    debug!("Finding image '{}' and clicking (confidence: {})", image, confidence);

                    match Self::find_and_click(image, *confidence).await {
                        Ok((x, y, actual_confidence)) => {
                            info!("Found image '{}' at ({}, {}) with confidence {}",
                                image, x, y, actual_confidence);
                            variables.insert("click_x".to_string(), x.to_string());
                            variables.insert("click_y".to_string(), y.to_string());
                            variables.insert("match_confidence".to_string(), actual_confidence.to_string());

                            // Perform the actual click
                            #[cfg(feature = "auto-working")]
                            {
                                use enigo::{Mouse, Button};
                                self.enigo.move_mouse(x, y, enigo::Coordinate::Abs)
                                    .map_err(|e| RpaError::DesktopError(e.to_string()))?;
                                tokio::time::sleep(Duration::from_millis(100)).await;
                                self.enigo.button(Button::Left, enigo::Direction::Click)
                                    .map_err(|e| RpaError::DesktopError(e.to_string()))?;
                            }
                        }
                        Err(e) => {
                            warn!("Image '{}' not found: {}", image, e);
                            variables.insert("image_found".to_string(), "false".to_string());
                            // Non-fatal: image not found is a soft error
                            return Err(e);
                        }
                    }
                }
            }
        }

        info!("Desktop workflow completed in {:?}", start.elapsed());

        Ok(WorkflowExecutionResult {
            success: true,
            execution_time: start.elapsed(),
            variables,
            screenshots,
        })
    }

    /// Capture a screenshot of the screen (or a specific region)
    pub async fn capture_screenshot(path: &str, region: Option<Region>) -> RpaResult<String> {
        Self::capture_screenshot_impl(path, region).await
    }

    /// Internal: capture screenshot using platform APIs
    async fn capture_screenshot_impl(path: &str, region: Option<Region>) -> RpaResult<String> {
        #[cfg(feature = "auto-working")]
        {
            use screenshots::Screen;

            let screen = Screen::all().map_err(|e| {
                RpaError::DesktopError(format!("Failed to enumerate screens: {}", e))
            })?;

            let primary = screen.first().ok_or_else(|| {
                RpaError::DesktopError("No screen found".to_string())
            })?;

            let image = if let Some(r) = region {
                primary.capture_area(r.x, r.y, r.width, r.height)
            } else {
                primary.capture()
            }.map_err(|e| {
                RpaError::DesktopError(format!("Failed to capture screenshot: {}", e))
            })?;

            // Ensure parent directory exists
            let path_buf = PathBuf::from(path);
            if let Some(parent) = path_buf.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| RpaError::DesktopError(format!("Failed to create directory: {}", e)))?;
            }

            image.save(path).map_err(|e| {
                RpaError::DesktopError(format!("Failed to save screenshot: {}", e))
            })?;

            Ok(path.to_string())
        }

        #[cfg(not(feature = "auto-working"))]
        {
            // Simulated: create a 1x1 pixel PNG as placeholder
            let path_buf = PathBuf::from(path);
            if let Some(parent) = path_buf.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            // Write a minimal valid PNG (1x1 transparent pixel)
            let minimal_png: &[u8] = &[
                0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D,
                0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
                0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00,
                0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x62, 0x00, 0x00, 0x00, 0x02,
                0x00, 0x01, 0xE2, 0x21, 0xBC, 0x33,
            ];
            std::fs::write(path, minimal_png)
                .map_err(|e| RpaError::DesktopError(format!("Failed to write placeholder: {}", e)))?;
            Ok(path.to_string())
        }
    }

    /// Set clipboard content
    pub fn set_clipboard(content: &str) -> RpaResult<()> {
        #[cfg(feature = "auto-working")]
        {
            let mut clipboard = arboard::Clipboard::new()
                .map_err(|e| RpaError::DesktopError(format!("Failed to access clipboard: {}", e)))?;

            clipboard.set_text(content)
                .map_err(|e| RpaError::DesktopError(format!("Failed to set clipboard: {}", e)))?;
            Ok(())
        }

        #[cfg(not(feature = "auto-working"))]
        {
            let _ = content;
            Ok(())
        }
    }

    /// Get clipboard content
    pub fn get_clipboard() -> RpaResult<String> {
        #[cfg(feature = "auto-working")]
        {
            let mut clipboard = arboard::Clipboard::new()
                .map_err(|e| RpaError::DesktopError(format!("Failed to access clipboard: {}", e)))?;

            clipboard.get_text()
                .map_err(|e| RpaError::DesktopError(format!("Failed to get clipboard: {}", e)))
        }

        #[cfg(not(feature = "auto-working"))]
        {
            Ok(String::new())
        }
    }

    /// Find an image on screen and click on it
    /// Returns (x, y, actual_confidence)
    pub async fn find_and_click(image_path: &str, min_confidence: f32) -> RpaResult<(i32, i32, f32)> {
        // Step 1: Take a full screenshot
        let screenshot_path = format!("/tmp/crablet_rpa_search_{}.png",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        Self::capture_screenshot(&screenshot_path, None).await?;

        // Step 2: Load both images
        let screenshot_img = image::open(&screenshot_path)
            .map_err(|e| RpaError::DesktopError(format!("Failed to load screenshot: {}", e)))?;

        let template_img = image::open(image_path)
            .map_err(|e| RpaError::DesktopError(format!("Failed to load template '{}': {}", image_path, e)))?;

        // Step 3: Template matching using normalized cross-correlation (NCC)
        let screenshot_gray = screenshot_img.to_luma8();
        let template_gray = template_img.to_luma8();

        let (tw, th) = template_gray.dimensions();
        let (sw, sh) = screenshot_gray.dimensions();

        if tw > sw || th > sh {
            return Err(RpaError::DesktopError(format!(
                "Template ({}x{}) is larger than screenshot ({}x{})",
                tw, th, sw, sh
            )));
        }

        // Compute NCC over the search area
        let mut best_x: i32 = 0;
        let mut best_y: i32 = 0;
        let mut best_score: f32 = -1.0;

        // Sample step for performance (can be 1 for pixel-perfect matching)
        let step = if (sw - tw) > 2000 || (sh - th) > 2000 { 4 } else { 2 };

        for y in 0..=(sh - th).saturating_sub(step as u32) {
            let yi = y as usize;
            for x in 0..=(sw - tw).saturating_sub(step as u32) {
                let xi = x as usize;
                let score = compute_ncc(
                    &screenshot_gray,
                    &template_gray,
                    xi, yi,
                    tw as usize, th as usize,
                );

                if score > best_score {
                    best_score = score;
                    best_x = xi as i32;
                    best_y = yi as i32;
                }
            }
        }

        // Clean up temporary screenshot
        let _ = std::fs::remove_file(&screenshot_path);

        if best_score >= min_confidence {
            Ok((best_x + (tw / 2) as i32, best_y + (th / 2) as i32, best_score))
        } else {
            Err(RpaError::DesktopError(format!(
                "Image '{}' not found on screen. Best match: {} at ({}, {}), required: {}",
                image_path, best_score, best_x, best_y, min_confidence
            )))
        }
    }

    /// Convert our Key enum to enigo::Key
    #[cfg(feature = "auto-working")]
    fn convert_key(key: &Key) -> enigo::Key {
        match key {
            Key::Control => enigo::Key::Control,
            Key::Alt => enigo::Key::Alt,
            Key::Shift => enigo::Key::Shift,
            Key::Meta => enigo::Key::Meta,
            Key::Return => enigo::Key::Return,
            Key::Escape => enigo::Key::Escape,
            Key::Tab => enigo::Key::Tab,
            Key::Space => enigo::Key::Space,
            Key::Backspace => enigo::Key::Backspace,
            Key::Delete => enigo::Key::Delete,
            Key::Home => enigo::Key::Home,
            Key::End => enigo::Key::End,
            Key::PageUp => enigo::Key::PageUp,
            Key::PageDown => enigo::Key::PageDown,
            Key::Left => enigo::Key::LeftArrow,
            Key::Right => enigo::Key::RightArrow,
            Key::Up => enigo::Key::UpArrow,
            Key::Down => enigo::Key::DownArrow,
            Key::F(n) => match n {
                1 => enigo::Key::F1,
                2 => enigo::Key::F2,
                3 => enigo::Key::F3,
                4 => enigo::Key::F4,
                5 => enigo::Key::F5,
                6 => enigo::Key::F6,
                7 => enigo::Key::F7,
                8 => enigo::Key::F8,
                9 => enigo::Key::F9,
                10 => enigo::Key::F10,
                11 => enigo::Key::F11,
                12 => enigo::Key::F12,
                _ => enigo::Key::Unicode('?'),
            },
            Key::Char(c) => enigo::Key::Unicode(*c),
        }
    }

    /// Resolve variables in a string
    fn resolve_variables(&self, text: &str, variables: &HashMap<String, String>) -> String {
        let mut result = text.to_string();
        for (key, value) in variables {
            result = result.replace(&format!("{{{{{}}}}}", key), value);
        }
        result
    }
}

/// Compute Normalized Cross-Correlation between a region of `source` and `template`
fn compute_ncc(
    source: &image::GrayImage,
    template: &image::GrayImage,
    sx: usize,
    sy: usize,
    tw: usize,
    th: usize,
) -> f32 {
    // Compute mean of source patch
    let mut src_sum: f64 = 0.0;
    let mut tpl_sum: f64 = 0.0;
    let pixels = tw * th;

    for y in 0..th {
        for x in 0..tw {
            src_sum += source.get_pixel((sx + x) as u32, (sy + y) as u32).0[0] as f64;
            tpl_sum += template.get_pixel(x as u32, y as u32).0[0] as f64;
        }
    }

    let src_mean = src_sum / pixels as f64;
    let tpl_mean = tpl_sum / pixels as f64;

    // Compute NCC
    let mut numerator: f64 = 0.0;
    let mut src_var: f64 = 0.0;
    let mut tpl_var: f64 = 0.0;

    for y in 0..th {
        for x in 0..tw {
            let sv = source.get_pixel((sx + x) as u32, (sy + y) as u32).0[0] as f64 - src_mean;
            let tv = template.get_pixel(x as u32, y as u32).0[0] as f64 - tpl_mean;
            numerator += sv * tv;
            src_var += sv * sv;
            tpl_var += tv * tv;
        }
    }

    // Handle constant patches (zero variance) deterministically.
    // If both patches are constant and have same mean, treat as perfect match.
    if src_var < 1e-12 && tpl_var < 1e-12 {
        return if (src_mean - tpl_mean).abs() < 1e-9 { 1.0 } else { 0.0 };
    }

    let denom = (src_var.sqrt() * tpl_var.sqrt()).max(0.001);
    (numerator / denom) as f32
}

/// Desktop workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopWorkflow {
    pub name: String,
    pub steps: Vec<DesktopStep>,
    #[serde(default)]
    pub variables: HashMap<String, String>,
}

/// Desktop automation step
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum DesktopStep {
    /// Move mouse to position
    MouseMove { x: i32, y: i32 },
    /// Click mouse button
    MouseClick { button: MouseButton },
    /// Drag mouse from one position to another
    MouseDrag { from: Point, to: Point },
    /// Type text
    KeyboardType { text: String },
    /// Press hotkey combination
    KeyboardHotkey { keys: Vec<Key> },
    /// Take screenshot
    Screenshot { region: Option<Region>, path: String },
    /// Wait for specified seconds
    Wait { seconds: u64 },
    /// Set clipboard content
    ClipboardSet { content: String },
    /// Get clipboard content to variable
    ClipboardGet { variable: String },
    /// Find image on screen and click
    FindAndClick { image: String, confidence: f32 },
}

/// Mouse button
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Keyboard key
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Control,
    Alt,
    Shift,
    Meta,
    Return,
    Escape,
    Tab,
    Space,
    Backspace,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    Left,
    Right,
    Up,
    Down,
    F(u8),
    Char(char),
}

impl Serialize for Key {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = match self {
            Key::Control => "control".to_string(),
            Key::Alt => "alt".to_string(),
            Key::Shift => "shift".to_string(),
            Key::Meta => "meta".to_string(),
            Key::Return => "return".to_string(),
            Key::Escape => "escape".to_string(),
            Key::Tab => "tab".to_string(),
            Key::Space => "space".to_string(),
            Key::Backspace => "backspace".to_string(),
            Key::Delete => "delete".to_string(),
            Key::Home => "home".to_string(),
            Key::End => "end".to_string(),
            Key::PageUp => "page_up".to_string(),
            Key::PageDown => "page_down".to_string(),
            Key::Left => "left".to_string(),
            Key::Right => "right".to_string(),
            Key::Up => "up".to_string(),
            Key::Down => "down".to_string(),
            Key::F(n) => format!("f{}", n),
            Key::Char(c) => c.to_string(),
        };
        serializer.serialize_str(&s)
    }
}

impl<'de> Deserialize<'de> for Key {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct KeyVisitor;

        impl<'de> Visitor<'de> for KeyVisitor {
            type Value = Key;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a key name like 'control', 'f1', or a single character")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let s = v.trim().to_lowercase();
                match s.as_str() {
                    "control" => Ok(Key::Control),
                    "alt" => Ok(Key::Alt),
                    "shift" => Ok(Key::Shift),
                    "meta" => Ok(Key::Meta),
                    "return" | "enter" => Ok(Key::Return),
                    "escape" | "esc" => Ok(Key::Escape),
                    "tab" => Ok(Key::Tab),
                    "space" => Ok(Key::Space),
                    "backspace" => Ok(Key::Backspace),
                    "delete" | "del" => Ok(Key::Delete),
                    "home" => Ok(Key::Home),
                    "end" => Ok(Key::End),
                    "page_up" | "pageup" => Ok(Key::PageUp),
                    "page_down" | "pagedown" => Ok(Key::PageDown),
                    "left" => Ok(Key::Left),
                    "right" => Ok(Key::Right),
                    "up" => Ok(Key::Up),
                    "down" => Ok(Key::Down),
                    _ => {
                        if let Some(rest) = s.strip_prefix('f') {
                            if let Ok(n) = rest.parse::<u8>() {
                                return Ok(Key::F(n));
                            }
                        }
                        let mut chars = s.chars();
                        let c1 = chars.next().ok_or_else(|| E::custom("empty key string"))?;
                        if chars.next().is_none() {
                            Ok(Key::Char(c1))
                        } else {
                            Err(E::custom(format!("unknown key: {}", v)))
                        }
                    }
                }
            }
        }

        deserializer.deserialize_str(KeyVisitor)
    }
}

/// Point on screen
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

/// Screen region
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Workflow execution result
#[derive(Debug)]
pub struct WorkflowExecutionResult {
    pub success: bool,
    pub execution_time: Duration,
    pub variables: HashMap<String, String>,
    pub screenshots: Vec<String>,
}

/// Desktop platform trait for abstraction
#[async_trait]
pub trait DesktopPlatform: Send + Sync {
    /// Move mouse to position
    async fn mouse_move(&mut self, x: i32, y: i32) -> RpaResult<()>;
    /// Click mouse button
    async fn mouse_click(&mut self, button: MouseButton) -> RpaResult<()>;
    /// Type text
    async fn keyboard_type(&mut self, text: &str) -> RpaResult<()>;
    /// Press hotkey
    async fn keyboard_hotkey(&mut self, keys: &[Key]) -> RpaResult<()>;
    /// Take screenshot
    async fn screenshot(&self, region: Option<Region>) -> RpaResult<Vec<u8>>;
    /// Get clipboard
    async fn get_clipboard(&self) -> RpaResult<String>;
    /// Set clipboard
    async fn set_clipboard(&mut self, content: &str) -> RpaResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_desktop_workflow_serialization() {
        let workflow = DesktopWorkflow {
            name: "Test Workflow".to_string(),
            steps: vec![
                DesktopStep::MouseMove { x: 100, y: 200 },
                DesktopStep::MouseClick { button: MouseButton::Left },
                DesktopStep::KeyboardType { text: "Hello".to_string() },
            ],
            variables: HashMap::new(),
        };

        let yaml = serde_yaml::to_string(&workflow).unwrap();
        assert!(yaml.contains("Test Workflow"));
        assert!(yaml.contains("mouse_move"));
        assert!(yaml.contains("mouse_click"));
    }

    #[test]
    fn test_key_serialization() {
        let keys = vec![Key::Control, Key::Char('c')];
        let yaml = serde_yaml::to_string(&keys).unwrap();
        assert!(yaml.contains("control"));
        assert!(yaml.contains("c"));
    }

    #[test]
    fn test_all_step_types_serialization() {
        // Ensure all step types round-trip through YAML
        let steps = vec![
            DesktopStep::MouseMove { x: 50, y: 50 },
            DesktopStep::MouseClick { button: MouseButton::Right },
            DesktopStep::MouseDrag {
                from: Point { x: 0, y: 0 },
                to: Point { x: 100, y: 100 },
            },
            DesktopStep::KeyboardType { text: "test".to_string() },
            DesktopStep::KeyboardHotkey {
                keys: vec![Key::Control, Key::Char('v')],
            },
            DesktopStep::Screenshot {
                region: Some(Region { x: 0, y: 0, width: 800, height: 600 }),
                path: "/tmp/test.png".to_string(),
            },
            DesktopStep::Wait { seconds: 1 },
            DesktopStep::ClipboardSet { content: "hello".to_string() },
            DesktopStep::ClipboardGet { variable: "clip".to_string() },
            DesktopStep::FindAndClick {
                image: "button.png".to_string(),
                confidence: 0.8,
            },
        ];

        let workflow = DesktopWorkflow {
            name: "Full Coverage Test".to_string(),
            steps,
            variables: HashMap::new(),
        };

        let yaml = serde_yaml::to_string(&workflow).unwrap();
        let deserialized: DesktopWorkflow = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(deserialized.name, workflow.name);
        assert_eq!(deserialized.steps.len(), workflow.steps.len());
    }

    #[test]
    fn test_region_serialization() {
        let region = Region { x: 10, y: 20, width: 800, height: 600 };
        let yaml = serde_yaml::to_string(&region).unwrap();
        assert!(yaml.contains("x: 10"));
        assert!(yaml.contains("y: 20"));
        assert!(yaml.contains("width: 800"));
        assert!(yaml.contains("height: 600"));
    }

    #[test]
    fn test_variable_resolution() {
        let variables: HashMap<String, String> = [
            ("name".to_string(), "world".to_string()),
            ("count".to_string(), "42".to_string()),
        ].into_iter().collect();

        let text = "Hello {{name}}, count={{count}}!";
        let engine = DesktopAutomation::new().unwrap();
        let resolved = engine.resolve_variables(text, &variables);
        assert_eq!(resolved, "Hello world, count=42!");
    }

    #[test]
    fn test_ncc_identical_images() {
        // NCC of identical image regions should be ~1.0
        let img_data = vec![128u8; 10 * 10];
        let img = image::GrayImage::from_raw(10, 10, img_data).unwrap();

        let score = compute_ncc(&img, &img, 0, 0, 10, 10);
        assert!((score - 1.0).abs() < 0.001, "NCC of identical regions should be ~1.0, got {}", score);
    }

    #[test]
    fn test_ncc_different_images() {
        // NCC of completely different images should be near 0
        let dark_data = vec![0u8; 10 * 10];
        let bright_data = vec![255u8; 10 * 10];
        let dark = image::GrayImage::from_raw(10, 10, dark_data).unwrap();
        let bright = image::GrayImage::from_raw(10, 10, bright_data).unwrap();

        let score = compute_ncc(&dark, &bright, 0, 0, 10, 10);
        assert!(score.abs() < 0.1, "NCC of black vs white should be near 0, got {}", score);
    }

    #[tokio::test]
    async fn test_desktop_automation_creation() {
        let engine = DesktopAutomation::new();
        assert!(engine.is_ok());
    }
}
