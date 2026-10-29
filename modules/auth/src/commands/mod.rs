use loco_rs::task::Tasks;

mod prune_sessions;
// mods-inject (do not remove)

/// Commands of this module; run them with `cargo run -- task <signature>`.
#[allow(unused_variables)]
pub fn register(tasks: &mut Tasks) {
    tasks.register(prune_sessions::PruneSessions);
    // tasks-inject (do not remove)
}
