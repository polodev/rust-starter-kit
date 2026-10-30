use loco_rs::task::Tasks;

// mods-inject (do not remove)

/// Commands of this module; run them with `cargo run -- task <signature>`.
pub fn register(tasks: &mut Tasks) {
    tasks.register(crate::import::ImportMedia);
    tasks.register(crate::disk::MoveLocalUploads);
    // tasks-inject (do not remove)
}
