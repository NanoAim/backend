use std::collections::HashMap;
use std::time::SystemTime;
use serde_json::Value;
use revolt_result::Result;
use log::{info, error, warn};

use crate::Database;

/// Set server notification preference to "all" for a user
pub async fn set_server_notification_to_all(db: &Database, user_id: &str, server_id: &str) -> Result<()> {
    info!("Attempting to set server notification to 'all' for user {} in server {}", user_id, server_id);
    
    // Fetch current notification settings
    let mut settings = match db.fetch_user_settings(
        user_id, 
        &["notifications".to_string()]
    ).await {
        Ok(settings) => {
            info!("Successfully fetched settings for user {}", user_id);
            settings
        },
        Err(err) => {
            warn!("Failed to fetch user settings, defaulting to empty: {}", err);
            HashMap::new()
        }
    };
    
    // Parse the current notification settings or create a new empty structure
    let notifications_value = if let Some((_, json_str)) = settings.get("notifications") {
        info!("Found existing notification settings: {}", json_str);
        match serde_json::from_str::<Value>(json_str) {
            Ok(value) => {
                info!("Successfully parsed notification settings");
                value
            },
            Err(err) => {
                warn!("Failed to parse notification settings JSON, creating new one: {}", err);
                serde_json::json!({
                    "server": {},
                    "channel": {}
                })
            }
        }
    } else {
        info!("No existing notification settings, creating new structure");
        serde_json::json!({
            "server": {},
            "channel": {}
        })
    };
    
    // Create mutable copy to modify
    let mut notifications = notifications_value.clone();
    
    // Ensure the server object exists
    if !notifications.get("server").is_some() {
        info!("Server object doesn't exist in settings, creating it");
        notifications["server"] = serde_json::json!({});
    }
    
    // Set notification preference for this server to "all"
    if let Some(server_obj) = notifications.get_mut("server").and_then(|v| v.as_object_mut()) {
        info!("Setting notification preference for server {} to 'all'", server_id);
        
        // Based on the client example, we set the value as a string "all"
        server_obj.insert(server_id.to_string(), serde_json::json!("all"));
        
        // Save the updated settings
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as i64;
            
        let json_str = match serde_json::to_string(&notifications) {
            Ok(str) => {
                info!("Successfully serialized notification settings: {}", str);
                str
            },
            Err(err) => {
                error!("Failed to serialize notification settings: {}", err);
                return Err(create_error!(InvalidOperation));
            }
        };
        
        let mut new_settings = HashMap::new();
        new_settings.insert("notifications".to_string(), (timestamp, json_str));
        
        match db.set_user_settings(user_id, &new_settings).await {
            Ok(_) => {
                info!("Successfully saved server notification settings for user {} in server {}", user_id, server_id);
            },
            Err(err) => {
                error!("Failed to save server notification settings: {}", err);
                return Err(err);
            }
        };
    } else {
        error!("Failed to get mutable reference to server object");
        return Err(create_error!(InvalidOperation));
    }
    
    info!("Completed setting server notification to 'all' for user {} in server {}", user_id, server_id);
    Ok(())
}

/// Set channel notification preference to "all" for a user
pub async fn set_channel_notification_to_all(db: &Database, user_id: &str, channel_id: &str) -> Result<()> {
    info!("Attempting to set notification to 'all' for user {} in channel {}", user_id, channel_id);
    
    // Fetch current notification settings
    let mut settings = match db.fetch_user_settings(
        user_id, 
        &["notifications".to_string()]
    ).await {
        Ok(settings) => {
            info!("Successfully fetched settings for user {}", user_id);
            settings
        },
        Err(err) => {
            warn!("Failed to fetch user settings, defaulting to empty: {}", err);
            HashMap::new()
        }
    };
    
    // Parse the current notification settings or create a new empty structure
    let notifications_value = if let Some((_, json_str)) = settings.get("notifications") {
        info!("Found existing notification settings: {}", json_str);
        match serde_json::from_str::<Value>(json_str) {
            Ok(value) => {
                info!("Successfully parsed notification settings");
                value
            },
            Err(err) => {
                warn!("Failed to parse notification settings JSON, creating new one: {}", err);
                serde_json::json!({
                    "server": {},
                    "channel": {}
                })
            }
        }
    } else {
        info!("No existing notification settings, creating new structure");
        serde_json::json!({
            "server": {},
            "channel": {}
        })
    };
    
    // Create mutable copy to modify
    let mut notifications = notifications_value.clone();
    
    // Ensure the channel object exists
    if !notifications.get("channel").is_some() {
        info!("Channel object doesn't exist in settings, creating it");
        notifications["channel"] = serde_json::json!({});
    }
    
    // Set notification preference for this channel to "all"
    if let Some(channel_obj) = notifications.get_mut("channel").and_then(|v| v.as_object_mut()) {
        info!("Setting notification preference for channel {} to 'all'", channel_id);
        
        // Based on the client example, we set the value as a string "all"
        channel_obj.insert(channel_id.to_string(), serde_json::json!("all"));
        
        // Save the updated settings
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as i64;
            
        let json_str = match serde_json::to_string(&notifications) {
            Ok(str) => {
                info!("Successfully serialized notification settings: {}", str);
                str
            },
            Err(err) => {
                error!("Failed to serialize notification settings: {}", err);
                return Err(create_error!(InvalidOperation));
            }
        };
        
        let mut new_settings = HashMap::new();
        new_settings.insert("notifications".to_string(), (timestamp, json_str));
        
        match db.set_user_settings(user_id, &new_settings).await {
            Ok(_) => {
                info!("Successfully saved notification settings for user {} in channel {}", user_id, channel_id);
            },
            Err(err) => {
                error!("Failed to save notification settings: {}", err);
                return Err(err);
            }
        };
    } else {
        error!("Failed to get mutable reference to channel object");
        return Err(create_error!(InvalidOperation));
    }
    
    info!("Completed setting notification to 'all' for user {} in channel {}", user_id, channel_id);
    Ok(())
} 