//! Shared application identity used by the daemon and desktop client.

#[cfg(debug_assertions)]
pub const APP_NAME: &str = "Xigon Debug";
#[cfg(not(debug_assertions))]
pub const APP_NAME: &str = "Xigon";

#[cfg(debug_assertions)]
pub const APP_ID: &str = "com.github.itisbryan.xigon.dev";
#[cfg(not(debug_assertions))]
pub const APP_ID: &str = "com.github.itisbryan.xigon";

#[cfg(debug_assertions)]
pub const DATA_DIRECTORY_NAME: &str = "Xigon Debug";
#[cfg(not(debug_assertions))]
pub const DATA_DIRECTORY_NAME: &str = "Xigon";
