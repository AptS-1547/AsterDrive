//! 运行时子模块：`panic`。

use std::fs::OpenOptions;
use std::io::Write;
use std::panic;
use std::sync::{Mutex, OnceLock};

static CRASH_LOG: OnceLock<Mutex<std::fs::File>> = OnceLock::new();

/// 安装自定义 panic hook。
///
/// crash.log 文件句柄在首次 panic 时惰性打开后复用（`OnceLock`），
/// 写入用 `try_lock()` 避免 panic storm 下的递归死锁或无限阻塞。
pub fn install_panic_hook() {
    panic::set_hook(Box::new(|info| {
        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("<unnamed>");

        let location = info
            .location()
            .map(|loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column()))
            .unwrap_or_else(|| "<unknown>".to_string());

        let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "<no message>".to_string()
        };

        let platform = std::env::consts::OS;
        let version = env!("CARGO_PKG_VERSION");
        let repository = env!("CARGO_PKG_REPOSITORY");
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        // Backtrace::force_capture 是同步阻塞操作，在 panic storm 下会拖慢所有线程。
        // 只在实际写入 crash.log 时才 capture，stderr 行只打轻量信息。
        let backtrace = std::backtrace::Backtrace::force_capture();

        let crash_report = format!(
            "=== PANIC ===\n\
             AsterDrive {version} - {platform} \n\
             Please report this crash to the developers with the above information: {repository}/issues/new?template=bug_report.yml\n\
             Timestamp: {timestamp}\n\
             Thread:    {thread_name}\n\
             Location:  {location}\n\
             Message:   {message}\n\
             Backtrace:\n{backtrace}\n\
             =============\n\n"
        );

        // 预绑定文件句柄，panic storm 下只打开一次。
        // try_lock() 失败说明另一个 panic 正在写，直接跳过而非阻塞。
        let file_mutex = CRASH_LOG.get_or_init(|| {
            OpenOptions::new()
                .create(true)
                .append(true)
                .open("crash.log")
                .map(Mutex::new)
                .unwrap_or_else(|_| {
                    // 打不开 crash.log（权限/磁盘满），退化为 Mutex<空文件句柄> 占位，
                    // 后续 try_lock 会得到 err 并静默跳过写入。
                    // 使用 /dev/null 作为无害的占位：
                    Mutex::new(
                        OpenOptions::new()
                            .write(true)
                            .open(if cfg!(unix) { "/dev/null" } else { "NUL" })
                            .expect("fallback sink should always open"),
                    )
                })
        });

        if let Ok(mut guard) = file_mutex.try_lock() {
            let _ = guard.write_all(crash_report.as_bytes());
        }

        eprintln!("[PANIC] thread '{thread_name}' at {location}: {message}");
    }));
}
