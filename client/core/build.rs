//! Declares the `frb_expand` cfg that `flutter_rust_bridge`'s `#[frb]` attribute emits (set
//! only while the codegen expands the crate).

fn main() {
    println!("cargo::rustc-check-cfg=cfg(frb_expand)");
}
