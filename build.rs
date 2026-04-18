//! 构建脚本：注入构建时间并兜底生成前端占位产物。

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=frontend-panel/dist");

    // 构建时间
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    println!("cargo:rustc-env=ASTER_BUILD_TIME={now}");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let dist_path = Path::new(&manifest_dir).join("frontend-panel/dist");

    if !dist_path.exists() {
        eprintln!("Warning: frontend-panel/dist directory not found!");
        eprintln!("Please build the frontend first:");
        eprintln!("  cd frontend-panel && bun install && bun run build");

        create_fallback_files(&dist_path);
    }
}

fn create_fallback_files(dist_path: &Path) {
    fs::create_dir_all(dist_path).expect("Failed to create dist directory");

    let fallback_html = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>AsterDrive - Frontend Not Built</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            max-width: 600px;
            margin: 100px auto;
            padding: 20px;
            text-align: center;
            color: #333;
        }
        .warning {
            background: #fff3cd;
            border: 1px solid #ffeaa7;
            padding: 20px;
            border-radius: 8px;
            margin: 20px 0;
        }
        code {
            background: #f1f3f4;
            padding: 2px 6px;
            border-radius: 4px;
            font-family: monospace;
        }
    </style>
</head>
<body>
    <h1>AsterDrive</h1>
    <div class="warning">
        <h2>Frontend Not Built</h2>
        <p>The admin panel needs to be built before it can be served.</p>
        <p>Run:</p>
        <p><code>cd frontend-panel && bun install && bun run build</code></p>
    </div>
    <p>API is still available at <code>/api/v1/</code></p>
</body>
</html>"#;

    fs::write(dist_path.join("index.html"), fallback_html)
        .expect("Failed to write fallback index.html");

    fs::write(dist_path.join("favicon.ico"), []).expect("Failed to write fallback favicon");

    fs::create_dir_all(dist_path.join("assets")).expect("Failed to create assets directory");
}
