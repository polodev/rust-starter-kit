use sea_orm_migration::MigrationTrait;

mod m20261030_000001_create_pages;
mod m20261031_000070_page_translations_optional_text;

/// Register this module's migrations here; they are merged by name with every other module.
pub fn all() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20261030_000001_create_pages::Migration),
        Box::new(m20261031_000070_page_translations_optional_text::Migration),
    ]
}
