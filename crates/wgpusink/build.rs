fn main() {
    gst_plugin_version_helper::info();

    // Provide fallback values for env vars used by gst::plugin_define!
    if std::env::var("COMMIT_ID").is_err() {
        println!("cargo:rustc-env=COMMIT_ID=NONE");
    }
    if std::env::var("BUILD_REL_DATE").is_err() {
        println!("cargo:rustc-env=BUILD_REL_DATE=");
    }
}
