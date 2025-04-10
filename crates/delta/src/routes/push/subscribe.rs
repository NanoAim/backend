use authifier::{
    models::{Session, WebPushSubscription},
    Authifier,
};
use revolt_result::{create_database_error, Result};
use rocket::{serde::json::Json, State};
use rocket_empty::EmptyResponse;

/// # Push Subscribe
///
/// Create a new Web Push subscription.
///
/// If an existing subscription exists on this session, it will be removed.
/// Checks for duplicate tokens to prevent multiple notifications to the same device.
#[openapi(tag = "Web Push")]
#[post("/subscribe", data = "<data>")]
pub async fn subscribe(
    authifier: &State<Authifier>,
    mut session: Session,
    data: Json<WebPushSubscription>,
) -> Result<EmptyResponse> {
    let subscription_data = data.into_inner();

    // Check if this user already has a session with this token
    if let Ok(sessions) = authifier.database.find_sessions(&session.user_id).await {
        for existing_session in sessions {
            if let Some(existing_sub) = existing_session.subscription {
                // For APN endpoint, check if the auth (token) is the same
                if existing_sub.endpoint == "apn"
                    && existing_sub.auth == subscription_data.auth
                    && existing_session.id != session.id
                {
                    // Token already exists in another session, just return success
                    return Ok(EmptyResponse);
                }
            }
        }
    }

    session.subscription = Some(subscription_data);
    session
        .save(authifier)
        .await
        .map(|_| EmptyResponse)
        .map_err(|_| create_database_error!("save", "session"))
}
