pub mod connection;
pub mod messages;
pub mod model;

pub fn get_socket_path() -> std::path::PathBuf {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").expect("XDG_RUNTIME_DIR is not set");

    std::path::PathBuf::from(runtime_dir).join("clipstash.sock")
}
