//! 数据库迁移文件路径解析器。
//!
//! 解决 `env!("CARGO_MANIFEST_DIR")` 在打包后指向编译机源码路径的问题。
//! 按以下优先级搜索 migrations 目录：
//! 1. `CRABLET_MIGRATIONS_DIR` 环境变量（显式覆盖）
//! 2. `CRABLET_RESOURCE_DIR/migrations`（桌面端 .app bundle Resources）
//! 3. 可执行文件同级 `migrations/` 目录
//! 4. `CARGO_MANIFEST_DIR/migrations`（开发模式，`cargo run` 时有效）
//! 5. CWD `migrations/` 目录（最后回退）

use std::path::PathBuf;

/// 解析 migrations 目录路径，返回第一个存在的路径。
/// 如果所有候选路径都不存在，返回 `CARGO_MANIFEST_DIR/migrations`（开发模式默认值），
/// 让 `Migrator::new` 自行报错，保持错误链可追踪。
pub fn resolve_migrations_dir() -> PathBuf {
    // 1. 环境变量显式覆盖
    if let Ok(dir) = std::env::var("CRABLET_MIGRATIONS_DIR") {
        let path = PathBuf::from(&dir);
        if path.is_dir() {
            return path;
        }
    }

    // 2. 桌面端 .app bundle Resources/migrations
    if let Ok(resource_dir) = std::env::var("CRABLET_RESOURCE_DIR") {
        let path = PathBuf::from(&resource_dir).join("migrations");
        if path.is_dir() {
            return path;
        }
    }

    // 3. 可执行文件同级 migrations/
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let path = exe_dir.join("migrations");
            if path.is_dir() {
                return path;
            }
            // macOS .app bundle: Contents/MacOS/ → Contents/Resources/migrations
            if let Some(contents_dir) = exe_dir.parent() {
                let path = contents_dir.join("Resources").join("migrations");
                if path.is_dir() {
                    return path;
                }
            }
        }
    }

    // 4. 开发模式：CARGO_MANIFEST_DIR/migrations（cargo run 时有效）
    let dev_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    if dev_path.is_dir() {
        return dev_path;
    }

    // 5. CWD/migrations（最后回退）
    let cwd_path = PathBuf::from("migrations");
    if cwd_path.is_dir() {
        return cwd_path;
    }

    // 全部回退失败：返回开发模式路径，让 Migrator::new 报出清晰错误
    dev_path
}
