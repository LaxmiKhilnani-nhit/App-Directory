use std::collections::HashSet;

use uuid::Uuid;

use crate::store::Data;

/// What a user may see, defined in one place.
///
/// Effective access = direct user grants ∪ group grants, where group membership is either
/// explicit (`group_members`), derived from `users.department` (auto-by-department groups),
/// or implicit through the built-in All Users group. Inactive users and
/// disabled/deleted apps never match.
pub fn apps_for_user(data: &Data, user_id: Uuid) -> Vec<crate::store::AppRow> {
    let Some(user) = data.user_by_id(user_id) else {
        return Vec::new();
    };
    if !user.is_active {
        return Vec::new();
    }

    let mut entitled_groups: HashSet<Uuid> = HashSet::new();

    for group in &data.groups {
        if group.is_all_users {
            entitled_groups.insert(group.id);
            continue;
        }
        if let Some(department) = group.auto_department.as_deref() {
            if user.department.as_deref() == Some(department) {
                entitled_groups.insert(group.id);
            }
            continue;
        }
        if data
            .group_members
            .iter()
            .any(|member| member.group_id == group.id && member.user_id == user_id)
        {
            entitled_groups.insert(group.id);
        }
    }

    let mut apps: Vec<crate::store::AppRow> = data
        .apps
        .iter()
        .filter(|app| {
            if app.deleted_at.is_some() || !app.is_enabled {
                return false;
            }
            if data
                .user_apps
                .iter()
                .any(|grant| grant.app_id == app.id && grant.user_id == user_id)
            {
                return true;
            }
            data.app_groups
                .iter()
                .any(|grant| grant.app_id == app.id && entitled_groups.contains(&grant.group_id))
        })
        .cloned()
        .collect();

    apps.sort_by(|left, right| {
        left.sort_order
            .cmp(&right.sort_order)
            .then_with(|| left.name.cmp(&right.name))
    });
    apps
}

/// Whether the user may see one specific app. Used by the icon route.
pub fn is_entitled(data: &Data, user_id: Uuid, app_id: Uuid) -> bool {
    apps_for_user(data, user_id).iter().any(|app| app.id == app_id)
}
