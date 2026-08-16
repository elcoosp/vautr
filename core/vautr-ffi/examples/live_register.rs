//! Reproduce the mobile register+login flow against a live vautr-server.
//! Run: cargo run -p vautr-ffi --example live_register -- http://localhost:8080
use std::sync::Arc;

use vautr_ffi::client::MobileClient;

#[tokio::main]
async fn main() {
    let server = std::env::args().nth(1).unwrap_or_else(|| "http://localhost:8080".into());
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("vault.sqlite3").to_string_lossy().into_owned();
    let client = Arc::new(MobileClient::new(db_path).expect("client"));
    let username = format!("live_{}", uuid::Uuid::new_v4());
    println!("registering {username} on {server}");
    match client.register(server.clone(), username.clone(), "password12345".into()).await {
        Ok(out) => println!("REGISTER OK: {out}"),
        Err(e) => println!("REGISTER ERR: {e:?}"),
    }
    println!("logging in {username}");
    match client.login(server, username.clone(), "password12345".into()).await {
        Ok(out) => println!("LOGIN OK: {out}"),
        Err(e) => println!("LOGIN ERR: {e:?}"),
    }
}
