fn main() {
    // Recompile when migrations change so `sqlx::migrate!` embeds the new files.
    println!("cargo:rerun-if-changed=migrations");
}
