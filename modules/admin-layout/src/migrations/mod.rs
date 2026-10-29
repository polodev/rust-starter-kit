use sea_orm_migration::MigrationTrait;

/// The admin layout has no tables of its own.
pub fn all() -> Vec<Box<dyn MigrationTrait>> {
    Vec::new()
}
