//! SeaORM 2.0 entity definitions for the Vautr client local DB.
//! Spec: docs/architecture/db-contract.md §3. Entities: item_overview,
//! item_payload, sync_meta, local_blacklist, quarantine.
//!
//! SeaORM 2.0.2 `#[sea_orm::model]` attribute-macro form: the macro emits
//! `Entity`/`ActiveModel`/`Column`/`COLUMN`/`PrimaryKey` and (with
//! `DeriveEntityModel`) the `Model` struct. The separate `Relation` enum is no
//! longer required (relations are expressed in the transaction layer,
//! db-contract §5, not via relation-field attributes). `DashMapState` is stored
//! as a plain `String` column matching the `CHECK` constraint in db-contract §3.

use serde::{Deserialize, Serialize};

/// Blacklist entry state (db-contract §3, data.md §7.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DashMapState {
    ToxicIgnored,
    ValidIgnored,
}

impl DashMapState {
    /// Persisted string form (matches `CHECK` constraint in db-contract §3).
    pub fn as_db_str(&self) -> &'static str {
        match self {
            DashMapState::ToxicIgnored => "ToxicIgnored",
            DashMapState::ValidIgnored => "ValidIgnored",
        }
    }

    /// Parse from the DB string form.
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "ToxicIgnored" => Some(DashMapState::ToxicIgnored),
            "ValidIgnored" => Some(DashMapState::ValidIgnored),
            _ => None,
        }
    }
}

/// `ItemOverview` entity (Hot Data). db-contract §3.
pub mod item_overview {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "item_overviews")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub uuid: String,
        pub version: i64,
        pub enc_key_gen: i64,
        pub deleted_date: Option<i64>,
        // DecryptedOverview (Plaintext for UI/FTS)
        pub overview_title: String,
        pub overview_subtitle: String,
        pub overview_icon_key: String,
        pub overview_urls: String, // JSON Array
        pub created_at: i64,
        pub updated_at: i64,
    }

    impl ActiveModelBehavior for ActiveModel {}
}

/// `ItemPayload` entity (Cold Data). db-contract §3.
pub mod item_payload {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "item_payloads")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub uuid: String,
        pub payload: Vec<u8>,
    }

    impl ActiveModelBehavior for ActiveModel {}
}

/// `SyncMeta` singleton entity. db-contract §3.
pub mod sync_meta {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "sync_meta")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: i32, // Always 1
        pub sync_cursor: i64,
        pub min_enc_key_gen: i64,
        pub svk_ciphertext_blob: Vec<u8>,
    }

    impl ActiveModelBehavior for ActiveModel {}
}

/// `LocalBlacklist` entity (the batch-persisted DashMap). db-contract §3.
pub mod local_blacklist {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "local_blacklist")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub uuid: String,
        pub ignored_version: i64,
        pub state: String, // DashMapState::as_db_str()
    }

    impl ActiveModelBehavior for ActiveModel {}
}

/// `Quarantine` entity (the Reaper's domain). db-contract §3.
pub mod quarantine {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "quarantine")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub uuid: String,
        pub target_version: i64,
        pub quarantine_until: i64,
        pub retries: i32,
    }

    impl ActiveModelBehavior for ActiveModel {}
}
