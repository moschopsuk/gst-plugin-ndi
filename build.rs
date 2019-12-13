extern crate gst_plugin_version_helper;

fn main() {
    println!("cargo:libdir=/usr/local/lib");
    println!("cargo:include=/usr/local/include");
    gst_plugin_version_helper::get_info();
}