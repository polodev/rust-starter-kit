use sea_orm_migration::MigrationTrait;

mod m20261031_000060_create_activity_log;

/// Register this module's migrations here; they are merged by name with every other module.
pub fn all() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(m20261031_000060_create_activity_log::Migration)]
}
