//! Intègre au jeu les blocs de mouvement de l'éditeur (`assets/editeur/blocs/*.json`) : ajouter un
//! bloc = ajouter un fichier (règle 4 de `ROADMAP-0.12-editeur.md`). Les zips des versions ne
//! contiennent pas de dossier `assets`, d'où l'intégration à la compilation.

use std::io::Write;

fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/editeur/blocs");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "json")).collect())
        .unwrap_or_default();
    files.sort();
    let out = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("blocs.rs");
    let mut f = std::fs::File::create(out).unwrap();
    writeln!(f, "/// Blocs de mouvement intégrés : (fichier, contenu JSON).").unwrap();
    writeln!(f, "pub const BUILTIN_BLOCKS: &[(&str, &str)] = &[").unwrap();
    for p in &files {
        println!("cargo:rerun-if-changed={}", p.display());
        let name = p.file_name().unwrap().to_string_lossy();
        writeln!(f, "    ({:?}, include_str!({:?})),", name, p.display().to_string()).unwrap();
    }
    writeln!(f, "];").unwrap();
}
