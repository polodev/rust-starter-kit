use loco_rs::task::Tasks;

mod clean;
// mods-inject (do not remove)

/// Commands of this module; run them with `cargo run -- task <signature>`.
pub fn register(tasks: &mut Tasks) {
    tasks.register(clean::Clean);
    // tasks-inject (do not remove)
}
