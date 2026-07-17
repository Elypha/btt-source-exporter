fn main() {
    println!("cargo::rerun-if-changed=schemas");
    println!("cargo::rerun-if-changed=dialogue-sources.json");
}
