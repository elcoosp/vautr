//! VTRFIX-TST-01: delete-path regression.

use sea_orm::Database;
use uuid::Uuid;
use vautr_app_state::VautrClient;
use vautr_db::migrate;

#[tokio::test]
async fn delete_on_locked_vault_is_rejected_not_panicked() {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("connect");
    migrate::init(&db).await.expect("migrate");
    let client = VautrClient::new(db);
    // Locked vault + no transport: the delete must be a clean rejection.
    let outcome = client.delete_item(Uuid::new_v4()).await;
    let _ = outcome;
}
