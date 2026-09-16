#!/usr/bin/env python3
"""
FINAL: Extract all key variables/constants/values from Rust files.
Output: a .txt file with value, very short description, and file location.
"""

import re
from pathlib import Path

ROOT = Path(".")

# All Rust source files in the project
RS_FILES = sorted(ROOT.rglob("*.rs"))

# Short descriptions for common variable names
DESC_MAP = {
    "orbit_distance": "distance orbite",
    "orbit_speed": "vitesse orbite",
    "radius": "rayon",
    "seed": "graine",
    "temperature": "température",
    "mouse_sensitivity": "sensibilité souris",
    "scroll_speed": "vitesse zoom",
    "keyboard_speed": "vitesse clavier",
    "invert_y": "inverser Y",
    "show_light_indicator": "afficher éclairage",
    "show_orbits": "afficher orbites",
    "planet_chunk_divisions": "divisions planète",
    "cloud_density": "densité nuages",
    "cloud_altitude": "altitude nuages",
    "cloud_speed": "vitesse nuages",
    "flare_count": "nombre éruptions",
    "flare_height": "hauteur éruptions",
    "flare_speed": "vitesse éruptions",
    "flare_size": "taille éruptions",
    "flare_distance": "distance éruptions",
    "belt_distance": "distance ceinture",
    "belt_width": "écartement ceinture",
    "belt_min_size": "taille min ceinture",
    "belt_max_size": "taille max ceinture",
    "belt_count": "nombre ceintures",
    "moon_orbit_distance": "distance orbite lune",
    "moon_radius": "rayon lune",
    "moon_seed": "graine lune",
    "event_horizon": "horizon événements",
    "influence_radius": "rayon influence",
    "angular_force": "force angulaire",
    "max_velocity": "vitesse max",
    "spaghetti_radius": "rayon spaghetti",
    "spaghetti_max_scale": "échelle max spaghetti",
    "spaghetti_speed": "vitesse spaghetti",
    "core_voxel_size": "taille voxel noyau",
    "photon_ring_emissive": "émissivité anneau photon",
    "photon_ring_radius": "rayon anneau photon",
    "photon_ring_count": "nombre anneaux photon",
    "photon_ring_width": "largeur anneau photon",
    "accretion_inner": "accretion interne",
    "jet_length": "longueur jet",
    "jet_speed": "vitesse jet",
    "jet_width": "largeur jet",
    "jet_emissive": "émissivité jet",
    "perihelion": "périhélie",
    "aphelion": "aphélie",
    "period_initial": "période initiale",
    "sn_type": "type supernova",
    "composition": "composition",
}

UNIMPORTANT = {
    'pub', 'fn', 'struct', 'enum', 'impl', 'mod', 'use', 'let', 'mut', 
    'ref', 'async', 'await', 'match', 'if', 'else', 'for', 'while', 
    'loop', 'return', 'break', 'continue', 'type', 'const', 'static',
    'macro', 'trait', 'where', 'as', 'in', 'move', 'dyn', 'Box', 'Rc', 
    'Arc', 'Vec', 'HashMap', 'BTreeMap', 'Option', 'Result', 'String', 
    'str', 'bool', 'char', 'u8', 'u16', 'u32', 'u64', 'u128',
    'i8', 'i16', 'i32', 'i64', 'i128', 'f32', 'f64', 'usize', 'isize',
    'self', 'Super', 'And', 'Or', 'Not'
}

def is_meaningful(name):
    return name.lower() not in UNIMPORTANT

def short_desc(name, value):
    """Very short description based on variable name."""
    name_lower = name.lower()
    for key, desc in DESC_MAP.items():
        if key in name_lower:
            return desc
    return None

def extract_key_values(filepath):
    """Extract important key values from a single Rust file."""
    results = []
    try:
        with open(filepath, 'r', encoding='utf-8') as f:
            content = f.read()
    except Exception:
        return results
    
    lines = content.split('\n')
    
    for line_num, line in enumerate(lines, 1):
        stripped = line.strip()
        if not stripped or stripped.startswith('//'):
            continue
        if '=' not in stripped:
            continue
        
        # 1. const NAME: TYPE = VALUE;
        m = re.match(r'^const\s+(\w+)\s*:\s*[^=]+\s*=\s*([^;]+);', stripped)
        if m:
            name, value = m.group(1), m.group(2).strip()
            if is_meaningful(name):
                desc = short_desc(name, value)
                results.append({
                    'file': filepath.as_posix(),
                    'line': line_num,
                    'type': 'const',
                    'name': name,
                    'value': value,
                    'description': desc,
                })
            continue
        
        # 2. pub field: TYPE = VALUE, (config struct fields)
        m = re.match(r'pub\s+(\w+)\s*:\s*[\w<>\[\]]+\s*=\s*([^,\n]+)', stripped)
        if m:
            name, value = m.group(1), m.group(2).strip()
            if is_meaningful(name) and value and not value.startswith('&'):
                desc = short_desc(name, value)
                results.append({
                    'file': filepath.as_posix(),
                    'line': line_num,
                    'type': 'config_field',
                    'name': name,
                    'value': value,
                    'description': desc,
                })
            continue
        
        # 3. Self { field: VALUE, ... } (default implementations)
        m = re.search(r'Self\s*\{\s*([^}]+)\s*\}', stripped)
        if m:
            content_str = m.group(1)
            for fm in re.finditer(r'(\w+)\s*:\s*([^,}]+)', content_str):
                name, value = fm.group(1), fm.group(2).strip()
                if is_meaningful(name) and value and not value.startswith('&'):
                    desc = short_desc(name, value)
                    results.append({
                        'file': filepath.as_posix(),
                        'line': line_num,
                        'type': 'default_impl',
                        'name': name,
                        'value': value,
                        'description': desc,
                    })
            continue
        
        # 4. let NAME = VALUE; (key computed values)
        m = re.match(r'let\s+(?:mut\s+)?(\w+)\s*=\s*([^;]+);', stripped)
        if m:
            name, value = m.group(1), m.group(2).strip()
            if (is_meaningful(name) and not value.startswith('&') and len(value) < 60
                and re.search(r'[0-9]', value)):
                desc = short_desc(name, value)
                results.append({
                    'file': filepath.as_posix(),
                    'line': line_num,
                    'type': 'let',
                    'name': name,
                    'value': value,
                    'description': desc,
                })
            continue
        
        # 5. Color values
        m = re.search(r'Color::srgb\(([^)]+)\)', stripped)
        if m:
            results.append({
                'file': filepath.as_posix(),
                'line': line_num,
                'type': 'color',
                'name': 'Color',
                'value': m.group(1).strip(),
                'description': 'couleur RGB',
            })
        
        m = re.search(r'Color::srgba\(([^)]+)\)', stripped)
        if m:
            results.append({
                'file': filepath.as_posix(),
                'line': line_num,
                'type': 'color',
                'name': 'Color',
                'value': m.group(1).strip(),
                'description': 'couleur RGBA',
            })
        
        # 6. Val::Px / Val::Percent
        m = re.search(r'Val::Px\(([^)]+)\)', stripped)
        if m:
            results.append({
                'file': filepath.as_posix(),
                'line': line_num,
                'type': 'UI_val',
                'name': 'Val::Px',
                'value': m.group(1).strip(),
                'description': 'valeur UI',
            })
        
        m = re.search(r'Val::Percent\(([^)]+)\)', stripped)
        if m:
            results.append({
                'file': filepath.as_posix(),
                'line': line_num,
                'type': 'UI_val',
                'name': 'Val::Percent',
                'value': m.group(1).strip(),
                'description': 'valeur UI',
            })
        
        # 7. Vec3::new
        m = re.search(r'Vec3::new\(([^)]+)\)', stripped)
        if m:
            results.append({
                'file': filepath.as_posix(),
                'line': line_num,
                'type': 'vec3',
                'name': 'Vec3::new',
                'value': m.group(1).strip(),
                'description': 'position 3D',
            })
        
        # 8. Slider ranges: (SettingKey::X, min, max)
        m = re.search(r'\(SettingKey::(\w+)(?:\([^)]*\))?,\s*([0-9.]+),\s*([0-9.]+)\)', stripped)
        if m:
            name, minv, maxv = m.group(1), m.group(2), m.group(3)
            results.append({
                'file': filepath.as_posix(),
                'line': line_num,
                'type': 'UI_slider',
                'name': name,
                'value': f"min={minv}, max={maxv}",
                'description': 'intervalle réglage',
            })
        
        # 9. Window resolution
        m = re.search(r'resolution:\s*\(([^)]+)\)', stripped)
        if m:
            results.append({
                'file': filepath.as_posix(),
                'line': line_num,
                'type': 'window',
                'name': 'resolution',
                'value': m.group(1).strip(),
                'description': 'résolution fenêtre',
            })
        
        # 10. Key numeric constants in important contexts
        m = re.search(r'(\w+)\s*=\s*([0-9]+\.?[0-9]*[eE][+-]?\d*)', stripped)
        if m:
            name, value = m.group(1), m.group(2)
            if is_meaningful(name) and len(value) <= 12:
                desc = short_desc(name, value)
                results.append({
                    'file': filepath.as_posix(),
                    'line': line_num,
                    'type': 'key_value',
                    'name': name,
                    'value': value,
                    'description': desc,
                })
    
    return results

def main():
    all_results = []
    for filepath in RS_FILES:
        results = extract_key_values(filepath)
        if results:
            print(f"{filepath.as_posix()}: {len(results)} entries")
        all_results.extend(results)
    
    # Write to file
    output_path = ROOT / "all_variables.txt"
    with open(output_path, 'w', encoding='utf-8') as f:
        f.write("=== SPACESPORE - TOUS LES VARIABLES / CONSTANTES / VALEURS ===\n")
        f.write(f"Projet: SpaceSpore | Fichiers analysés: {len(RS_FILES)} fichiers .rs\n")
        f.write(f"Total entrées: {len(all_results)}\n")
        f.write("Format: Valeur | Texte très court | Emplacement (fichier:ligne)\n\n")
        
        # Group by file
        by_file = {}
        for r in all_results:
            if r['file'] not in by_file:
                by_file[r['file']] = []
            by_file[r['file']].append(r)
        
        for filepath in sorted(by_file.keys()):
            f.write(f"\n{'='*80}\n")
            f.write(f"FILE: {filepath}\n")
            f.write(f"{'='*80}\n\n")
            
            for r in by_file[filepath]:
                desc = r.get('description') or ''
                f.write(f"{r['value']:40} | {r['type']:10s} | {r['name']:30s} | {desc}\n")
                f.write(f"  Emplacement: {r['file']}:{r['line']}\n")
    
    print(f"\n=== FIN ===")
    print(f"Fichier créé: {output_path.as_posix()}")
    print(f"Total entrées: {len(all_results)}")

if __name__ == "__main__":
    main()