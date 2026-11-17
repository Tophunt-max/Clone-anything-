//! Workspace roles and the rules that use them (docs/platform/01-architecture.md §5).

use serde::{Deserialize, Serialize};

use crate::error::ApiError;

/// Ordered from least to most power: `Viewer < Editor < Maintainer < Admin < Owner`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Viewer,
    Editor,
    Maintainer,
    Admin,
    Owner,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Viewer => "viewer",
            Role::Editor => "editor",
            Role::Maintainer => "maintainer",
            Role::Admin => "admin",
            Role::Owner => "owner",
        }
    }

    pub fn parse(s: &str) -> Option<Role> {
        Some(match s {
            "viewer" => Role::Viewer,
            "editor" => Role::Editor,
            "maintainer" => Role::Maintainer,
            "admin" => Role::Admin,
            "owner" => Role::Owner,
            _ => return None,
        })
    }
}

/// What each action needs. One table, so a review can check every rule in one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    ViewWorkspace,
    RenameWorkspace,
    DeleteWorkspace,
    ManageMembers,
    ViewProject,
    CreateProject,
    EditProject,
    DeleteProject,
    /// Run a read-only engine tool (view evidence, listings).
    RunReadOnlyTool,
    /// Run a tool that changes project state or uses the sandboxed worker.
    RunTool,
    /// Approve a tool that runs code or changes a device; turn Deep mode on or off.
    ApproveSensitive,
}

pub fn minimum(action: Action) -> Role {
    match action {
        Action::ViewWorkspace | Action::ViewProject | Action::RunReadOnlyTool => Role::Viewer,
        Action::RunTool => Role::Editor,
        Action::CreateProject | Action::EditProject => Role::Maintainer,
        Action::ApproveSensitive => Role::Maintainer,
        Action::RenameWorkspace | Action::ManageMembers | Action::DeleteProject => Role::Admin,
        Action::DeleteWorkspace => Role::Owner,
    }
}

pub fn require(role: Role, action: Action) -> Result<(), ApiError> {
    let need = minimum(action);
    if role >= need { Ok(()) } else { Err(ApiError::forbidden("insufficient_role", &format!("This needs the {} role or higher.", need.as_str()))) }
}

/// Whether `actor` may give a member role `new` (`None` = remove them) when they now have
/// `current` (`None` = not a member yet). Owners may do anything; admins only below themselves.
/// The "last owner" rule is checked separately against the database.
pub fn may_change_member(actor: Role, current: Option<Role>, new: Option<Role>) -> bool {
    if actor == Role::Owner {
        return true;
    }
    actor >= Role::Admin && current.is_none_or(|c| c < actor) && new.is_none_or(|n| n < actor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_are_ordered_and_parse_round_trip() {
        assert!(Role::Viewer < Role::Editor && Role::Editor < Role::Maintainer && Role::Maintainer < Role::Admin && Role::Admin < Role::Owner);
        for r in [Role::Viewer, Role::Editor, Role::Maintainer, Role::Admin, Role::Owner] {
            assert_eq!(Role::parse(r.as_str()), Some(r));
        }
        assert_eq!(Role::parse("root"), None);
    }

    #[test]
    fn the_permission_table() {
        assert!(require(Role::Viewer, Action::ViewProject).is_ok());
        assert!(require(Role::Editor, Action::CreateProject).is_err());
        assert!(require(Role::Maintainer, Action::CreateProject).is_ok());
        assert!(require(Role::Maintainer, Action::DeleteProject).is_err());
        assert!(require(Role::Admin, Action::DeleteWorkspace).is_err());
        assert!(require(Role::Owner, Action::DeleteWorkspace).is_ok());
    }

    #[test]
    fn member_changes() {
        use Role::*;
        assert!(may_change_member(Owner, Some(Owner), None));
        assert!(may_change_member(Admin, None, Some(Maintainer)));
        assert!(!may_change_member(Admin, None, Some(Admin)), "admins can't create admins");
        assert!(!may_change_member(Admin, Some(Admin), Some(Viewer)), "admins can't demote admins");
        assert!(!may_change_member(Admin, Some(Owner), None));
        assert!(!may_change_member(Maintainer, Some(Viewer), Some(Editor)));
        assert!(may_change_member(Admin, Some(Editor), None));
    }
}
