// HashMap: A standard Rust data structure that stores key-value pairs with O(1) lookup time
// HashSet: A collection of unique values with O(1) lookup, used for efficient server ID lookups
use std::collections::{HashMap, HashSet};
// Error: A trait for types that can be treated as errors
// Box<dyn Error> allows us to return different error types from our functions
use std::error::Error;
// File: For file operations
// OpenOptions: For configuring file open operations with custom settings
use std::fs::{File, OpenOptions};
// BufRead: Trait that provides buffered reading of lines
// BufReader: Implementation of buffered reader for files
// Write: Trait for writing to files
use std::io::{BufRead, BufReader, Write};
// Path: For working with file system paths
use std::path::Path;
// Arc: Atomic Reference Counting for thread-safe shared ownership
// Mutex: Mutual exclusion primitive for thread safety
use std::sync::{Arc, Mutex};
// Duration: Represents a span of time for delays between batches
// Instant: For high-precision time measurement
use std::time::{Duration, Instant};

// Local: For getting and formatting the local time with timezone
use chrono::Local;
// StreamExt: Extension trait that provides additional methods for asynchronous streams
// stream: Module for working with asynchronous streams of values
use futures::{stream, StreamExt};
// MongoDB driver modules
use mongodb::{
    // bson: Binary JSON format used by MongoDB for data storage
    bson::{
        // doc!: Macro for creating BSON documents with a syntax similar to JSON
        doc, 
        // to_bson: Function to convert Rust types to BSON values
        to_bson, 
        // Document: BSON document type for representing MongoDB records
        Document
    },
    // Options for MongoDB operations
    options::{
        // UpdateOptions: Configuration options for MongoDB update operations
        UpdateOptions, 
        // FindOptions: Configuration options for MongoDB find operations (projections, batch size)
        FindOptions
    },
    // Client: Connection handle to a MongoDB server or cluster
    Client, 
    // Collection: Interface to a MongoDB collection for performing operations
    Collection,
};
// json!: Macro for creating serde_json::Value objects with JSON-like syntax
// This makes it easy to build complex JSON structures in Rust code
use serde_json::json;
// Semaphore: Used to limit the number of concurrent operations
use tokio::sync::Semaphore;
// Progress bar libraries for showing progress in the terminal
// ProgressBar: Shows a visual progress indicator
// ProgressStyle: Customizes the appearance of the progress bar
use indicatif::{ProgressBar, ProgressStyle};

// Configuration constants to control batch sizes and concurrency
// BATCH_SIZE: Number of users to process at once before moving to the next batch
const BATCH_SIZE: usize = 100; // Number of users to process in a batch
// MAX_CONCURRENT_OPERATIONS: Maximum number of parallel user operations to run
const MAX_CONCURRENT_OPERATIONS: usize = 10; // Maximum number of concurrent operations
// DELAY_BETWEEN_BATCHES_MS: Milliseconds to wait between processing batches to reduce database load
const DELAY_BETWEEN_BATCHES_MS: u64 = 500; // Delay between batches to reduce DB pressure
// PATH to store progress information for recovery
const PROGRESS_FILE: &str = "notification_update_progress.txt";
// LOG_FILE for storing execution logs
const LOG_FILE: &str = "notification_update.log";

// Shared statistics for logging
// This structure keeps track of all metrics during execution
struct Statistics {
    // Total number of users to process
    total_users: u64,
    // Number of users processed so far (including skipped ones)
    processed_users: u64,
    // Number of users skipped (already processed in previous runs)
    skipped_users: u64,
    // Number of users successfully processed
    successful_users: u64,
    // Number of users that failed to process
    failed_users: u64,
    // When the process started (for calculating elapsed time)
    start_time: Instant,
    // When the last status log was printed (to avoid too frequent logging)
    last_log_time: Instant,
    // When the current batch started processing (for batch timing)
    batch_start_time: Instant,
    // How long the previous batch took to process
    last_batch_duration: Duration,
    // A list of the recently processed user IDs (for monitoring)
    last_processed_users: Vec<String>,
}

// Implementation of methods for the Statistics struct
impl Statistics {
    // Create a new Statistics instance with initial values
    fn new(total_users: u64, previously_processed: usize) -> Self {
        // Get current time for initialization
        let now = Instant::now();
        // Return initialized Statistics
        Statistics {
            total_users,
            processed_users: previously_processed as u64,
            skipped_users: previously_processed as u64,
            successful_users: 0,
            failed_users: 0,
            start_time: now,
            last_log_time: now,
            batch_start_time: now,
            last_batch_duration: Duration::from_secs(0),
            // Keep a limited list of most recent users for logging
            last_processed_users: Vec::with_capacity(10), // Keep last 10 processed users
        }
    }
    
    // Record that a user has been processed
    fn user_processed(&mut self, user_id: &str, success: bool) {
        // Increment total processed count
        self.processed_users += 1;
        // Handle successful processing
        if success {
            // Increment successful count
            self.successful_users += 1;
            // Add to list of recently processed users
            self.last_processed_users.push(user_id.to_string());
            // Keep only the 10 most recent users
            if self.last_processed_users.len() > 10 {
                // Remove oldest entry (at index 0)
                self.last_processed_users.remove(0);
            }
        } else {
            // Increment failure count
            self.failed_users += 1;
        }
    }
    
    // Record completion of a batch
    fn batch_completed(&mut self, duration: Duration) {
        // Store the duration for logging
        self.last_batch_duration = duration;
    }
    
    // Log current status if enough time has passed
    fn log_status(&mut self, logger: &Logger) -> bool {
        // Get current time
        let now = Instant::now();
        // Calculate time since last log
        let elapsed = now.duration_since(self.last_log_time);
        
        // Log every 5 seconds or when explicitly requested
        if elapsed.as_secs() >= 5 {
            // Update last log time
            self.last_log_time = now;
            
            // Calculate progress metrics
            // Total time elapsed since start
            let total_elapsed = now.duration_since(self.start_time);
            // Percentage of completion
            let percent_complete = (self.processed_users as f64 / self.total_users as f64) * 100.0;
            // Processing rate (users per second)
            let users_per_second = if total_elapsed.as_secs() > 0 {
                self.processed_users as f64 / total_elapsed.as_secs() as f64
            } else {
                0.0
            };
            
            // Calculate estimated time remaining
            // Users still to process
            let remaining_users = self.total_users - self.processed_users;
            // Estimated seconds to complete remaining users
            let estimated_seconds_left = if users_per_second > 0.0 {
                remaining_users as f64 / users_per_second
            } else {
                0.0
            };
            
            // Convert estimated seconds to hours, minutes, seconds
            let hours_left = (estimated_seconds_left / 3600.0).floor();
            let minutes_left = ((estimated_seconds_left % 3600.0) / 60.0).floor();
            let seconds_left = (estimated_seconds_left % 60.0).floor();
            
            // Format last processed users for log
            // Join user IDs with commas or show "None" if empty
            let last_users = if self.last_processed_users.is_empty() {
                "None".to_string()
            } else {
                self.last_processed_users.join(", ")
            };
            
            // Log formatted status information
            logger.log(&format!(
                "PROGRESS: {:.2}% complete ({}/{})\n\
                 STATISTICS: Processed: {}, Skipped: {}, Successful: {}, Failed: {}\n\
                 PERFORMANCE: {:.2} users/sec, Last batch: {:.2} sec\n\
                 TIME: Elapsed: {:.2} sec, Estimated remaining: {:.0}h {:.0}m {:.0}s\n\
                 LAST USERS: {}",
                percent_complete, self.processed_users, self.total_users,
                self.processed_users, self.skipped_users, self.successful_users, self.failed_users,
                users_per_second, self.last_batch_duration.as_secs_f64(),
                total_elapsed.as_secs_f64(), hours_left, minutes_left, seconds_left,
                last_users
            ));
            
            // Return true to indicate log was written
            return true;
        }
        
        // Return false if no log was written
        false
    }
}

// Logger for both console and file output
// Handles writing logs to both terminal and log file
struct Logger {
    // File handle for the log file, wrapped in thread-safe primitives
    log_file: Arc<Mutex<File>>,
}

// Implementation of methods for the Logger struct
impl Logger {
    // Create a new Logger instance
    fn new() -> Result<Self, Box<dyn Error>> {
        // Open log file with timestamp in append mode
        // This creates the file if it doesn't exist and appends to it if it does
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(LOG_FILE)?;
            
        // Return initialized Logger
        Ok(Logger {
            // Wrap file in Arc and Mutex for thread safety
            log_file: Arc::new(Mutex::new(log_file)),
        })
    }
    
    // Log a message to both console and file
    fn log(&self, message: &str) {
        // Get current time for log entry
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        // Format log entry with timestamp
        let log_entry = format!("[{}] {}\n\n", now, message);
        
        // Print to console
        println!("{}", log_entry);
        
        // Write to log file
        if let Ok(mut file) = self.log_file.lock() {
            // Ignore errors when writing to log file
            let _ = file.write_all(log_entry.as_bytes());
        }
    }
}

// Load previously processed user IDs from file for recovery
fn load_processed_users() -> HashSet<String> {
    // Create an empty HashSet to store the IDs
    let mut processed = HashSet::new();
    
    // If the progress file exists, load its contents
    if Path::new(PROGRESS_FILE).exists() {
        // Open the file for reading
        if let Ok(file) = File::open(PROGRESS_FILE) {
            // Create a buffered reader for efficient line reading
            let reader = BufReader::new(file);
            // Read each line (each line is one user ID)
            for line in reader.lines() {
                // If the line can be read successfully
                if let Ok(user_id) = line {
                    // Add the user ID to the set
                    processed.insert(user_id);
                }
            }
        }
    }
    
    // Return the set of processed user IDs
    processed
}

// Process a batch of users concurrently
// This function takes a batch of user IDs and processes them in parallel
async fn process_user_batch(
    // Reference to the database
    db: &mongodb::Database,
    // Collection for user settings
    user_settings: &Collection<Document>,
    // Collection for server memberships
    server_members: &Collection<Document>,
    // Collection for channels
    channels: &Collection<Document>,
    // Vector of user IDs to process
    user_ids: Vec<String>,
    // Timestamp for all updates
    timestamp: i64,
    // Semaphore to limit concurrency
    semaphore: Arc<Semaphore>,
    // Progress bar for visual feedback
    progress_bar: &ProgressBar,
    // File handle for recording progress
    progress_file: Arc<File>,
    // Statistics for logging
    stats: Arc<Mutex<Statistics>>,
    // Logger for logging
    logger: &Logger,
) -> Result<(), Box<dyn Error>> {
    // Log the user IDs being processed in this batch
    // If there are 5 or fewer users, show all IDs
    let user_ids_str = if user_ids.len() <= 5 {
        // Join all user IDs with commas
        user_ids.join(", ")
    } else {
        // Show first two and count of others
        format!("{}, {} and {} others", 
            user_ids[0], user_ids[1], user_ids.len() - 2)
    };
    // Log the users being processed
    logger.log(&format!("Processing users: {}", user_ids_str));
    
    // Process users concurrently with limited parallelism
    let results = stream::iter(user_ids)
        .map(|user_id| {
            // Clone references to collections for use in the async task
            let user_settings = user_settings.clone();
            let server_members = server_members.clone();
            let channels = channels.clone();
            let db = db.clone();
            let semaphore = Arc::clone(&semaphore);
            let progress_bar = progress_bar.clone();
            let progress_file = Arc::clone(&progress_file);
            let stats = Arc::clone(&stats);
            let user_id_clone = user_id.clone();
            
            // Create an async move block for this user
            async move {
                // Acquire a permit from the semaphore
                // This ensures we don't exceed MAX_CONCURRENT_OPERATIONS
                let _permit = semaphore.acquire().await.expect("Failed to acquire semaphore");
                
                // Process the user and update their settings
                let result = process_single_user(
                    &db,
                    &user_settings,
                    &server_members,
                    &channels,
                    &user_id,
                    timestamp
                ).await;
                
                // Record success or failure in stats
                if let Ok(mut stats) = stats.lock() {
                    // Update statistics with result
                    stats.user_processed(&user_id, result.is_ok());
                    // Log progress periodically
                    stats.log_status(&Logger::new().unwrap_or_else(|_| {
                        // Fall back to simpler logging if creating logger fails
                        struct SimpleLogger;
                        impl SimpleLogger {
                            fn log(&self, msg: &str) {
                                println!("{}", msg);
                            }
                        }
                        Logger { log_file: Arc::new(Mutex::new(File::create("/dev/null").unwrap())) }
                    }));
                }
                
                // Record progress for recovery if successful
                if result.is_ok() {
                    // Write user ID to progress file
                    let mut file = progress_file.as_ref();
                    if writeln!(file, "{}", user_id_clone).is_err() {
                        eprintln!("Warning: Failed to write progress for user {}", user_id_clone);
                    }
                }
                
                // Update progress bar
                progress_bar.inc(1);
                
                // Return result with user ID for error reporting
                result.map_err(|e| {
                    format!("Error processing user {}: {}", user_id_clone, e).into()
                })
            }
        })
        // Allow up to MAX_CONCURRENT_OPERATIONS to run at once
        .buffer_unordered(MAX_CONCURRENT_OPERATIONS)
        // Collect all results
        .collect::<Vec<Result<(), Box<dyn Error>>>>()
        .await;
    
    // Check for and log errors
    let mut error_count = 0;
    for result in results {
        if let Err(e) = result {
            // Count errors
            error_count += 1;
            // Log the error message
            eprintln!("{}", e);
        }
    }
    
    // Report error summary if any
    if error_count > 0 {
        // Log the number of failures
        logger.log(&format!("WARNING: {} users in this batch failed to process", error_count));
    }
    
    // Return success for the batch
    Ok(())
}

// Process a single user's notification settings
// This function handles the notification settings for one specific user
async fn process_single_user(
    _db: &mongodb::Database,
    user_settings: &Collection<Document>,
    server_members: &Collection<Document>,
    channels: &Collection<Document>,
    user_id: &str,
    timestamp: i64,
) -> Result<(), Box<dyn Error>> {
    // Start timing this user's processing
    let start_time = Instant::now();
    
    // Find all servers the user is a member of using a more robust query
    let pipeline = vec![
        doc! {
            "$match": {
                "$or": [
                    { "_id.user": user_id },
                    { "user": user_id }
                ]
            }
        }
    ];
    
    // Add try/catch block to handle aggregation errors
    let member_docs = match server_members.aggregate(pipeline.clone(), None).await {
        Ok(cursor) => cursor.collect::<Vec<Result<Document, mongodb::error::Error>>>().await,
        Err(e) => {
            // Log the error but continue with empty results
            println!("DEBUG: Error in main aggregation query: {}", e);
            Vec::new()
        }
    };
    
    // Debug - inspect first document returned to verify structure
    if !member_docs.is_empty() {
        if let Ok(doc) = &member_docs[0] {
            println!("DEBUG: Sample server_member document: {}", doc);
            println!("DEBUG: Document keys: {:?}", doc.keys().collect::<Vec<_>>());
            
            // Check _id object
            if let Some(id_obj) = doc.get("_id").and_then(|id| id.as_document()) {
                println!("DEBUG: _id object keys: {:?}", id_obj.keys().collect::<Vec<_>>());
                
                // Check server field in _id
                if let Some(server_id) = id_obj.get("server") {
                    println!("DEBUG: _id.server field type: {:?}, value: {}", server_id.element_type(), server_id);
                } else {
                    println!("DEBUG: server field not found in _id object");
                }
                
                // Check user field in _id
                if let Some(user_id_val) = id_obj.get("user") {
                    println!("DEBUG: _id.user field type: {:?}, value: {}", user_id_val.element_type(), user_id_val);
                } else {
                    println!("DEBUG: user field not found in _id object");
                }
            } else {
                println!("DEBUG: _id is not an object or is missing");
            }
            
            // Check for direct server field (fallback)
            if let Some(server_id) = doc.get("server") {
                println!("DEBUG: direct server field type: {:?}, value: {}", server_id.element_type(), server_id);
            } else {
                println!("DEBUG: direct server field not found in document");
            }
        }
    } else {
        // Try alternative approaches to find server_members
        println!("DEBUG: No server_member document found for user {}", user_id);
        
        let direct_user_count = server_members.count_documents(doc! { "user": user_id }, None).await?;
        println!("DEBUG: Documents with direct user field: {}", direct_user_count);
        
        // Try a regex approach with error handling
        let regex_pipeline = vec![
            doc! {
                "$match": {
                    "$expr": {
                        "$regexMatch": {
                            "input": { 
                                "$convert": {
                                    "input": "$_id",
                                    "to": "string",
                                    "onError": ""  // Return empty string on conversion error
                                }
                            },
                            "regex": user_id
                        }
                    }
                }
            }
        ];
        
        // Safely handle potential errors in regex aggregation
        let regex_docs = match server_members.aggregate(regex_pipeline, None).await {
            Ok(cursor) => cursor.collect::<Vec<Result<Document, mongodb::error::Error>>>().await,
            Err(e) => {
                // Log the error but continue with empty results
                println!("DEBUG: Error in regex aggregation: {}", e);
                Vec::new()
            }
        };
        
        println!("DEBUG: Documents matching user ID with regex: {}", regex_docs.len());
        
        if !regex_docs.is_empty() {
            for (i, doc_result) in regex_docs.iter().enumerate().take(3) { // Limit to first 3 for brevity
                if let Ok(doc) = doc_result {
                    println!("DEBUG: Found document {}: {}", i, doc);
                }
            }
        }
    }
    
    // Extract server IDs into a HashSet for efficient lookups
    let mut server_ids = HashSet::new();
    
    for doc_result in &member_docs {
        if let Ok(doc) = doc_result {
            // Try to get the _id object first
            if let Some(id_obj) = doc.get("_id").and_then(|id| id.as_document()) {
                // Then get the server field from within _id
                if let Some(server_id) = id_obj.get("server").and_then(|id| id.as_str()) {
                    server_ids.insert(server_id.to_string());
                    continue;
                }
            }
            
            // Fallback to traditional server field
            if let Some(server_id) = doc.get("server").and_then(|id| id.as_str()) {
                server_ids.insert(server_id.to_string());
            }
        }
    }
    
    // Log number of servers found - DEBUG
    println!("DEBUG: User {} is a member of {} servers", user_id, server_ids.len());
    if !server_ids.is_empty() {
        println!("DEBUG: First server ID: {}", server_ids.iter().next().unwrap());
    }
    
    // Create HashMap for server notification settings
    let mut server_settings = HashMap::new();
    // Add all server IDs with "all" notification setting
    for server_id in &server_ids {
        server_settings.insert(server_id.clone(), "all");
    }
    
    // Log size of server_settings - DEBUG
    println!("DEBUG: Server settings count: {}", server_settings.len());
    
    // Setup options for channel queries
    let personal_channel_options = FindOptions::builder()
        // Only retrieve _id and channel_type fields
        .projection(doc! { "_id": 1, "channel_type": 1 })
        .build();
    
    // Query for personal channels (DMs and Groups)
    // This combines two queries into one for better performance
    let personal_channel_cursor = channels.find(
        // Use $or operator to find channels of either type
        doc! {
            "$or": [
                { "channel_type": "DirectMessage", "recipients": user_id },
                { "channel_type": "Group", "recipients": user_id }
            ]
        },
        personal_channel_options
    ).await?;
    
    // Query for server channels if user is in any servers
    let mut server_channel_cursor = None;
    // Only run this query if the user is in at least one server
    if !server_ids.is_empty() {
        // Convert HashSet to Vec for use in MongoDB $in operator
        let server_id_array: Vec<&String> = server_ids.iter().collect();
        // Set up options to only return channel IDs
        let text_channel_options = FindOptions::builder()
            .projection(doc! { "_id": 1 })
            .build();
            
        // Find all TextChannel channels in servers where user is a member
        // Use $in operator to find channels in any of the user's servers
        let cursor = channels.find(
            doc! {
                "channel_type": "TextChannel",
                "server": { "$in": server_id_array }
            },
            text_channel_options
        ).await?;
        server_channel_cursor = Some(cursor);
    }
    
    // Collect channel settings into HashMap
    let mut channel_settings = HashMap::new();
    
    // Process personal channels
    // First collect all personal channels into a vector
    let personal_channels: Vec<Result<Document, mongodb::error::Error>> = personal_channel_cursor.collect().await;
    // Then iterate through them
    for channel_result in personal_channels {
        // Handle successful document retrieval
        if let Ok(channel) = channel_result {
            // Extract channel ID
            if let Some(channel_id) = channel.get("_id").and_then(|id| id.as_str()) {
                // Add channel ID with "all" notification setting
                channel_settings.insert(channel_id.to_string(), "all");
            }
        }
    }
    
    // Process server channels if any
    // Only process if user is a member of at least one server
    if let Some(cursor) = server_channel_cursor {
        // Collect all server channels into a vector
        let server_channels: Vec<Result<Document, mongodb::error::Error>> = cursor.collect().await;
        // Iterate through server channels
        for channel_result in server_channels {
            // Handle successful document retrieval
            if let Ok(channel) = channel_result {
                // Extract channel ID
                if let Some(channel_id) = channel.get("_id").and_then(|id| id.as_str()) {
                    // Add channel ID with "all" notification setting
                    channel_settings.insert(channel_id.to_string(), "all");
                }
            }
        }
    }
    
    // Create final notification settings JSON object
    let final_notifications = json!({
        "server": server_settings,
        "channel": channel_settings
    });
    
    // Log the final notifications object - DEBUG
    println!("DEBUG: Final notifications: {}", final_notifications);
    
    // Convert to JSON string for storage
    let notification_str = serde_json::to_string(&final_notifications)?;
    
    // Create document to save in MongoDB
    let mut set = doc! {};
    // Insert notification settings as an array with timestamp and settings string
    set.insert(
        "notifications",
        vec![to_bson(&timestamp)?, to_bson(&notification_str)?],
    );
    
    // Debug log to show the document being saved
    println!("DEBUG: Saving settings document: {}", set);
    
    // Update user settings in the database
    let update_result = user_settings
        .update_one(
            // Find user by ID
            doc! { "_id": user_id },
            // Set the notifications field
            doc! { "$set": set },
            // Create document if it doesn't exist
            UpdateOptions::builder().upsert(true).build(),
        )
        .await?;
        
    // Log update result
    println!("DEBUG: Update result - matched: {}, modified: {}, upserted: {}", 
        update_result.matched_count, 
        update_result.modified_count,
        update_result.upserted_id.is_some());
    
    // Record processing stats and log if operation was slow
    let duration = start_time.elapsed();
    // Only log operations that took longer than 500ms as they may indicate issues
    if duration.as_millis() > 500 {
        // Log slow operation with user ID for investigation
        eprintln!("SLOW OPERATION: User {} took {}ms to process", 
            user_id, duration.as_millis());
    }
    
    // Return success
    Ok(())
}

// Main function with Tokio runtime to handle async operations
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logger
    let logger = Logger::new()?;
    
    // Log start message with timestamp
    let start_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    logger.log(&format!("Starting notification settings update at {}", start_time));
    
    // Check if recovery file exists and log accordingly
    if Path::new(PROGRESS_FILE).exists() {
        logger.log("Recovery file found. Will resume from last saved point.");
    }
    
    // Log configuration details
    logger.log(&format!(
        "CONFIGURATION:\n\
         Batch size: {}\n\
         Max concurrent operations: {}\n\
         Delay between batches: {}ms",
        BATCH_SIZE, MAX_CONCURRENT_OPERATIONS, DELAY_BETWEEN_BATCHES_MS
    ));
    
    // Set up signal handling for graceful shutdown
    setup_signal_handler();
    
    // Start timer for measuring total execution time
    let start_time = std::time::Instant::now();
    
    // Run main process with error handling
    match set_all_notifications_to_all().await {
        Ok(_) => {
            // Success case
            let duration = start_time.elapsed();
            // Log success with total execution time
            logger.log(&format!("✅ Process completed successfully in {:.2} seconds", duration.as_secs_f64()));
            Ok(())
        },
        Err(e) => {
            // Error case
            let duration = start_time.elapsed();
            // Log failure with error details
            logger.log(&format!("❌ Process failed after {:.2} seconds: {}", duration.as_secs_f64(), e));
            Err(e)
        }
    }
}

// Setup signal handling for graceful shutdown
// This allows the script to handle Ctrl+C gracefully
fn setup_signal_handler() {
    // Setup handler for CTRL+C
    ctrlc::set_handler(move || {
        // Create a logger for output
        let logger = Logger::new().unwrap_or_else(|_| {
            // Fall back to simpler logging if creating logger fails
            struct SimpleLogger;
            impl SimpleLogger {
                fn log(&self, msg: &str) {
                    println!("{}", msg);
                }
            }
            Logger { log_file: Arc::new(Mutex::new(File::create("/dev/null").unwrap())) }
        });
        
        // Log interruption message
        logger.log("\n⚠️ Operation interrupted. Progress has been saved and can be resumed later.");
        logger.log("Run the script again to continue from where it left off.");
        // Exit cleanly
        std::process::exit(0);
    }).expect("Error setting Ctrl-C handler");
}

// Main function that connects to MongoDB and updates all user notification settings to "all"
async fn set_all_notifications_to_all() -> Result<(), Box<dyn Error>> {
    // Initialize logger
    let logger = Logger::new()?;
    logger.log("Starting notification settings update process");
    
    // MongoDB connection string for local development
    // Replace with your actual MongoDB connection string when needed
    let mongodb_uri = "mongodb://localhost:27017";
    
    // Output a message indicating we're connecting to the database
    logger.log("Connecting to MongoDB...");
    // Create a MongoDB client using the connection string
    let client = Client::with_uri_str(mongodb_uri).await?;
    
    // Access the 'revolt' database
    let db = client.database("revolt");
    
    // Get a reference to the required collections
    // Collection for user notification settings
    let user_settings: Collection<Document> = db.collection("user_settings");
    // Collection for all users
    let users: Collection<Document> = db.collection("users");
    // Collection for server memberships
    let server_members: Collection<Document> = db.collection("server_members");
    // Collection for all channels
    let channels: Collection<Document> = db.collection("channels");
    
    // Count total users for progress reporting
    // This helps us show accurate progress information
    let total_users = users.count_documents(doc! {}, None).await?;
    logger.log(&format!("Total users to process: {}", total_users));
    
    // Load previously processed user IDs if recovery file exists
    let processed_users = load_processed_users();
    logger.log(&format!("{} users have been processed previously and will be skipped", processed_users.len()));
    
    // Setup progress bar for visual feedback
    // This shows a progress bar in the terminal with time estimates
    // Adjust total by subtracting already processed users
    let progress_bar = ProgressBar::new(total_users - processed_users.len() as u64);
    // Set a custom style for the progress bar
    progress_bar.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} users ({eta})")
            .expect("Failed to set progress bar style")
    );
    
    // Current timestamp for all updates
    // All users will get the same timestamp for their notification update
    let timestamp = chrono::Utc::now().timestamp_millis();
    
    // Find options to limit fields returned and improve performance
    // Only fetch the _id field to minimize data transfer
    let user_options = FindOptions::builder()
        // Only retrieve _id field from user documents to save bandwidth
        .projection(doc! { "_id": 1 })
        // Set MongoDB driver's batch size (different from our application batch size)
        .batch_size(BATCH_SIZE as u32)
        .build();
    
    // Get cursor to all users with minimal projection
    // This cursor will let us iterate through all users
    let mut user_cursor = users.find(doc! {}, user_options).await?;
    
    // Create a semaphore to limit concurrent operations
    // This prevents overwhelming the database with too many simultaneous requests
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_OPERATIONS));
    
    // Process users in batches to reduce memory consumption
    // Track how many batches we've processed
    let mut batch_count = 0;
    // Create a vector to hold user IDs for the current batch
    let mut user_batch = Vec::with_capacity(BATCH_SIZE);
    
    // Create a file handle for appending processed users for recovery
    let progress_file = Arc::new(
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(PROGRESS_FILE)?
    );
    
    // Initialize statistics tracking
    let stats = Arc::new(Mutex::new(Statistics::new(total_users, processed_users.len())));
    
    // Iterate through all users in the database
    while let Some(user_result) = user_cursor.next().await {
        // Check if we successfully got a user document
        if let Ok(user) = user_result {
            // Extract the user ID from the document
            if let Some(user_id) = user.get("_id").and_then(|id| id.as_str()) {
                // Skip this user if already processed in a previous run
                if processed_users.contains(user_id) {
                    continue;
                }
                
                // Add this user ID to the current batch
                user_batch.push(user_id.to_string());
                
                // When batch is full, process it
                if user_batch.len() >= BATCH_SIZE {
                    // Increment batch counter for logging
                    batch_count += 1;
                    // Log which batch we're processing
                    logger.log(&format!("Processing batch {} with {} users", batch_count, user_batch.len()));
                    
                    // Update batch start time for stats
                    if let Ok(mut stats) = stats.lock() {
                        stats.batch_start_time = Instant::now();
                    }
                    
                    // Process the current batch of users
                    // Pass all necessary collections and the semaphore
                    process_user_batch(
                        &db, 
                        &user_settings, 
                        &server_members, 
                        &channels, 
                        user_batch.clone(), 
                        timestamp, 
                        Arc::clone(&semaphore),
                        &progress_bar,
                        Arc::clone(&progress_file),
                        Arc::clone(&stats),
                        &logger
                    ).await?;
                    
                    // Log batch completion time
                    let batch_duration = if let Ok(mut stats) = stats.lock() {
                        let duration = stats.batch_start_time.elapsed();
                        stats.batch_completed(duration);
                        duration
                    } else {
                        Duration::from_secs(0)
                    };
                    
                    logger.log(&format!(
                        "Completed batch {} in {:.2} seconds ({:.2} users/sec)", 
                        batch_count, 
                        batch_duration.as_secs_f64(),
                        user_batch.len() as f64 / batch_duration.as_secs_f64().max(0.001)
                    ));
                    
                    // Clear batch for next iteration
                    user_batch.clear();
                    
                    // Add a small delay between batches to reduce database pressure
                    // This prevents database overload
                    tokio::time::sleep(Duration::from_millis(DELAY_BETWEEN_BATCHES_MS)).await;
                }
            }
        }
    }
    
    // Process remaining users in the last batch
    // The last batch might not be full-sized
    if !user_batch.is_empty() {
        // Log that we're processing the final batch
        logger.log(&format!("Processing final batch with {} users", user_batch.len()));
        // Update batch start time for stats
        if let Ok(mut stats) = stats.lock() {
            stats.batch_start_time = Instant::now();
        }
        // Process this last batch of users
        process_user_batch(
            &db, 
            &user_settings, 
            &server_members, 
            &channels, 
            user_batch, 
            timestamp, 
            semaphore,
            &progress_bar,
            progress_file,
            Arc::clone(&stats),
            &logger
        ).await?;
        
        // Log final batch completion
        if let Ok(stats) = stats.lock() {
            let duration = stats.batch_start_time.elapsed();
            logger.log(&format!("Completed final batch in {:.2} seconds", duration.as_secs_f64()));
        }
    }
    
    // Complete the progress bar with a final message
    progress_bar.finish_with_message("All user notification settings have been updated to 'all'");
    
    // Final statistics
    if let Ok(stats) = stats.lock() {
        let total_duration = stats.start_time.elapsed();
        logger.log(&format!(
            "FINAL STATISTICS\n\
             Processed: {}/{} users ({:.2}%)\n\
             Results: {} successful, {} failed, {} skipped\n\
             Performance: {:.2} users/sec overall\n\
             Total time: {:.2} seconds",
            stats.processed_users, stats.total_users,
            (stats.processed_users as f64 / stats.total_users as f64) * 100.0,
            stats.successful_users, stats.failed_users, stats.skipped_users,
            stats.processed_users as f64 / total_duration.as_secs_f64().max(0.001),
            total_duration.as_secs_f64()
        ));
    }
    
    // When completed successfully, remove the progress file
    if Path::new(PROGRESS_FILE).exists() {
        std::fs::remove_file(PROGRESS_FILE)?;
        logger.log("Process completed successfully, removed recovery file.");
    }
    
    // Return success
    Ok(())
} 