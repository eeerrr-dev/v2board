fn main() {
    // `sqlx::migrate!` embeds the directory at compile time. Cargo does not
    // reliably notice newly added migration files unless the package declares
    // the directory as an explicit build input.
    println!("cargo:rerun-if-changed=../../migrations");
}
