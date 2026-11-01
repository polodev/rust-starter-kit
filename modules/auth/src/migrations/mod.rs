use sea_orm_migration::MigrationTrait;

mod m20261030_000020_create_sessions;
mod m20261031_000091_create_user_security_tokens;

/// Register this module's migrations here; they are merged by name with every other module.
pub fn all() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20261030_000020_create_sessions::Migration),
        Box::new(m20261031_000091_create_user_security_tokens::Migration),
    ]
}
