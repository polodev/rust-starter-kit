use sea_orm_migration::MigrationTrait;

mod m20261031_000070_create_queue_watcher_tables;

/// Register this module's migrations here; they are merged by name with every other module.
pub fn all() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(
        m20261031_000070_create_queue_watcher_tables::Migration,
    )]
}
