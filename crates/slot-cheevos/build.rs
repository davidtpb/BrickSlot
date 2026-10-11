use std::path::Path;

fn main() {
    let root = Path::new("rcheevos");
    let mut build = cc::Build::new();
    build
        .include(root.join("include"))
        .include(root.join("src"))
        .file("shim.c")
        .warnings(false)
        .flag_if_supported("-std=gnu99");
    for dir in ["src", "src/rapi", "src/rcheevos", "src/rhash"] {
        let dir = root.join(dir);
        for entry in std::fs::read_dir(&dir).expect("rcheevos source") {
            let path = entry.expect("rcheevos source").path();
            if path.extension().is_some_and(|e| e == "c") {
                build.file(&path);
            }
        }
    }
    build.compile("rcheevos");
    println!("cargo:rerun-if-changed=shim.c");
    println!("cargo:rerun-if-changed=rcheevos");
}
