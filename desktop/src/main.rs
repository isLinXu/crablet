// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

/// 桌面服务优先端口；被占用时由操作系统分配空闲 loopback 端口。
const PREFERRED_SERVE_PORT: u16 = 18799;
const STARTUP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

struct DesktopEndpoint {
    port: u16,
    instance: String,
}

impl DesktopEndpoint {
    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

fn select_loopback_port(preferred: u16) -> Result<u16, String> {
    let preferred_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), preferred);
    if let Ok(listener) = TcpListener::bind(preferred_addr) {
        return listener
            .local_addr()
            .map(|a| a.port())
            .map_err(|e| e.to_string());
    }
    TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .and_then(|listener| listener.local_addr().map(|a| a.port()))
        .map_err(|e| format!("无法选择空闲 loopback 端口: {e}"))
}

fn new_desktop_endpoint() -> Result<DesktopEndpoint, String> {
    let port = select_loopback_port(PREFERRED_SERVE_PORT)?;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    Ok(DesktopEndpoint {
        port,
        instance: format!("{}-{nanos}", std::process::id()),
    })
}

fn endpoint_is_ours(endpoint: &DesktopEndpoint) -> bool {
    use std::io::{Read, Write};

    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), endpoint.port);
    let Ok(mut stream) =
        std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(400))
    else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(400)));
    let request = format!(
        "GET /health HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
        endpoint.port
    );
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    let mut response = String::new();
    if stream.read_to_string(&mut response).is_err() || !response.starts_with("HTTP/1.1 200") {
        return false;
    }
    let Some(body) = response.split("\r\n\r\n").nth(1) else {
        return false;
    };
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("desktop_instance")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
        })
        .as_deref()
        == Some(endpoint.instance.as_str())
}

#[tauri::command]
async fn wait_for_sidecar(endpoint: tauri::State<'_, DesktopEndpoint>) -> Result<String, String> {
    let deadline = std::time::Instant::now() + STARTUP_TIMEOUT;
    while std::time::Instant::now() < deadline {
        if endpoint_is_ours(&endpoint) {
            return Ok(endpoint.url());
        }
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
    Err(format!(
        "sidecar 未在 {} 秒内通过身份健康检查",
        STARTUP_TIMEOUT.as_secs()
    ))
}

/// 保存 sidecar 子进程句柄，退出时终止并释放所有权。
struct SidecarState(Mutex<Option<CommandChild>>);

/// 统一退出状态：第一个退出入口负责清理，后续事件只允许自然退出。
#[derive(Default)]
struct ExitState(AtomicBool);

impl ExitState {
    fn begin(&self) -> bool {
        self.0
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    fn is_exiting(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// API Key 内存缓存，避免每次都访问 keyring（macOS Keychain 可能耗时 ~46s）。
/// 使用 Mutex 保证线程安全，替代不安全的 std::env::set_var（Rust 1.80+ 多线程下不安全）。
struct ApiKeyState(Mutex<Option<String>>);

/// 前端调用：保存 API Key 到系统 keyring + 内存缓存。
/// 保存成功后自动重启 sidecar，使新 Key 立即生效（无需用户手动重启应用）。
fn normalize_api_key(key: String) -> Result<String, String> {
    let key = key.trim().to_string();
    if key.is_empty() {
        return Err("API Key 不能为空".to_string());
    }
    Ok(key)
}

#[tauri::command]
fn save_api_key(app: tauri::AppHandle, key: String) -> Result<(), String> {
    let key = normalize_api_key(key)?;

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
fn server_url(endpoint: tauri::State<'_, DesktopEndpoint>) -> String {
    endpoint.url()
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

fn packaged_resource_dir(exe_path: &Path) -> Option<PathBuf> {
    let resource_dir = exe_path.parent()?.parent()?.join("Resources");
    resource_dir.exists().then_some(resource_dir)
}

fn sidecar_search_paths(exe_path: &Path, triple: &str) -> Vec<PathBuf> {
    let Some(exe_dir) = exe_path.parent() else {
        return vec![];
    };
    let with_triple = format!("crablet-{triple}");
    let resources = exe_dir.parent().map(|p| p.join("Resources"));
    vec![
        exe_dir.join("binaries").join(&with_triple),
        exe_dir.join("binaries").join("crablet"),
        exe_dir.join(&with_triple),
        resources
            .as_ref()
            .map(|p| p.join("binaries").join(&with_triple))
            .unwrap_or_default(),
        resources
            .as_ref()
            .map(|p| p.join("binaries").join("crablet"))
            .unwrap_or_default(),
        resources.map(|p| p.join(&with_triple)).unwrap_or_default(),
    ]
}

/// 收集环境变量（keyring API Key + CRABLET_RESOURCE_DIR）。
/// keyring 访问放在这里（而非 Config::load 中），因为桌面端启动时
/// 可以在 sidecar 启动前异步完成，不阻塞 UI。
fn desktop_data_root_from(home: &Path) -> PathBuf {
    home.join("Library/Application Support/com.crablet.desktop")
}

fn desktop_data_root() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or_else(|| "HOME is not set".to_string())?;
    Ok(desktop_data_root_from(Path::new(&home)))
}

fn collect_envs(api_key_cache: &Mutex<Option<String>>) -> Vec<(String, String)> {
    let mut envs: Vec<(String, String)> = Vec::new();

    if let Ok(data_root) = desktop_data_root() {
        for subdir in ["db", "config", "skills", "uploads", "logs"] {
            if let Err(error) = std::fs::create_dir_all(data_root.join(subdir)) {
                eprintln!("[crablet-desktop] 创建数据目录失败: {error}");
            }
        }
        envs.push((
            "CRABLET_DATA_DIR".to_string(),
            data_root.display().to_string(),
        ));
        let env_file = data_root.join("config").join(".env");
        if !env_file.exists() {
            if let Err(error) = std::fs::write(&env_file, "") {
                eprintln!("[crablet-desktop] 创建配置文件失败: {error}");
            }
        }
        envs.push((
            "CRABLET_ENV_FILE".to_string(),
            env_file.display().to_string(),
        ));
        if let Some(cache_root) = dirs::cache_dir() {
            let cache_dir = cache_root.join("com.crablet.desktop");
            let _ = std::fs::create_dir_all(&cache_dir);
            envs.push((
                "CRABLET_CACHE_DIR".to_string(),
                cache_dir.display().to_string(),
            ));
        }
    }

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
        if let Some(resource_dir) = packaged_resource_dir(&exe_path) {
            envs.push((
                "CRABLET_RESOURCE_DIR".to_string(),
                resource_dir.display().to_string(),
            ));
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

/// 启动 crablet sidecar，注入运行时环境变量。
/// 对于本地桌面应用：
/// - CRABLET_ALLOW_ANY_ORIGIN=true：允许所有跨域请求（无公网暴露风险）
/// - CRABLET_AUTH_MODE=off：禁用 API token 认证（桌面端通过 keyring 管理凭证，无需额外 token）
fn spawn_sidecar_with_cors(app: &tauri::AppHandle) -> Result<CommandChild, String> {
    // 通过子进程 env 注入（不使用 std::env::set_var，Rust 1.80+ 多线程下不安全）
    spawn_sidecar_with_env(
        app,
        &[
            ("CRABLET_ALLOW_ANY_ORIGIN".to_string(), "false".to_string()),
            ("CRABLET_AUTH_MODE".to_string(), "off".to_string()),
        ],
    )
}

/// 带额外环境变量启动 sidecar（用于注入 CORS 等运行时配置）。
fn spawn_sidecar_with_env(
    app: &tauri::AppHandle,
    extra_envs: &[(String, String)],
) -> Result<CommandChild, String> {
    let api_key_state = app.state::<ApiKeyState>();
    let endpoint = app.state::<DesktopEndpoint>();
    let mut envs = collect_envs(&api_key_state.0);
    envs.extend_from_slice(extra_envs);
    envs.push((
        "CRABLET_DESKTOP_INSTANCE".to_string(),
        endpoint.instance.clone(),
    ));
    let port_string = endpoint.port.to_string();
    let port_args = [
        "serve-web",
        "--host",
        "127.0.0.1",
        "--port",
        port_string.as_str(),
    ];

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
                    eprintln!(
                        "[crablet-desktop] Tauri sidecar API 启动失败: {}，尝试手动查找...",
                        e
                    );
                }
            }
        }
        Err(e) => {
            eprintln!(
                "[crablet-desktop] Tauri sidecar API 定位失败: {}，尝试手动查找...",
                e
            );
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
    port_args: &[&str; 5],
) -> Result<CommandChild, String> {
    let triple = target_triple();

    // 搜索可能的 sidecar 二进制路径（按优先级排序）
    let search_paths = std::env::current_exe()
        .map(|path| sidecar_search_paths(&path, &triple))
        .unwrap_or_default();

    eprintln!("[crablet-desktop] 搜索 sidecar 二进制路径:");
    for path in &search_paths {
        eprintln!("   检查: {}", path.display());
        if path.exists() && path.is_file() {
            eprintln!("   ✅ 找到: {}", path.display());
            let envs_owned: Vec<(String, String)> = envs.to_vec();
            let cmd = app
                .shell()
                .command(path.to_string_lossy().as_ref())
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
        search_paths
            .iter()
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

/// 获取 sidecar 所有权并终止。`take` 保证并发退出入口最多清理一次。
fn stop_sidecar(app: &tauri::AppHandle) {
    let child = app
        .state::<SidecarState>()
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take();
    if let Some(child) = child {
        match child.kill() {
            Ok(()) => eprintln!("[crablet-desktop] sidecar 已终止"),
            Err(error) => eprintln!("[crablet-desktop] sidecar 终止失败: {error}"),
        }
    }
}

/// 汇聚 Cmd+Q、窗口关闭和其他退出来源；返回是否由本次调用执行了清理。
fn begin_exit(app: &tauri::AppHandle) -> bool {
    let exit = app.state::<ExitState>();
    if !exit.begin() {
        return false;
    }

    stop_sidecar(app);
    true
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .manage(SidecarState(Mutex::new(None)))
        .manage(ExitState::default())
        .manage(ApiKeyState(Mutex::new(None)))
        .manage(new_desktop_endpoint().expect("无法为桌面 sidecar 选择安全端口"))
        .invoke_handler(tauri::generate_handler![
            save_api_key,
            has_api_key,
            has_api_key_async,
            server_url,
            wait_for_sidecar,
            start_sidecar,
            is_sidecar_running
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            // 异步启动 sidecar：不阻塞窗口加载。
            // keyring 读取由 spawn_sidecar_with_cors -> collect_envs 在该阻塞任务中完成，
            // 避免 macOS Keychain 首次访问卡住 Tauri setup/UI 线程。

            // 前端 BackendStatus 组件会轮询后端 health 端点，
            // 后端就绪后自动移除覆盖层。
            let handle_clone = handle.clone();
            tauri::async_runtime::spawn_blocking(move || {
                // 短暂延迟，让窗口先完成渲染
                std::thread::sleep(std::time::Duration::from_millis(500));
                match spawn_sidecar_with_cors(&handle_clone) {
                    Ok(child) => {
                        // Cmd+Q 可能发生在后台 spawn 完成前；退出开始后不得遗留晚到的子进程。
                        if handle_clone.state::<ExitState>().is_exiting() {
                            let _ = child.kill();
                            eprintln!("[crablet-desktop] 退出期间终止晚到的 sidecar");
                        } else {
                            let state = handle_clone.state::<SidecarState>();
                            *state.0.lock().unwrap() = Some(child);
                            eprintln!("[crablet-desktop] ✅ sidecar 异步启动成功");
                        }
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
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                // 当前产品不采用“关窗隐藏”；splash 也是同一个 WebView 窗口，关闭即退出。
                begin_exit(window.app_handle());
            }
        })
        .build(tauri::generate_context!())
        .expect("构建 Crablet 桌面应用时出错")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                // macOS Cmd+Q 走应用级事件，不保证先产生窗口 CloseRequested。
                // 不调用 prevent_exit，让 Tauri 在幂等清理后完成事件循环退出。
                begin_exit(app);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::{
        desktop_data_root_from, endpoint_is_ours, normalize_api_key, packaged_resource_dir,
        select_loopback_port, sidecar_search_paths, DesktopEndpoint, ExitState,
        PREFERRED_SERVE_PORT,
    };
    use std::fs;
    use std::io::{Read, Write};
    use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
    use std::path::Path;

    #[test]
    fn desktop_data_root_is_per_user_and_outside_bundle() {
        let root = desktop_data_root_from(Path::new("/Users/alice"));
        assert_eq!(
            root,
            Path::new("/Users/alice/Library/Application Support/com.crablet.desktop")
        );
        assert!(!root.starts_with("/Applications/Crablet.app"));
        assert!(!root.starts_with("/Volumes/Crablet"));
    }

    #[test]
    fn exit_state_allows_cleanup_exactly_once() {
        let state = ExitState::default();
        assert!(!state.is_exiting());
        assert!(state.begin());
        assert!(state.is_exiting());
        assert!(!state.begin());
    }

    #[test]
    fn exit_state_is_idempotent_across_threads() {
        let state = std::sync::Arc::new(ExitState::default());
        let winners = (0..8)
            .map(|_| {
                let state = state.clone();
                std::thread::spawn(move || state.begin() as usize)
            })
            .map(|thread| thread.join().unwrap())
            .sum::<usize>();
        assert_eq!(winners, 1);
    }

    #[test]
    fn api_key_is_trimmed_before_persisting() {
        assert_eq!(
            normalize_api_key("  sk-test  ".to_string()).unwrap(),
            "sk-test"
        );
    }

    #[test]
    fn empty_api_key_is_rejected() {
        assert!(normalize_api_key("  \n\t ".to_string()).is_err());
    }

    #[test]
    fn desktop_prefers_the_documented_port() {
        assert_eq!(PREFERRED_SERVE_PORT, 18799);
    }

    #[test]
    fn occupied_preferred_port_falls_back_to_an_available_loopback_port() {
        let occupied = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
        let occupied_port = occupied.local_addr().unwrap().port();
        let selected = select_loopback_port(occupied_port).unwrap();
        assert_ne!(selected, occupied_port);
        assert!(TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, selected)).is_ok());
    }

    #[test]
    fn selected_port_is_loopback_only() {
        let selected = select_loopback_port(0).unwrap();
        let wildcard = TcpListener::bind((Ipv4Addr::UNSPECIFIED, selected));
        assert!(
            wildcard.is_ok(),
            "selection must not leave a wildcard listener behind"
        );
    }

    #[test]
    fn health_identity_rejects_a_service_that_only_returns_ok() {
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 512];
            let _ = stream.read(&mut request);
            let body = r#"{"status":"ok","desktop_instance":"attacker"}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });
        let endpoint = DesktopEndpoint {
            port,
            instance: "expected".into(),
        };
        assert!(!endpoint_is_ours(&endpoint));
        server.join().unwrap();
    }

    #[test]
    fn health_identity_accepts_the_spawned_instance() {
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 512];
            let _ = stream.read(&mut request);
            let body = r#"{"status":"ok","desktop_instance":"expected"}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });
        let endpoint = DesktopEndpoint {
            port,
            instance: "expected".into(),
        };
        assert!(endpoint_is_ours(&endpoint));
        server.join().unwrap();
    }

    #[test]
    fn packaged_resource_dir_resolves_from_bundle_executable() {
        let root = Path::new(
            &std::env::var("CRABLET_TEST_TMPDIR").expect("CRABLET_TEST_TMPDIR must be set"),
        )
        .join(format!("resource-path-{}", std::process::id()));
        let resources = root.join("Crablet.app/Contents/Resources");
        fs::create_dir_all(&resources).unwrap();
        let executable = root.join("Crablet.app/Contents/MacOS/crablet-desktop");

        assert_eq!(packaged_resource_dir(&executable), Some(resources.clone()));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_packaged_resource_dir_is_not_injected() {
        assert_eq!(
            packaged_resource_dir(Path::new("/nonexistent/Crablet.app/Contents/MacOS/app")),
            None
        );
    }

    #[test]
    fn sidecar_paths_cover_tauri_and_packaged_locations_in_priority_order() {
        let executable = Path::new("/Applications/Crablet.app/Contents/MacOS/crablet-desktop");
        let paths = sidecar_search_paths(executable, "aarch64-apple-darwin");
        assert_eq!(paths.len(), 6);
        assert_eq!(
            paths[0],
            Path::new(
                "/Applications/Crablet.app/Contents/MacOS/binaries/crablet-aarch64-apple-darwin"
            )
        );
        assert_eq!(
            paths[3],
            Path::new("/Applications/Crablet.app/Contents/Resources/binaries/crablet-aarch64-apple-darwin")
        );
        assert_eq!(
            paths[5],
            Path::new("/Applications/Crablet.app/Contents/Resources/crablet-aarch64-apple-darwin")
        );
    }
}
