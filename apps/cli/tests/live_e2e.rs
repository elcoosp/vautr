//! Live-server end-to-end test for `vautr-cli` (Wave B4).
//!
//! Drives the real `vautr-cli` binary (`CARGO_BIN_EXE_vautr-cli`) through the
//! full bws-style flow against a running Vautr server:
//!
//!   register → login (OPAQUE) → machine-account provision → create project →
//!   create secret (flag + stdin) → list → get (reveal+decrypt) → run (env
//!   injection, asserted) → edit → logout.
//!
//! Server resolution:
//! - Uses `VAUTR_SERVER` (default `http://127.0.0.1:8080`).
//! - If no server is reachable and `VAUTR_SERVER_BIN` points at a built
//!   `vautr-server` binary, this test boots one on a throwaway DB and tears it
//!   down. Otherwise the test skips (keeps the default `cargo test` green when
//!   no server is available).

use std::io::Read;
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_vautr-cli");

fn server_url() -> String {
    std::env::var("VAUTR_SERVER").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string())
}

/// Minimal GET /health probe to see whether a server is up.
fn server_up(url: &str) -> bool {
    let host_port = url
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .split('/')
        .next()
        .unwrap_or("127.0.0.1:8080");
    let Ok(mut stream) = TcpStream::connect_timeout(
        &host_port.parse().expect("valid host:port"),
        Duration::from_millis(1500),
    ) else {
        return false;
    };
    let req = format!("GET /health HTTP/1.1\r\nHost: {host_port}\r\nConnection: close\r\n\r\n");
    let _ = std::io::Write::write_all(&mut stream, req.as_bytes());
    let mut buf = String::new();
    let _ = stream.read_to_string(&mut buf);
    buf.contains("\"ok\"") || buf.contains("\"status\":\"ok\"")
}

/// Spawn a throwaway server from `VAUTR_SERVER_BIN` on a fresh DB.
fn spawn_server(server_bin: &PathBuf) -> Child {
    let db = std::env::temp_dir().join(format!("vautr_cli_e2e_{}.db", uuid::Uuid::new_v4()));
    let _ = std::fs::remove_file(&db);
    Command::new(server_bin)
        .env("VAUTR_DB_URL", format!("sqlite:{}", db.display()))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn vautr-server")
}

fn run_cli(server: &str, config: &str, args: &[&str], stdin: Option<&str>) -> Output {
    let mut cmd = Command::new(BIN);
    cmd.arg("--server").arg(server);
    cmd.args(args);
    cmd.env("VAUTR_CONFIG", config);
    cmd.env("VAUTR_PASSWORD", "correct horse battery staple");
    if stdin.is_some() {
        cmd.stdin(Stdio::piped());
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("spawn vautr-cli");
    if let Some(input) = stdin {
        use std::io::Write;
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    child.wait_with_output().expect("wait vautr-cli")
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

fn read_project_uuid(config: &str) -> String {
    let text = std::fs::read_to_string(config).expect("read config");
    let v: serde_json::Value = serde_json::from_str(&text).expect("parse config");
    v["project_keys"]
        .as_object()
        .and_then(|m| m.keys().next().cloned())
        .expect("a project key is stored")
}

#[test]
fn live_server_cli_e2e() {
    let server = server_url();

    // Optional: boot our own server from a pre-built binary.
    let mut child: Option<Child> = None;
    if !server_up(&server) {
        if let Ok(bin) = std::env::var("VAUTR_SERVER_BIN") {
            child = Some(spawn_server(&PathBuf::from(bin)));
            // Wait for readiness.
            let deadline = Instant::now() + Duration::from_secs(20);
            while Instant::now() < deadline {
                if server_up(&server) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(300));
            }
        }
    }
    if !server_up(&server) {
        eprintln!("SKIP: no live server at {server} (set VAUTR_SERVER and/or VAUTR_SERVER_BIN)");
        return;
    }

    let config =
        std::env::temp_dir().join(format!("vautr_cli_e2e_cfg_{}.json", uuid::Uuid::new_v4()));
    let config = config.to_str().unwrap().to_string();
    let user = format!("cli-e2e-{}@example.com", uuid::Uuid::new_v4().to_string());

    // 1. register
    let o = run_cli(&server, &config, &["register", &user], None);
    assert!(o.status.success(), "register failed: {}", err(&o));
    assert!(
        out(&o).contains("registered"),
        "register output: {}",
        out(&o)
    );

    // 2. login (OPAQUE)
    let o = run_cli(&server, &config, &["login", &user], None);
    assert!(o.status.success(), "login failed: {}", err(&o));
    assert!(
        out(&o).contains("logged in as"),
        "login output: {}",
        out(&o)
    );

    // 3. create project
    let o = run_cli(
        &server,
        &config,
        &["create", "project", "e2e-app", "--description", "E2E"],
        None,
    );
    assert!(o.status.success(), "create project failed: {}", err(&o));
    assert!(
        out(&o).contains("created project"),
        "create project output: {}",
        out(&o)
    );
    let project = read_project_uuid(&config);

    // 4. create secrets (flag + stdin)
    let o = run_cli(
        &server,
        &config,
        &[
            "create",
            "secret",
            "API_TOKEN",
            "--project",
            &project,
            "--value",
            "tok-abc-123",
        ],
        None,
    );
    assert!(o.status.success(), "create secret failed: {}", err(&o));
    let o = run_cli(
        &server,
        &config,
        &["create", "secret", "DB_PASS", "--project", &project],
        Some("pw-x\n"),
    );
    assert!(
        o.status.success(),
        "create secret (stdin) failed: {}",
        err(&o)
    );

    // 5. list
    let o = run_cli(&server, &config, &["list"], None);
    assert!(o.status.success(), "list failed: {}", err(&o));
    assert!(
        out(&o).contains("API_TOKEN"),
        "list missing API_TOKEN: {}",
        out(&o)
    );
    assert!(
        out(&o).contains("DB_PASS"),
        "list missing DB_PASS: {}",
        out(&o)
    );

    // 6. get (reveal + decrypt)
    let o = run_cli(&server, &config, &["get", "API_TOKEN"], None);
    assert!(o.status.success(), "get failed: {}", err(&o));
    assert_eq!(out(&o).trim(), "tok-abc-123", "get value mismatch");
    let o = run_cli(
        &server,
        &config,
        &["get", &format!("e2e-app/DB_PASS")],
        None,
    );
    assert_eq!(out(&o).trim(), "pw-x", "scoped get value mismatch");

    // 7. run with env injection + assert
    let o = run_cli(
        &server,
        &config,
        &[
            "run",
            "--",
            "sh",
            "-c",
            "printf '%s|%s' \"$API_TOKEN\" \"$DB_PASS\"",
        ],
        None,
    );
    assert!(o.status.success(), "run failed: {}", err(&o));
    assert_eq!(
        out(&o).trim(),
        "tok-abc-123|pw-x",
        "run env injection mismatch"
    );

    // 8. machine-account provision (issue/use an access token)
    let o = run_cli(
        &server,
        &config,
        &["machine-account", "--name", "ci-runner"],
        None,
    );
    assert!(o.status.success(), "machine-account failed: {}", err(&o));
    let otext = out(&o);
    assert!(
        otext.contains("machine account: ci-runner"),
        "ma output: {otext}"
    );
    assert!(
        otext.contains("access token (shown once):"),
        "ma token missing: {otext}"
    );

    // 9. edit secret value, then get reflects it
    let o = run_cli(
        &server,
        &config,
        &["edit", "secret", "API_TOKEN", "--value", "rotated-xyz"],
        None,
    );
    assert!(o.status.success(), "edit failed: {}", err(&o));
    let o = run_cli(&server, &config, &["get", "API_TOKEN"], None);
    assert_eq!(out(&o).trim(), "rotated-xyz", "edit not reflected");

    // 10. logout, then list fails
    let o = run_cli(&server, &config, &["logout"], None);
    assert!(o.status.success(), "logout failed: {}", err(&o));
    let o = run_cli(&server, &config, &["list"], None);
    assert!(!o.status.success(), "list should fail after logout");

    // teardown
    let _ = std::fs::remove_file(&config);
    if let Some(mut c) = child {
        let _ = c.kill();
        let _ = c.wait();
    }
}
