use loco_rs::task::Tasks;

mod prune_tokens;
// mods-inject (do not remove)

/// Commands of this module; run them with `cargo run -- task <signature>`.
#[allow(unused_variables)]
pub fn register(tasks: &mut Tasks) {
    tasks.register(prune_tokens::PruneTokens);
    // tasks-inject (do not remove)
}
