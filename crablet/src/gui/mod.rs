//! GUI System Module
//!
//! Provides system integration features: tray icon, notifications, global shortcuts.

use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use tracing::info;
use chrono::Utc;

/// System tray icon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrayIcon {
    pub icon_path: String,
    pub tooltip: String,
    pub menu_items: Vec<TrayMenuItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrayMenuItem {
    pub id: String,
    pub label: String,
    pub enabled: bool,
    pub action: Option<String>,
}

/// Global shortcut
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalShortcut {
    pub id: String,
    pub keys: Vec<String>, // e.g., ["Ctrl", "Shift", "P"]
    pub action: String,
    pub enabled: bool,
}

/// System notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemNotification {
    pub id: Option<String>,
    pub title: String,
    pub body: String,
    pub icon: Option<String>,
    pub urgency: String, // "low", "normal", "critical"
    pub timeout: Option<u32>,
    pub created_at: i64,
}

impl SystemNotification {
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            id: Some(uuid::Uuid::new_v4().to_string()),
            title: title.into(),
            body: body.into(),
            icon: None,
            urgency: "normal".to_string(),
            timeout: None,
            created_at: Utc::now().timestamp_millis(),
        }
    }
}

/// Notification center for managing notifications
#[derive(Clone)]
pub struct NotificationCenter {
    notifications: Arc<RwLock<Vec<SystemNotification>>>,
    max_capacity: usize,
}

impl NotificationCenter {
    pub fn new(max_capacity: usize) -> Self {
        Self {
            notifications: Arc::new(RwLock::new(Vec::new())),
            max_capacity,
        }
    }

    /// Add notification
    pub async fn add(&self, notification: SystemNotification) -> String {
        let mut notifications = self.notifications.write().await;
        let id = notification.id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        
        notifications.push(notification);
        
        // Keep size under limit
        if notifications.len() > self.max_capacity {
            let drain_count = notifications.len() - self.max_capacity;
            notifications.drain(0..drain_count);
        }
        
        id
    }

    /// Get all notifications
    pub async fn get_all(&self) -> Vec<SystemNotification> {
        let notifications = self.notifications.read().await;
        notifications.clone()
    }

    /// Clear all notifications
    pub async fn clear(&self) {
        let mut notifications = self.notifications.write().await;
        notifications.clear();
    }

    /// Remove notification by id
    pub async fn remove(&self, id: &str) -> bool {
        let mut notifications = self.notifications.write().await;
        let len_before = notifications.len();
        notifications.retain(|n| n.id.as_ref() != Some(&id.to_string()));
        notifications.len() < len_before
    }
}

/// Shortcut manager
#[derive(Clone)]
pub struct ShortcutManager {
    shortcuts: Arc<RwLock<Vec<GlobalShortcut>>>,
}

impl ShortcutManager {
    pub fn new() -> Self {
        Self {
            shortcuts: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Register a shortcut
    pub async fn register(&self, shortcut: GlobalShortcut) -> Result<String, String> {
        let mut shortcuts = self.shortcuts.write().await;
        
        // Check for conflicts
        for existing in shortcuts.iter() {
            if existing.keys == shortcut.keys {
                return Err("Shortcut already registered".to_string());
            }
        }
        
        let id = format!("shortcut_{}", uuid::Uuid::new_v4());
        let mut new_shortcut = shortcut;
        new_shortcut.id = id.clone();
        shortcuts.push(new_shortcut);
        
        Ok(id)
    }

    /// Unregister a shortcut
    pub async fn unregister(&self, id: &str) -> Result<(), String> {
        let mut shortcuts = self.shortcuts.write().await;
        shortcuts.retain(|s| s.id != id);
        Ok(())
    }

    /// Get all shortcuts
    pub async fn get_all(&self) -> Vec<GlobalShortcut> {
        let shortcuts = self.shortcuts.read().await;
        shortcuts.clone()
    }

    /// Enable/disable shortcut
    pub async fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), String> {
        let mut shortcuts = self.shortcuts.write().await;
        for shortcut in shortcuts.iter_mut() {
            if shortcut.id == id {
                shortcut.enabled = enabled;
                return Ok(());
            }
        }
        Err("Shortcut not found".to_string())
    }
}

impl Default for ShortcutManager {
    fn default() -> Self {
        Self::new()
    }
}

/// GUI System - main coordinator
pub struct GuiSystem {
    notification_center: NotificationCenter,
    shortcut_manager: ShortcutManager,
    tray_icons: Arc<RwLock<Vec<TrayIcon>>>,
    enabled: bool,
}

impl GuiSystem {
    pub fn new() -> Self {
        Self {
            notification_center: NotificationCenter::new(100),
            shortcut_manager: ShortcutManager::new(),
            tray_icons: Arc::new(RwLock::new(Vec::new())),
            enabled: true,
        }
    }

    /// Get notification center
    pub fn notification_center(&self) -> &NotificationCenter {
        &self.notification_center
    }

    /// Get shortcut manager
    pub fn shortcut_manager(&self) -> &ShortcutManager {
        &self.shortcut_manager
    }

    /// Create tray icon
    pub async fn create_tray(&self, icon: TrayIcon) -> Result<String, String> {
        info!("Creating tray icon: {}", icon.tooltip);
        
        let mut icons = self.tray_icons.write().await;
        let id = format!("tray_{}", uuid::Uuid::new_v4());
        
        // TODO: 实现系统托盘创建
        // - Windows: 使用 windows-rs 创建系统托盘
        // - macOS: 使用 NSStatusItem
        // - Linux: 使用 libappindicator
        
        icons.push(icon);
        Ok(id)
    }

    /// Remove tray icon
    pub async fn remove_tray(&self, id: &str) -> Result<(), String> {
        let mut icons = self.tray_icons.write().await;
        icons.retain(|i| !i.menu_items.iter().any(|m| m.id == id));
        Ok(())
    }

    /// Send system notification
    pub async fn send_notification(&self, notification: SystemNotification) -> Result<String, String> {
        info!("Sending notification: {}", notification.title);
        
        let notification_center = self.notification_center.clone();
        let id = notification_center.add(notification.clone()).await;
        
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            let _ = Command::new("osascript")
                .arg("-e")
                .arg(format!(
                    r#"display notification "{}" with title "{}""#,
                    notification.body, notification.title
                ))
                .spawn();
        }
        
        #[cfg(target_os = "linux")]
        {
            use std::process::Command;
            let _ = Command::new("notify-send")
                .arg(&notification.title)
                .arg(&notification.body)
                .spawn();
        }
        
        #[cfg(target_os = "windows")]
        {
            // TODO: 使用 windows-rs 实现 toast 通知
        }
        
        Ok(id)
    }

    /// Register global shortcut
    pub async fn register_shortcut(&self, shortcut: GlobalShortcut) -> Result<String, String> {
        info!("Registering shortcut: {:?}", shortcut.keys);
        
        let shortcut_manager = self.shortcut_manager.clone();
        shortcut_manager.register(shortcut).await
    }

    /// Check if GUI is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable/disable GUI
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

impl Default for GuiSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gui_system_creation() {
        let gui = GuiSystem::new();
        assert!(gui.is_enabled());
    }

    #[tokio::test]
    async fn test_notification_center() {
        let center = NotificationCenter::new(10);
        
        let notification = SystemNotification::new("Test", "Hello");
        let id = center.add(notification).await;
        
        assert!(!id.is_empty());
        
        let all = center.get_all().await;
        assert_eq!(all.len(), 1);
    }

    #[tokio::test]
    async fn test_shortcut_manager() {
        let manager = ShortcutManager::new();
        
        let shortcut = GlobalShortcut {
            id: String::new(),
            keys: vec!["Ctrl".to_string(), "P".to_string()],
            action: "toggle_panel".to_string(),
            enabled: true,
        };
        
        let id = manager.register(shortcut).await;
        assert!(id.is_ok());
        
        let all = manager.get_all().await;
        assert_eq!(all.len(), 1);
    }
}
