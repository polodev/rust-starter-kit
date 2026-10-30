use sea_orm_migration::MigrationTrait;

/// Register this module's migrations here; they are merged by name with every other module.
pub fn all() -> Vec<Box<dyn MigrationTrait>> {
    Vec::new()
}
