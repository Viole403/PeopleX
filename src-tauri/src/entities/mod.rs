//! Entity SeaORM berdampingan dengan rusqlite.
//!
//! Jalur produksi tetap memakai rusqlite; modul ini untuk spike paritas:
//! membaca database yang sama lewat SeaORM dan membandingkan hasilnya.

pub mod system_setting {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "system_settings")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub setting_key: String,
        pub setting_value: Option<String>,
        pub setting_group: String,
        pub created_at: Option<String>,
        pub updated_at: Option<String>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod user {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "users")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub employee_id: Option<i32>,
        pub username: String,
        pub email: String,
        pub password: String,
        pub status: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod company {    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "companies")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub code: String,
        pub name: String,
        pub legal_name: Option<String>,
        pub address: Option<String>,
        pub city: Option<String>,
        pub province: Option<String>,
        pub postal_code: Option<String>,
        pub phone: Option<String>,
        pub email: Option<String>,
        pub logo: Option<String>,
        pub npwp: Option<String>,
        pub established_date: Option<String>,
        pub created_at: Option<String>,
        pub updated_at: Option<String>,
        pub deleted_at: Option<String>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(test)]
mod tests {
    use super::system_setting::Entity as SettingEntity;
    use crate::{db, seed};
    use sea_orm::EntityTrait;

    #[tokio::test]
    async fn baca_seaorm_sama_dengan_rusqlite() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("t.db");
        let pool = db::init_pool(&path).expect("pool");
        let mut c = pool.get().expect("get");
        db::migrate(&mut c).expect("migrate");
        seed::seed(&mut c).expect("seed");
        drop(c);
        let url = format!("sqlite://{}?mode=rwc", path.display());
        let db = sea_orm::Database::connect(&url).await.expect("connect");
        let via_sea = SettingEntity::find().all(&db).await.expect("baca");
        let conn = pool.get().expect("get");
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM system_settings", [], |r| r.get(0))
            .expect("count");
        assert_eq!(via_sea.len() as i64, n);
        let company = via_sea
            .iter()
            .find(|s| s.setting_key == "company_name")
            .expect("ada");
        let direct: String = conn
            .query_row(
                "SELECT setting_value FROM system_settings WHERE setting_key = 'company_name'",
                [],
                |r| r.get(0),
            )
            .expect("nilai");
        assert_eq!(company.setting_value.as_deref(), Some(direct.as_str()));
    }
}

pub mod notification {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "notifications")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub user_id: i32,
        #[sea_orm(column_name = "type")]
        pub kind: String,
        pub title: String,
        pub message: Option<String>,
        pub link: Option<String>,
        pub is_read: i32,
        pub read_at: Option<String>,
        pub created_at: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod announcement {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "announcements")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub title: String,
        pub content: String,
        pub target_type: String,
        pub publish_at: Option<String>,
        pub expire_at: Option<String>,
        pub status: String,
        pub created_by: Option<i32>,
        pub created_at: String,
        pub updated_at: Option<String>,
        pub deleted_at: Option<String>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod announcement_target {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "announcement_targets")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub announcement_id: i32,
        pub target_type: String,
        pub target_id: i32,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod announcement_read {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "announcement_reads")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub announcement_id: i32,
        pub employee_id: i32,
        pub read_at: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod user_role {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "user_roles")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub user_id: i32,
        pub role_id: i32,
        pub created_at: Option<String>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod employee {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "employees")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub employee_number: String,
        pub first_name: String,
        pub last_name: Option<String>,
        pub department_id: Option<i32>,
        pub branch_id: Option<i32>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod department {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "departments")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub name: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod branch {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "branches")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub name: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod role {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "roles")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub name: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}
