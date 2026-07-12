fn main() {
    // Keep the integration gate's embedded migrator in lockstep with the
    // runtime migrator when a migration file is added, removed, or edited.
    println!("cargo:rerun-if-changed=../../migrations");
}
