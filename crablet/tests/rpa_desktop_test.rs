//! RPA Desktop 模块测试
//!
//! 运行: cargo test --test rpa_desktop_test

#![cfg(feature = "auto-working")]

use crablet::rpa::desktop::{DesktopStep, DesktopWorkflow, Key, MouseButton, Point, Region};

#[test]
fn test_desktop_workflow_find_and_click_serialization() {
    let workflow = DesktopWorkflow {
        name: "Image Click Test".to_string(),
        steps: vec![
            DesktopStep::FindAndClick {
                image: "button.png".to_string(),
                confidence: 0.85,
            },
        ],
        variables: std::collections::HashMap::new(),
    };

    let yaml = serde_yaml::to_string(&workflow).unwrap();
    assert!(yaml.contains("Image Click Test"));
    assert!(yaml.contains("find_and_click"));
    assert!(yaml.contains("button.png"));
    assert!(yaml.contains("confidence: 0.85"));
}

#[test]
fn test_desktop_workflow_multiple_steps() {
    let workflow = DesktopWorkflow {
        name: "Complex Workflow".to_string(),
        steps: vec![
            DesktopStep::MouseMove { x: 100, y: 200 },
            DesktopStep::FindAndClick {
                image: "submit.png".to_string(),
                confidence: 0.9,
            },
            DesktopStep::Wait { seconds: 1 },
            DesktopStep::KeyboardType {
                text: "Hello World".to_string(),
            },
            DesktopStep::MouseClick { button: MouseButton::Left },
        ],
        variables: std::collections::HashMap::new(),
    };

    let yaml = serde_yaml::to_string(&workflow).unwrap();
    assert!(yaml.contains("mouse_move"));
    assert!(yaml.contains("find_and_click"));
    assert!(yaml.contains("wait"));
    assert!(yaml.contains("keyboard_type"));
    assert!(yaml.contains("mouse_click"));
}

#[test]
fn test_keyboard_key_serialization() {
    let keys = vec![Key::Control, Key::Shift, Key::Char('x')];
    let yaml = serde_yaml::to_string(&keys).unwrap();
    assert!(yaml.contains("control"));
    assert!(yaml.contains("shift"));
    assert!(yaml.contains("x"));
}

#[test]
fn test_region_serialization() {
    let region = Region {
        x: 10,
        y: 20,
        width: 800,
        height: 600,
    };

    let yaml = serde_yaml::to_string(&region).unwrap();
    assert!(yaml.contains("x: 10"));
    assert!(yaml.contains("y: 20"));
    assert!(yaml.contains("width: 800"));
    assert!(yaml.contains("height: 600"));
}

#[test]
fn test_workflow_variables_storage() {
    let mut variables = std::collections::HashMap::new();
    variables.insert("click_x".to_string(), "100".to_string());
    variables.insert("click_y".to_string(), "200".to_string());
    variables.insert("match_confidence".to_string(), "0.95".to_string());
    variables.insert("image_found".to_string(), "true".to_string());

    assert_eq!(variables.get("click_x"), Some(&"100".to_string()));
    assert_eq!(variables.get("click_y"), Some(&"200".to_string()));
    assert_eq!(variables.get("match_confidence"), Some(&"0.95".to_string()));
    assert_eq!(variables.get("image_found"), Some(&"true".to_string()));
}

#[test]
fn test_mouse_drag_serialization() {
    let step = DesktopStep::MouseDrag {
        from: Point { x: 10, y: 20 },
        to: Point { x: 110, y: 220 },
    };

    let yaml = serde_yaml::to_string(&step).unwrap();
    assert!(yaml.contains("mouse_drag"));
    assert!(yaml.contains("x: 10"));
    assert!(yaml.contains("y: 220"));
}

#[test]
fn test_mouse_button_serialization() {
    let yaml = serde_yaml::to_string(&MouseButton::Middle).unwrap();
    assert!(yaml.contains("middle"));
}
