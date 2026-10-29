use loco_rs::task::Tasks;

mod assign_role;
mod list_roles;
mod revoke_role;
mod sync_roles;
// mods-inject (do not remove)

/// Commands of this module; run them with `cargo run -- task <signature>`.
#[allow(unused_variables)]
pub fn register(tasks: &mut Tasks) {
    tasks.register(sync_roles::SyncRoles);
    tasks.register(list_roles::ListRoles);
    tasks.register(assign_role::AssignRole);
    tasks.register(revoke_role::RevokeRole);
    // tasks-inject (do not remove)
}
