use revolt_database::{util::reference::Reference, Channel, Database, Invite, Member, User};
use revolt_models::v0::{self, InviteJoinResponse};
use revolt_result::{create_error, Result};
use rocket::{serde::json::Json, State};
use log::{info, error};

/// # Join Invite
///
/// Join an invite by its ID
#[openapi(tag = "Invites")]
#[post("/<target>")]
pub async fn join(
    db: &State<Database>,
    user: User,
    target: Reference,
) -> Result<Json<v0::InviteJoinResponse>> {
    if user.bot.is_some() {
        return Err(create_error!(IsBot));
    }

    user.can_acquire_server(db).await?;

    let invite = target.as_invite(db).await?;
    match &invite {
        Invite::Server { server, .. } => {
            let server = db.fetch_server(server).await?;
            let (_, channels) = Member::create(db, &server, &user, None).await?;

            info!("User {} joined server {} with {} channels", user.id, server.id, channels.len());
            
            // Set server notification preference to "all"
            if let Err(err) = revolt_database::tasks::notification_settings::set_server_notification_to_all(db, &user.id, &server.id).await {
                error!("Failed to set server notification settings for server {}: {}", server.id, err);
            } else {
                info!("Successfully set server notification settings for server {}", server.id);
            }
            
            // Set notification preferences for all channels in this server
            for channel in &channels {
                info!("Setting notification preferences for channel {}", channel.id());
                
                if let Err(err) = revolt_database::tasks::notification_settings::set_channel_notification_to_all(db, &user.id, channel.id()).await {
                    error!("Failed to set notification settings for channel {}: {}", channel.id(), err);
                }
            }

            Ok(Json(InviteJoinResponse::Server {
                channels: channels.into_iter().map(|c| c.into()).collect(),
                server: server.into(),
            }))
        }
        Invite::Group {
            channel, creator, ..
        } => {
            let mut channel = db.fetch_channel(channel).await?;
            channel.add_user_to_group(db, &user, creator).await?;
            
            info!("User {} joined group channel {}", user.id, channel.id());
            
            // Set notification setting for this channel
            if let Err(err) = revolt_database::tasks::notification_settings::set_channel_notification_to_all(db, &user.id, channel.id()).await {
                error!("Failed to set notification settings for group channel {}: {}", channel.id(), err);
            } else {
                info!("Successfully set notification settings for group channel {}", channel.id());
            }

            Ok(Json(InviteJoinResponse::Group {
                users: User::fetch_many_ids_as_mutuals(db, &user, &channel.recipients).await?,
                channel: channel.into(),
            }))
        }
    }
}
