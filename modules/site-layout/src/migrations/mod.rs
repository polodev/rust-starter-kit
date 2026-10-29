use sea_orm_migration::MigrationTrait;

/// This module has no tables of its own.
pub fn all() -> Vec<Box<dyn MigrationTrait>> {
    Vec::new()
}
