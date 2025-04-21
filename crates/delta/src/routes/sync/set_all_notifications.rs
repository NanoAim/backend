// Import chrono for timestamp handling
use chrono::prelude::*;
// Import the necessary database structures and traits
use revolt_database::{Database, User, UserSettingsImpl};
// Import revolt models for data structures
use revolt_models::v0;
// Import Error handling utilities
use revolt_result::{create_error, Result};
// Import JsonSchema for OpenAPI documentation generation
use revolt_rocket_okapi::JsonSchema;
// Import Rocket components for handling HTTP requests and responses
use rocket::{serde::json::Json, State};
// Import EmptyResponse for returning an empty successful response
use rocket_empty::EmptyResponse;
// Import serialization and deserialization traits
use serde::{Deserialize, Serialize};
// Import HashMap for storing notification settings
use std::collections::HashMap;
// Import UUID for task identification
use uuid::Uuid;

/// # Notification Mode Request
///
/// Request object for setting notification mode for channels and servers
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NotificationModeRequest {
    /// Type of notification mode to set
    /// Can be "all" or "mute"
    #[serde(rename = "type")]
    pub mode_type: NotificationMode,

    /// Option to skip automatic resyncing with other APIs
    #[serde(default)]
    pub skip_resync: bool,
}

/// # Notification Mode
///
/// Notification mode for channels and servers
// Define an enum for notification modes that can be serialized/deserialized and documented
#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone)]
#[serde(rename_all = "lowercase")]
pub enum NotificationMode {
    /// All messages trigger notifications
    All,
    /// Only mentions trigger notifications
    Mute,
}

/// # Task Status Response
///
/// Response with task ID for tracking background job
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TaskResponse {
    /// Unique task ID for tracking the background job
    pub task_id: String,
    /// Status message
    pub message: String,
    /// Flag to indicate whether client should skip automatic resyncing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_resync: Option<bool>,
}

/// # Set All Notifications
///
/// Update notification settings for all channels and servers a user is a member of.
/// This operation runs in the background and returns a task ID for tracking.
// Mark this function as an OpenAPI endpoint with the "Sync" tag
#[openapi(tag = "Sync")]
// Define a POST endpoint that takes JSON data
#[post("/settings/set_all_notifications", data = "<data>")]
pub async fn set_all_notifications(
    // Get a reference to the database from the application state
    db: &State<Database>,
    // Get the authenticated user
    user: User,
    // Get the notification mode from the request body
    data: Json<NotificationModeRequest>,
) -> Result<Json<TaskResponse>> {
    // Generate a unique task ID
    let task_id = Uuid::new_v4().to_string();

    // Extract the data from the JSON wrapper
    let data = data.into_inner();
    let mode = data.mode_type;
    let skip_resync = data.skip_resync;

    // Clone data needed for the background task
    let user_id = user.id.clone();
    let db_clone = db.inner().clone();

    // Launch a background task to process the notification updates
    tokio::spawn(async move {
        // Process the notification update in the background
        if let Err(e) = process_notification_update(&db_clone, &user_id, mode).await {
            // Log any errors that occur during processing
            eprintln!(
                "Error processing notification update for user {}: {:?}",
                user_id, e
            );
        }
    });

    // Return a task ID and confirmation message to the client immediately
    Ok(Json(TaskResponse {
        task_id,
        message: "Notification update job queued successfully. Changes will be applied in the background.".to_string(),
        skip_resync: if skip_resync { Some(true) } else { None },
    }))
}

/// Process the notification update in the background
///
/// This function handles the actual work of updating notification settings
async fn process_notification_update(
    db: &Database,
    user_id: &str,
    mode: NotificationMode,
) -> Result<()> {
    // Get the current timestamp in milliseconds for versioning the settings
    let current_time = Utc::now().timestamp_millis();

    // Get all servers the user is a member of
    let server_members = db.fetch_all_memberships(user_id).await?;

    // Get all direct message channels the user is part of
    let dm_channels = db.find_direct_messages(user_id).await?;

    // Create HashMaps to store server and channel notification settings
    let mut server_settings = HashMap::new();
    let mut channel_settings = HashMap::new();

    // Set notification mode for all servers
    for member in server_members {
        // Get the server ID from the member
        let server_id = member.id.server;
        // Convert the NotificationMode enum to the appropriate string value
        let mode_str = match mode {
            NotificationMode::All => "all",
            NotificationMode::Mute => "muted",
        };
        // Add the server and its notification setting to the HashMap
        server_settings.insert(server_id, mode_str.to_string());
    }

    // Set notification mode for all direct message channels
    for channel in dm_channels {
        // Convert the NotificationMode enum to the appropriate string value
        let mode_str = match mode {
            NotificationMode::All => "all",
            NotificationMode::Mute => "muted",
        };
        // Add the channel and its notification setting to the HashMap
        channel_settings.insert(channel.id().to_string(), mode_str.to_string());
    }

    // Create a JSON object with server and channel notification settings
    let notification_settings = serde_json::json!({
        "server": server_settings,
        "channel": channel_settings
    });

    // Convert the JSON object to a string, handling any serialization errors
    let notification_str =
        serde_json::to_string(&notification_settings).map_err(|_| create_error!(InternalError))?;

    // Create a HashMap for the user settings entry
    let mut settings = HashMap::new();
    // Add the notification settings with the timestamp - using a different key to avoid conflicts
    settings.insert(
        "notification_preferences".to_string(),
        (current_time, notification_str),
    );

    // Update the user settings in the database
    settings.set(db, user_id).await
}

/// # Get Task Status
///
/// Check the status of a background notification update task
#[openapi(tag = "Sync")]
#[get("/settings/notification_task/<task_id>")]
pub async fn check_task_status(
    _db: &State<Database>,
    _user: User,
    task_id: String,
) -> Result<Json<TaskResponse>> {
    // Note: In a production environment, you would query a task tracking system
    // to get the real status of the task. This is a simplified version.

    // Check if the task ID is valid
    if Uuid::parse_str(&task_id).is_ok() {
        Ok(Json(TaskResponse {
            task_id,
            message: "Task is being processed. Check your notification settings in a few minutes."
                .to_string(),
            skip_resync: None,
        }))
    } else {
        // Use a valid error code from ErrorType enum that exists
        Err(create_error!(InvalidProperty))
    }
}
