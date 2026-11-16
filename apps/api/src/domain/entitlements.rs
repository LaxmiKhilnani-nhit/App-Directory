use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::models::AppSummary;

/// The single definition of "what may this user see".
///
/// Effective access = direct grants ∪ grants through any group the user belongs to, where
/// membership is either explicit (`group_members`), derived from `users.department`, or
/// implicit through the built-in All Users group. Disabled users and disabled/deleted apps
/// never match.
const ENTITLED_APPS_SQL: &str = r#"
select a.id,
       a.name,
       a.url,
       a.description,
       a.category,
       a.color,
       a.sort_order,
       a.offline_visible,
       a.icon_source,
       a.icon_etag,
       a.updated_at
  from apps a
 where a.deleted_at is null
   and a.is_enabled
   and exists (
        select 1 from users u
         where u.id = $1
           and u.is_active
   )
   and (
        exists (
            select 1 from user_apps ua
             where ua.app_id = a.id
               and ua.user_id = $1
        )
        or exists (
            select 1
              from app_groups ag
              join groups g on g.id = ag.group_id
              left join users u on u.id = $1
             where ag.app_id = a.id
               and (
                   g.is_all_users
                   or (g.auto_department is not null and g.auto_department = u.department)
                   or exists (
                       select 1 from group_members gm
                        where gm.group_id = g.id
                          and gm.user_id = $1
                   )
               )
        )
   )
 order by a.sort_order, a.name
"#;

/// Apps the user is entitled to, in display order.
pub async fn apps_for_user(pool: &PgPool, user_id: Uuid) -> sqlx::Result<Vec<AppSummary>> {
    sqlx::query_as::<_, AppSummary>(ENTITLED_APPS_SQL)
        .bind(user_id)
        .fetch_all(pool)
        .await
}

/// Whether the user may see one specific app. Used by the icon route and future SSO hand-off.
pub async fn is_entitled(pool: &PgPool, user_id: Uuid, app_id: Uuid) -> sqlx::Result<bool> {
    let sql = format!("select exists (select 1 from ({ENTITLED_APPS_SQL}) entitled where id = $2)");

    sqlx::query_scalar::<_, bool>(&sql)
        .bind(user_id)
        .bind(app_id)
        .fetch_one(pool)
        .await
}
