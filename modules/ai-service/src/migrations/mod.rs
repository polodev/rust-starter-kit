use sea_orm_migration::MigrationTrait;

mod m20261031_000090_create_ai_api_logs;

/// This module's migrations; they are merged by name with every other module.
pub fn all() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(m20261031_000090_create_ai_api_logs::Migration)]
}
