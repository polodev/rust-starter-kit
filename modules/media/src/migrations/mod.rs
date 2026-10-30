use sea_orm_migration::MigrationTrait;

mod m20261030_000030_create_media_files;
mod m20261031_000060_spatie_media_table;

/// Register this module's migrations here; they are merged by name with every other module.
pub fn all() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20261030_000030_create_media_files::Migration),
        Box::new(m20261031_000060_spatie_media_table::Migration),
    ]
}
