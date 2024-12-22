use std::env;

pub fn get_env_variable() -> Option<String> {
    // return Some("test".to_string());
    // return Some("production".to_string());
    match option_env!("STAGE") {
        Some(x) => return Some(x.to_string()),
        None => return Some("production".to_string()),
    };
}

pub fn get_debug_variable() -> Option<bool> {
    // return Some(true);
    return Some(false);
}
