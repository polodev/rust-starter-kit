use loco_rs::task::Tasks;

mod tinker_eloquent;
// mods-inject (do not remove)

/// Commands of this module; run them with `cargo run -- task <signature>`.
pub fn register(tasks: &mut Tasks) {
    tasks.register(tinker_eloquent::TinkerEloquent);
    // tasks-inject (do not remove)
}
