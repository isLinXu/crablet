// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;
use tauri::{Manager, Emitter};
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::{CommandChild, CommandEvent};

/// 固定的本地服务端口。
const SERVE_PORT: u16 = 18799;

/// 保存 sidecar 子进程句柄，退出时优雅 kill。
struct SidecarState(Mutex<Option<CommandChild>>);

/// API Key 内存缓存，避免每次都访问 keyring（macOS Keychain 可能耗时 ~46s）。
/// 使用 Mutex 保证线程安全，替代不安全的 std::env::set_var（Rust 1.80+ 多线程下不安全）。
struct ApiKeyState(Mutex<Option<String>>);

/// 前端调用：保存 API Key 到系统 keyring + 内存缓存。
/// 保存成功后自动重启 sidecar，使新 Key 立即生效（无需用户手动重启应用）。
#[tauri::command]
fn save_api_key(app: tauri::AppHandle, key: String) -> Result<(), String> {
    // 1. 写入系统 keyring（持久化）
    let entry = keyring::Entry::new("crablet", "openai_api_key").map_err(|e| e.to_string())?;
    entry.set_password(&key).map_err(|e| e.to_string())?;

    // 2. 更新内存缓存（线程安全，替代 std::env::set_var）
    let state = app.state::<ApiKeyState>();
    *state.0.lock().unwrap() = Some(key);

    // 3. 重启 sidecar 让新 Key 生效（新进程会从 keyring 读取）
    restart_sidecar(&app);
    Ok(())
}

/// 前端调用：检查是否已配置 API Key。
/// 优先检查内存缓存（快速路径，零延迟），仅在缓存未命中时才访问 keyring（慢速路径）。
#[tauri::command]
fn has_api_key(app: tauri::AppHandle) -> bool {
    // 1. 快速路径：检查内存缓存（零延迟）
    let state = app.state::<ApiKeyState>();
    if state.0.lock().unwrap().is_some() {
        return true;
    }
    // 2. 慢速路径：检查 keyring（macOS keychain 可能耗时 ~46s）
    //    如果 keyring 中有值，顺便更新内存缓存，后续调用走快速路径。
    if let Ok(entry) = keyring::Entry::new("crablet", "openai_api_key") {
        if let Ok(pwd) = entry.get_password() {
            *state.0.lock().unwrap() = Some(pwd);
            return true;
        }
    }
    false
}

/// 前端调用：异步检查是否已配置 API Key。
/// 与 has_api_key 功能相同，但使用异步上下文避免阻塞 UI 线程。
#[tauri::command]
async fn has_api_key_async(app: tauri::AppHandle) -> bool {
    has_api_key(app)
}

/// 重启 sidecar：优雅 kill 旧子进程，再重新 spawn 一个新的。
/// 用于 API Key 更新后让后端立即读取新配置，避免要求用户重启整个应用。
fn restart_sidecar(app: &tauri::AppHandle) {
    let state = app.state::<SidecarState>();
    // 先 kill 旧进程。
    if let Some(child) = state.0.lock().unwrap().take() {
        let _ = child.kill();
    }
    // 重新 spawn（带 CORS 环境变量）。
    match spawn_sidecar_with_cors(app) {
        Ok(child) => {
            *state.0.lock().unwrap() = Some(child);
            eprintln!("[crablet-desktop] ✅ sidecar 已重启（API Key 更新生效）");
        }
        Err(e) => {
            eprintln!("[crablet-desktop] sidecar 重启失败: {}", e);
            let _ = app.emit("sidecar-error", e);
        }
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
fn collect_envs(api_key_cache: &Mutex<Option<String>>) -> Vec<(String, String)> {
    let mut envs: Vec<(String, String)> = Vec::new();

    // 优先从内存缓存读取 API Key（避免重复访问 keyring）
    let cached = api_key_cache.lock().unwrap();
    if let Some(key) = cached.as_ref() {
        envs.push(("OPENAI_API_KEY".to_string(), key.clone()));
        envs.push(("DASHSCOPE_API_KEY".to_string(), key.clone()));
    } else {
        drop(cached); // 释放锁后再访问 keyring
        // 缓存未命中时，从 keyring 读取并更新缓存
        if let Ok(entry) = keyring::Entry::new("crablet", "openai_api_key") {
            if let Ok(pwd) = entry.get_password() {
                envs.push(("OPENAI_API_KEY".to_string(), pwd.clone()));
                envs.push(("DASHSCOPE_API_KEY".to_string(), pwd.clone()));
                *api_key_cache.lock().unwrap() = Some(pwd);
            }
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

/// 启动 crablet sidecar，注入 CRABLET_ALLOW_ANY_ORIGIN=true 环境变量。
/// 对于本地桌面应用，允许所有跨域请求是安全的（无安全风险）。
fn spawn_sidecar_with_cors(app: &tauri::AppHandle) -> Result<CommandChild, String> {
    // 通过子进程 env 注入 CORS 放开标志（不使用 std::env::set_var，Rust 1.80+ 多线程下不安全）
    // 本地桌面应用无公网暴露风险，CORS 限制可安全放开
    spawn_sidecar_with_env(app, &[("CRABLET_ALLOW_ANY_ORIGIN".to_string(), "true".to_string())])
}

/// 带额外环境变量启动 sidecar（用于注入 CORS 等运行时配置）。
fn spawn_sidecar_with_env(
    app: &tauri::AppHandle,
    extra_envs: &[(String, String)],
) -> Result<CommandChild, String> {
    let api_key_state = app.state::<ApiKeyState>();
    let mut envs = collect_envs(&api_key_state.0);
    envs.extend_from_slice(extra_envs);
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

/// 前端调用：手动启动 sidecar（用于首次配置后触发）。
#[tauri::command]
async fn start_sidecar(app: tauri::AppHandle) -> Result<String, String> {
    let state = app.state::<SidecarState>();
    // 如果已有 sidecar 在运行，直接返回
    if state.0.lock().unwrap().is_some() {
        return Ok("sidecar already running".to_string());
    }
    drop(state);

    match spawn_sidecar_with_cors(&app) {
        Ok(child) => {
            let state = app.state::<SidecarState>();
            *state.0.lock().unwrap() = Some(child);
            Ok("sidecar started".to_string())
        }
        Err(e) => Err(e),
    }
}

/// 前端调用：检查 sidecar 是否在运行。
#[tauri::command]
fn is_sidecar_running(app: tauri::AppHandle) -> bool {
    let state = app.state::<SidecarState>();
    let is_running = state.0.lock().unwrap().is_some();
    is_running
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .manage(SidecarState(Mutex::new(None)))
        .manage(ApiKeyState(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![
            save_api_key,
            has_api_key,
            has_api_key_async,
            server_url,
            start_sidecar,
            is_sidecar_running
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            // 预加载 API Key 到内存缓存，避免后续 keyring 访问阻塞 UI。
            // macOS Keychain 首次访问可耗时 ~46s，提前在 setup 中完成。
            let api_key_state = app.state::<ApiKeyState>();
            if api_key_state.0.lock().unwrap().is_none() {
                if let Ok(entry) = keyring::Entry::new("crablet", "openai_api_key") {
                    if let Ok(pwd) = entry.get_password() {
                        *api_key_state.0.lock().unwrap() = Some(pwd);
                        eprintln!("[crablet-desktop] ✅ API Key 已从 keyring 预加载到内存缓存");
                    }
                }
            }

            // 异步启动 sidecar：不阻塞窗口加载。
            // 前端 BackendStatus 组件会轮询后端 health 端点，
            // 后端就绪后自动移除覆盖层。
            let handle_clone = handle.clone();
            tauri::async_runtime::spawn_blocking(move || {
                // 短暂延迟，让窗口先完成渲染
                std::thread::sleep(std::time::Duration::from_millis(500));
                match spawn_sidecar_with_cors(&handle_clone) {
                    Ok(child) => {
                        let state = handle_clone.state::<SidecarState>();
                        *state.0.lock().unwrap() = Some(child);
                        eprintln!("[crablet-desktop] ✅ sidecar 异步启动成功");
                    }
                    Err(e) => {
                        eprintln!("[crablet-desktop] sidecar 异步启动失败: {}", e);
                        let _ = handle_clone.emit("sidecar-error", e);
                    }
                }
            });

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
