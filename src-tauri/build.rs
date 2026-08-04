fn main() {
    for name in [
        "PIXIV_OAUTH_CLIENT_ID",
        "PIXIV_OAUTH_CLIENT_SECRET",
        "PIXIV_OAUTH_HASH_SALT",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
        if let Ok(value) = std::env::var(name) {
            println!("cargo:rustc-env={name}={value}");
        }
    }
    tauri_build::build()
}
