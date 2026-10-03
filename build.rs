//! Intègre au jeu les données de l'éditeur (`assets/editeur/{blocs,anims,races}/*.json`) : ajouter
//! un bloc, une animation ou une race = ajouter un fichier (règle 4 de `ROADMAP-0.12-editeur.md`).
//! Les zips des versions ne contiennent pas de dossier `assets`, d'où l'intégration à la compilation.

use std::io::Write;

fn main() {
    let out = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("donnees.rs");
    let mut f = std::fs::File::create(out).unwrap();
    for (name, dir) in [("BUILTIN_BLOCKS", "blocs"), ("BUILTIN_ANIMS", "anims"), ("BUILTIN_RACES", "races")] {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/editeur").join(dir);
        println!("cargo:rerun-if-changed={}", dir.display());
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
            .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "json")).collect())
            .unwrap_or_default();
        files.sort();
        writeln!(f, "/// Données intégrées : (fichier, contenu JSON).").unwrap();
        writeln!(f, "pub const {name}: &[(&str, &str)] = &[").unwrap();
        for p in &files {
            println!("cargo:rerun-if-changed={}", p.display());
            let file = p.file_name().unwrap().to_string_lossy();
            writeln!(f, "    ({:?}, include_str!({:?})),", file, p.display().to_string()).unwrap();
        }
        writeln!(f, "];").unwrap();
    }
}
