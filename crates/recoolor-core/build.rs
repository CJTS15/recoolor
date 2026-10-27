//! Bundles every `palettes/**/*.toml` into the library, so adding a palette is
//! adding its file.

use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("palettes");
    let mut files = Vec::new();
    collect(&root, &mut files);
    files.sort();

    let mut list = String::from("&[\n");
    for file in &files {
        let name = file.file_name().unwrap().to_string_lossy();
        list.push_str(&format!(
            "    ({name:?}, include_str!({:?})),\n",
            file.display().to_string()
        ));
    }
    list.push(']');
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("bundled_palettes.rs");
    fs::write(out, list).unwrap();
}

fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    // Cargo watches a folder for added or removed files, not its subfolders.
    println!("cargo:rerun-if-changed={}", dir.display());
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect(&path, files);
        } else if path.extension().is_some_and(|e| e == "toml") {
            println!("cargo:rerun-if-changed={}", path.display());
            files.push(path);
        }
    }
}
