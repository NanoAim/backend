// Import the OpenAPI structure from the revolt_rocket_okapi crate to enable API documentation
use revolt_rocket_okapi::revolt_okapi::openapi3::OpenApi;
// Import the Route struct from rocket for defining API routes
use rocket::Route;

// Include the get_settings module for fetching user settings
mod get_settings;
// Include the get_unreads module for fetching unread messages
mod get_unreads;
// Include our custom set_all_notifications module for bulk notification settings
mod set_all_notifications;
// Include the set_settings module for updating user settings
mod set_settings;

// Define a function that returns the routes and OpenAPI documentation
// This function is used by the main application to mount these routes
pub fn routes() -> (Vec<Route>, OpenApi) {
    // Use the openapi_get_routes_spec macro to generate routes and documentation
    // This macro is imported in the main.rs file with #[macro_use]
    openapi_get_routes_spec![
        // Include the fetch route from get_settings module
        get_settings::fetch,
        // Include the set route from set_settings module
        set_settings::set,
        // Include the unreads route from get_unreads module
        get_unreads::unreads,
        // Include our new set_all_notifications route
        set_all_notifications::set_all_notifications,
        // Include our new check_task_status route
        set_all_notifications::check_task_status
    ]
}
