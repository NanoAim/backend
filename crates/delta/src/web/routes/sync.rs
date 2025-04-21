use crate::routes::sync::set_all_notifications::{check_task_status, set_all_notifications};
use crate::routes::sync::set_default_settings::set_default_settings;
use crate::routes::sync::set_settings::set;

pub fn get_routes() -> Vec<rocket::Route> {
    routes![
        settings,
        set,
        set_default_settings,
        set_all_notifications,
        check_task_status
    ]
}
