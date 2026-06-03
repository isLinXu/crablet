// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;
use tauri::{Manager, WebviewWindow, Emitter};
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::{CommandChild, CommandEvent};

/// 固定的本地服务端口。
const SERVE_PORT: u16 = 18799;

/// 保存 sidecar 子进程句柄，退出时优雅 kill。
struct SidecarState(Mutex<Option<CommandChild>>);

/// 前端调用：保存 API Key 到系统 keyring（与 crablet 后端约定 service="crablet", user="openai_api_key"），
/// 保存成功后自动重启 sidecar，使新 Key 立即生效（无需用户手动重启应用）。
#[tauri::command]
fn save_api_key(app: tauri::AppHandle, key: String) -> Result<(), String> {
    let entry = keyring::Entry::new("crablet", "openai_api_key").map_err(|e| e.to_string())?;
    entry.set_password(&key).map_err(|e| e.to_string())?;

    // 同时写入当前进程环境变量，确保后续 collect_envs() 与 has_api_key() 立即可见。
    std::env::set_var("OPENAI_API_KEY", &key);
    std::env::set_var("DASHSCOPE_API_KEY", &key);

    // 重启 sidecar：kill 旧进程 → 重新 spawn（新进程会读到刚保存的 Key）。
    restart_sidecar(&app);
    Ok(())
}

/// 重启 sidecar：优雅 kill 旧子进程，再重新 spawn 一个新的。
/// 用于 API Key 更新后让后端立即读取新配置，避免要求用户重启整个应用。
fn restart_sidecar(app: &tauri::AppHandle) {
    let state = app.state::<SidecarState>();
    // 先 kill 旧进程。
    if let Some(child) = state.0.lock().unwrap().take() {
        let _ = child.kill();
    }
    // 重新 spawn。
    match spawn_sidecar(app) {
        Ok(child) => {
            *state.0.lock().unwrap() = Some(child);
            eprintln!("[crablet-desktop] ✅ sidecar 已重启（API Key 更新生效）");
            // 重新触发导航：等新后端就绪后自动跳转到 Web UI，无需用户手动重启。
            if let Some(window) = app.get_webview_window("main") {
                tauri::async_runtime::spawn(wait_and_navigate(window));
            }
        }
        Err(e) => {
            eprintln!("[crablet-desktop] sidecar 重启失败: {}", e);
            let _ = app.emit("sidecar-error", e);
        }
    }
}

/// 前端调用：检查是否已配置 API Key。
/// 优先检查环境变量（快速），仅在环境变量未设置时才访问 keyring（避免 macOS keychain 延迟）。
#[tauri::command]
fn has_api_key() -> bool {
    // 1. 快速路径：检查环境变量（零延迟）
    if std::env::var("OPENAI_API_KEY").is_ok() || std::env::var("DASHSCOPE_API_KEY").is_ok() {
        return true;
    }
    // 2. 慢速路径：检查 keyring（macOS keychain 可能耗时 ~46s）
    match keyring::Entry::new("crablet", "openai_api_key") {
        Ok(entry) => entry.get_password().is_ok(),
        Err(_) => false,
    }
}

/// 前端调用：拿到后端服务地址，由 splash 页跳转。
#[tauri::command]
fn server_url() -> String {
    format!("http://127.0.0.1:{}", SERVE_PORT)
}

/// 构建 target triple 后缀（如 "aarch64-apple-darwin"）。
fn target_triple() -> String {
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;
    let arch_str = match arch {
        "aarch64" => "aarch64",
        "x86_64" => "x86_64",
        other => other,
    };
    let os_str = match os {
        "macos" => "apple-darwin",
        "windows" => "pc-windows-msvc",
        "linux" => "unknown-linux-gnu",
        other => other,
    };
    format!("{}-{}", arch_str, os_str)
}

/// 收集环境变量（keyring API Key + CRABLET_RESOURCE_DIR）。
/// keyring 访问放在这里（而非 Config::load 中），因为桌面端启动时
/// 可以在 sidecar 启动前异步完成，不阻塞 UI。
fn collect_envs() -> Vec<(String, String)> {
    let mut envs: Vec<(String, String)> = Vec::new();

    // 把已配置的 keyring API Key 注入为环境变量，确保 sidecar 能读到（即使其 keyring 访问受限）。
    if let Ok(entry) = keyring::Entry::new("crablet", "openai_api_key") {
        if let Ok(pwd) = entry.get_password() {
            envs.push(("OPENAI_API_KEY".to_string(), pwd.clone()));
            envs.push(("DASHSCOPE_API_KEY".to_string(), pwd));
        }
    }

    // 注入 CRABLET_RESOURCE_DIR，让 sidecar 能解析 $APP_RESOURCE 变量。
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            if let Some(contents_dir) = exe_dir.parent() {
                let resource_dir = contents_dir.join("Resources");
                if resource_dir.exists() {
                    envs.push(("CRABLET_RESOURCE_DIR".to_string(), resource_dir.display().to_string()));
                }
            }
        }
    }

    envs
}

/// 后台读取 sidecar 输出，转发到前端日志。
fn start_log_reader(app: tauri::AppHandle, mut rx: tauri::async_runtime::Receiver<CommandEvent>) {
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(line) => {
                    let text = String::from_utf8_lossy(&line).to_string();
                    let _ = app.emit("sidecar-log", text);
                }
                CommandEvent::Stderr(line) => {
                    let text = String::from_utf8_lossy(&line).to_string();
                    let _ = app.emit("sidecar-log", text);
                }
                CommandEvent::Terminated(payload) => {
                    let _ = app.emit(
                        "sidecar-exit",
                        format!("crablet 进程已退出: {:?}", payload.code),
                    );
                    break;
                }
                _ => {}
            }
        }
    });
}

/// 启动 crablet sidecar，执行 `serve-web --port <PORT>`。
/// 统一入口：先尝试 Tauri sidecar API，失败后自动回退到手动路径搜索。
fn spawn_sidecar(app: &tauri::AppHandle) -> Result<CommandChild, String> {
    let envs = collect_envs();
    let port_args = ["serve-web", "--port", &SERVE_PORT.to_string()];

    // 尝试 1: Tauri sidecar API（会自动追加 target triple 后缀查找二进制）
    match app.shell().sidecar("crablet") {
        Ok(cmd) => {
            eprintln!("[crablet-desktop] 尝试 Tauri sidecar API 启动...");
            let cmd = cmd.args(port_args).envs(envs.clone());
            match cmd.spawn() {
                Ok((rx, child)) => {
                    start_log_reader(app.clone(), rx);
                    eprintln!("[crablet-desktop] ✅ sidecar 启动成功 (Tauri API)");
                    return Ok(child);
                }
                Err(e) => {
                    eprintln!("[crablet-desktop] Tauri sidecar API 启动失败: {}，尝试手动查找...", e);
                }
            }
        }
        Err(e) => {
            eprintln!("[crablet-desktop] Tauri sidecar API 定位失败: {}，尝试手动查找...", e);
        }
    }

    // 尝试 2: 手动查找 sidecar 二进制
    spawn_sidecar_manual(app, &envs, &port_args)
}

/// 手动查找并启动 sidecar 二进制（当 Tauri sidecar API 失败时的回退方案）。
/// 搜索所有可能的路径，按优先级排序。
fn spawn_sidecar_manual(
    app: &tauri::AppHandle,
    envs: &[(String, String)],
    port_args: &[&str; 3],
) -> Result<CommandChild, String> {
    let triple = target_triple();
    let binary_name_with_triple = format!("crablet-{}", triple);
    let binary_name_without_triple = "crablet";

    // 搜索可能的 sidecar 二进制路径（按优先级排序）
    let search_paths = if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            vec![
                // macOS .app bundle: Contents/MacOS/binaries/ (Tauri 标准位置)
                exe_dir.join("binaries").join(&binary_name_with_triple),
                exe_dir.join("binaries").join(binary_name_without_triple),
                // macOS .app bundle: Contents/MacOS/ (Tauri 有时放在这里)
                exe_dir.join(&binary_name_with_triple),
                // macOS .app bundle: Contents/Resources/binaries/
                exe_dir.parent()
                    .map(|p| p.join("Resources").join("binaries").join(&binary_name_with_triple))
                    .unwrap_or_default(),
                exe_dir.parent()
                    .map(|p| p.join("Resources").join("binaries").join(binary_name_without_triple))
                    .unwrap_or_default(),
                // macOS .app bundle: Contents/Resources/ (备用位置)
                exe_dir.parent()
                    .map(|p| p.join("Resources").join(&binary_name_with_triple))
                    .unwrap_or_default(),
            ]
        } else {
            vec![]
        }
    } else {
        vec![]
    };

    eprintln!("[crablet-desktop] 搜索 sidecar 二进制路径:");
    for path in &search_paths {
        eprintln!("   检查: {}", path.display());
        if path.exists() && path.is_file() {
            eprintln!("   ✅ 找到: {}", path.display());
            let envs_owned: Vec<(String, String)> = envs.to_vec();
            let cmd = app.shell().command(path.to_string_lossy().as_ref())
                .args(port_args)
                .envs(envs_owned);

            match cmd.spawn() {
                Ok((rx, child)) => {
                    start_log_reader(app.clone(), rx);
                    eprintln!("[crablet-desktop] ✅ sidecar 启动成功 (手动路径)");
                    return Ok(child);
                }
                Err(e) => return Err(format!("手动启动 crablet 失败 ({}): {}", path.display(), e)),
            }
        }
    }

    Err(format!(
        "找不到 crablet sidecar 二进制。已搜索路径:\n{}",
        search_paths.iter()
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| format!("  - {}", p.display()))
            .collect::<Vec<_>>()
            .join("\n")
    ))
}

/// 轮询本地服务，就绪后让窗口导航过去。
/// 最多等待 ~120 秒（首次启动 keyring 访问 + LLM 初始化可能较慢）。
async fn wait_and_navigate(window: WebviewWindow) {
    let url = format!("http://127.0.0.1:{}/", SERVE_PORT);

    for i in 0..600 {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        if std::net::TcpStream::connect(("127.0.0.1", SERVE_PORT)).is_ok() {
            // 端口已监听，再给后端一点点时间完成路由挂载。
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            eprintln!("[crablet-desktop] 端口 {} 已就绪，正在导航… (第 {} 次探测)", SERVE_PORT, i + 1);
            match url.parse::<tauri::Url>() {
                Ok(parsed) => {
                    match window.navigate(parsed) {
                        Ok(()) => eprintln!("[crablet-desktop] 导航成功"),
                        Err(e) => eprintln!("[crablet-desktop] 导航失败: {}", e),
                    }
                }
                Err(e) => eprintln!("[crablet-desktop] URL 解析失败: {}", e),
            }
            return;
        }
    }

    eprintln!("[crablet-desktop] 后端服务启动超时");
    let _ = window.emit(
        "sidecar-error",
        "后端服务启动超时，请检查日志或重启应用。".to_string(),
    );
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .manage(SidecarState(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![
            save_api_key,
            has_api_key,
            server_url
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            // 启动 sidecar 服务。
            match spawn_sidecar(&handle) {
                Ok(child) => {
                    let state = app.state::<SidecarState>();
                    *state.0.lock().unwrap() = Some(child);
                }
                Err(e) => {
                    eprintln!("{}", e);
                    let _ = handle.emit("sidecar-error", e);
                }
            }

            // 主窗口在 splash.html，等服务就绪后自动导航。
            if let Some(window) = app.get_webview_window("main") {
                tauri::async_runtime::spawn(wait_and_navigate(window));
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // 窗口关闭时 kill sidecar，避免残留进程。
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let state = window.app_handle().state::<SidecarState>();
                if let Some(child) = state.0.lock().unwrap().take() {
                    let _ = child.kill();
                };
            }
        })
        .run(tauri::generate_context!())
        .expect("运行 Crablet 桌面应用时出错");
}
