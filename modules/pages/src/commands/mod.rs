use loco_rs::task::Tasks;

mod check_translations;
// mods-inject (do not remove)

/// Commands of this module; run them with `cargo run -- task <signature>`.
#[allow(unused_variables)]
pub fn register(tasks: &mut Tasks) {
    tasks.register(check_translations::CheckTranslations);
    // tasks-inject (do not remove)
}
