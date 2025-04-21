# Notification Settings Scripts

## Set All Notifications

The `set_all_notifications.rs` script updates all user notification settings to "all" for all servers and channels. This ensures all users receive notifications for all messages.

### Prerequisites

- Rust and Cargo installed
- Access to the MongoDB database where Revolt data is stored

### Usage

1. Navigate to the scripts directory:
```bash
cd scripts
```

2. Update the MongoDB connection string in `set_all_notifications.rs` if needed (default is `mongodb://localhost:27017`).

3. Run the script:
```bash
cargo run --bin set_all_notifications
```

### What the script does

The script:
1. Connects to the MongoDB database
2. Finds all users in the system
3. For each user, collects all server and channel IDs
4. Sets the notification setting to "all" for each server and channel
5. Updates the user settings document in the database

### Output

The script will print progress information as it processes each user and will confirm when all users have been updated. 