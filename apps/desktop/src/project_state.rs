//! Testable, GPUI-agnostic view state for the desktop Projects / roles /
//! members / Secrets UI.
//!
//! Pure data + transitions; the GPUI layer applies them inside
//! `cx.update_mut(...)` followed by `cx.notify()`. No GPUI types here, so the
//! state machine is unit-testable without a window.

use crate::api_client::{ProjectDto, ProjectMemberDto, SecretDto};

/// Which detail panel is shown for the selected project.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DetailTab {
    Members,
    Secrets,
}

/// The whole Projects/Secrets screen state.
pub struct ProjectsState {
    // ── Project list ──────────────────────────────────────────────────
    pub projects: Vec<ProjectDto>,
    pub selected_index: Option<usize>,
    pub loading: bool,
    pub error_message: Option<String>,
    pub status: String,

    // ── Detail of the selected project ────────────────────────────────
    pub members: Vec<ProjectMemberDto>,
    pub secrets: Vec<SecretDto>,
    pub detail_tab: DetailTab,

    // ── Create-project form ───────────────────────────────────────────
    pub new_name: String,
    pub new_description: String,
    pub new_kind: String, // "personal" | "shared"

    // ── Add-member form ───────────────────────────────────────────────
    pub member_user_uuid: String,
    pub member_role: String,       // "owner"|"admin"|"manager"|"member"
    pub member_permission: String, // "can_view"|"can_edit"|"can_manage"

    // ── Add-secret form ───────────────────────────────────────────────
    pub secret_key: String,
    pub secret_value: String,

    // ── Revealed secret plaintext ─────────────────────────────────────
    pub revealed: Option<(String, String)>,

    // ── Offboard form ─────────────────────────────────────────────────
    pub offboard_user_uuid: String,
    pub offboard_result: Option<String>,
}

impl Default for ProjectsState {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectsState {
    pub fn new() -> Self {
        Self {
            projects: Vec::new(),
            selected_index: None,
            loading: false,
            error_message: None,
            status: String::new(),
            members: Vec::new(),
            secrets: Vec::new(),
            detail_tab: DetailTab::Members,
            new_name: String::new(),
            new_description: String::new(),
            new_kind: "shared".into(),
            member_user_uuid: String::new(),
            member_role: "member".into(),
            member_permission: "can_view".into(),
            secret_key: String::new(),
            secret_value: String::new(),
            revealed: None,
            offboard_user_uuid: String::new(),
            offboard_result: None,
        }
    }

    // ── Project list ──────────────────────────────────────────────────

    pub fn set_projects(&mut self, projects: Vec<ProjectDto>) {
        self.projects = projects;
        self.reconcile_selection();
    }

    fn reconcile_selection(&mut self) {
        match self.selected_index {
            Some(i) if i < self.projects.len() => {}
            _ => {
                self.selected_index = if self.projects.is_empty() {
                    None
                } else {
                    Some(0)
                };
            }
        }
    }

    /// Select a project by index; loads no data itself (the view does).
    pub fn select_project(&mut self, index: usize) -> bool {
        if index < self.projects.len() {
            self.selected_index = Some(index);
            true
        } else {
            false
        }
    }

    pub fn selected_project(&self) -> Option<&ProjectDto> {
        self.selected_index.and_then(|i| self.projects.get(i))
    }

    pub fn selected_project_uuid(&self) -> Option<String> {
        self.selected_project().map(|p| p.uuid.clone())
    }

    /// Clear the members/secrets detail so the next fetch repopulates it.
    pub fn clear_detail(&mut self) {
        self.members.clear();
        self.secrets.clear();
        self.revealed = None;
    }

    // ── Detail data ───────────────────────────────────────────────────

    pub fn set_members(&mut self, members: Vec<ProjectMemberDto>) {
        self.members = members;
    }

    pub fn set_secrets(&mut self, secrets: Vec<SecretDto>) {
        self.secrets = secrets;
        self.revealed = None;
    }

    // ── Feedback helpers ──────────────────────────────────────────────

    pub fn show_error(&mut self, message: impl Into<String>) {
        self.error_message = Some(message.into());
    }

    pub fn dismiss_error(&mut self) {
        self.error_message = None;
    }

    pub fn set_status(&mut self, message: impl Into<String>) {
        self.status = message.into();
    }

    // ── Reveal ────────────────────────────────────────────────────────

    /// Store the revealed secret plaintext (wiped on project switch).
    pub fn reveal_secret(&mut self, key: String, plaintext: String) {
        self.revealed = Some((key, plaintext));
    }

    pub fn clear_revealed(&mut self) {
        self.revealed = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(name: &str) -> ProjectDto {
        ProjectDto {
            uuid: format!("uuid-{name}"),
            name: name.into(),
            description: None,
            kind: "shared".into(),
            role: "member".into(),
            permission: Some("can_view".into()),
            created_at: 1,
            updated_at: 1,
        }
    }

    #[test]
    fn selection_reconciles_after_replacement() {
        let mut st = ProjectsState::new();
        st.set_projects(vec![project("A"), project("B"), project("C")]);
        st.select_project(2);
        assert_eq!(st.selected_index, Some(2));
        assert_eq!(st.selected_project().unwrap().name, "C");

        // Replacing with a shorter list clamps the selection.
        st.set_projects(vec![project("X")]);
        assert_eq!(st.selected_index, Some(0));
        assert_eq!(st.selected_project().unwrap().name, "X");
    }

    #[test]
    fn empty_list_has_no_selection() {
        let mut st = ProjectsState::new();
        st.set_projects(vec![]);
        assert_eq!(st.selected_index, None);
        assert!(!st.select_project(0));
    }

    #[test]
    fn set_secrets_clears_revealed() {
        let mut st = ProjectsState::new();
        st.reveal_secret("K".into(), "V".into());
        assert!(st.revealed.is_some());
        st.set_secrets(vec![]);
        assert!(st.revealed.is_none());
    }

    #[test]
    fn clear_detail_drops_members_secrets_and_reveal() {
        let mut st = ProjectsState::new();
        st.set_members(vec![ProjectMemberDto {
            user_uuid: "u1".into(),
            display_name: None,
            role: "member".into(),
            permission: "can_view".into(),
            added_at: 0,
            hide_password: false,
        }]);
        st.set_secrets(vec![SecretDto {
            uuid: "s1".into(),
            project_uuid: "p1".into(),
            key: "K".into(),
            version: 1,
            created_by: "u1".into(),
            last_accessed_at: None,
            created_at: 0,
            updated_at: 0,
        }]);
        st.reveal_secret("K".into(), "V".into());
        st.clear_detail();
        assert!(st.members.is_empty());
        assert!(st.secrets.is_empty());
        assert!(st.revealed.is_none());
    }
}
