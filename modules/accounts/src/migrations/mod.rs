use sea_orm_migration::MigrationTrait;

/// The account area stores its data in the `users`, `sessions` and `media_files` tables.
pub fn all() -> Vec<Box<dyn MigrationTrait>> {
    Vec::new()
}
