fn main() {
    println!("cargo:rerun-if-changed=assets");
    embed_resource::compile("assets/backreel.rc", embed_resource::NONE)
        .manifest_required()
        .expect("failed to compile Windows resources");
}
