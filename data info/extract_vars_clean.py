#!/usr/bin/env python3
"""
Clean extraction of key variables/constants/values from Rust files.
Focuses on meaningful configuration values, not every default field.
"""

import os
import re
from pathlib import Path

ROOT = Path(".")

# Focused list of important variable patterns to extract
IMPORTANT_PATTERNS = [
    # Constants: const NAME: TYPE = VALUE;
    (r'^const\s+(\w+)\s*:', 'const'),
    # Static: static NAME: TYPE = VALUE;
    (r'^static\s+(\w+)\s*:', 'static'),
    # Let statements with values (not declarations)
    (r'let\s+(?:mut\s+)?(\w+)\s*=\s*([^;]+);', 'let'),
    # Config struct fields with defaults
    (r'pub\s+(\w+)\s*:\s*([\w<>\[\]]+)\s*=\s*([^,\n]+)', 'config_field'),
    # Default impl fields
    (r'Self\s*\{\s*([^}]+)\s*\}', 'self_default'),
    # Color values
    (r'Color::srgb\(([^)]+)\)', 'color_srgb'),
    (r'Color::srgba\(([^)]+)\)', 'color_srgba'),
    # Vec3 values
    (r'Vec3::new\(([^)]+)\)', 'vec3_new'),
    # Val::Px
    (r'Val::Px\(([^)]+)\)', 'val_px'),
    # Orbit speeds, divisions, etc. - key constants
    (r'(ORBIT_SPEED|STAR_ORBIT_SPEED|MOON_ORBIT_SPEED)\s*=\s*([0-9.]+)', 'orbit_speed'),
    # Lod divisions
    (r'(STAR_DIVISIONS|MOON_DIVISIONS)\s*=\s*(\d+)', 'lod_divisions'),
    # Planet/moon config key values
    (r'orbit_distance\s*=\s*([0-9.]+)', 'orbit_distance'),
    (r'radius\s*=\s*([0-9.]+)', 'radius'),
    (r'seed\s*=\s*(\d+)', 'seed'),
    # Temperature related
    (r'temperature\(\)\s*->\s*f32\s*\{[^}]*-270\.0\s*\+\s*500000\.0', 'temp_formula'),
    # Key numeric values in important contexts
    (r'(\w+)\s*=\s*([0-9]+\.?[0-9]*[eE]?[+-]?\d*)', 'key_value'),
]

def is_meaningful_name(name):
    """Filter out unimportant identifiers."""
    unimportant = {
        'pub', 'fn', 'struct', 'enum', 'impl', 'mod', 'use', 'let', 'mut', 
        'ref', 'async', 'await', 'match', 'if', 'else', 'for', 'while', 
        'loop', 'return', 'break', 'continue', 'type', 'const', 'static',
        'macro', 'trait', 'where', 'as', 'in', 'move', 'dyn', 'Box', 'Rc', 
        'Arc', 'Vec', 'HashMap', 'BTreeMap', 'Option', 'Result', 'String', 
        'str', 'bool', 'char', 'u8', 'u16', 'u32', 'u64', 'u128',
        'i8', 'i16', 'i32', 'i64', 'i128', 'f32', 'f64', 'usize', 'isize',
        'self', 'Super', 'And', 'Or', 'Not'
    }
    return name.lower() not in unimportant

def extract_from_file(filepath):
    """Extract key variables/values from a single Rust file."""
    results = []
    try:
        with open(filepath, 'r', encoding='utf-8') as f:
            content = f.read()
    except Exception as e:
        print(f"Error reading {filepath}: {e}")
        return results
    
    lines = content.split('\n')
    
    for line_num, line in enumerate(lines, 1):
        stripped = line.strip()
        if not stripped or stripped.startswith('//'):
            continue
        
        # Only process lines that have = assignments with values
        if '=' not in stripped:
            continue
        
        # Pattern 1: const NAME: TYPE = VALUE;
        m = re.match(r'^const\s+(\w+)\s*:', stripped)
        if m:
            name = m.group(1)
            if is_meaningful_name(name):
                # Extract the value part
                value_match = re.search(r'=\s*([^;]+)', stripped)
                value = value_match.group(1).strip() if value_match else ""
                results.append({
                    'file': filepath,
                    'line': line_num,
                    'type': 'const',
                    'name': name,
                    'value': value,
                    'context': stripped[:120]
                })
            continue
        
        # Pattern 2: static NAME: TYPE = VALUE;
        m = re.match(r'^static\s+(\w+)\s*:', stripped)
        if m:
            name = m.group(1)
            if is_meaningful_name(name):
                # Extract the value part
                value_match = re.search(r'=\s*([^;]+)', stripped)
                value = value_match.group(1).strip() if value_match else ""
                results.append({
                    'file': filepath,
                    'line': line_num,
                    'type': 'static',
                    'name': name,
                    'value': value,
                    'context': stripped[:120]
                })
            continue
        
        # Pattern 3: let NAME = VALUE;
        m = re.match(r'let\s+(?:mut\s+)?(\w+)\s*=\s*([^;]+);', stripped)
        if m:
            name = m.group(1)
            value = m.group(2).strip()
            if is_meaningful_name(name) and not value.startswith('&') and len(value) < 50:
                # Try to determine a short description
                desc = get_short_desc(name, value, stripped)
                results.append({
                    'file': filepath,
                    'line': line_num,
                    'type': 'let',
                    'name': name,
                    'value': value,
                    'description': desc,
                    'context': stripped[:120]
                })
            continue
        
        # Pattern 4: Config struct fields: pub field: TYPE = VALUE,
        m = re.match(r'pub\s+(\w+)\s*:\s*([\w<>\[\]]+)\s*=\s*([^,\n]+)', stripped)
        if m:
            name = m.group(1)
            value = m.group(3).strip()
            if is_meaningful_name(name) and value and not value.startswith('&'):
                desc = get_short_desc(name, value, stripped)
                results.append({
                    'file': filepath,
                    'line': line_num,
                    'type': 'config_field',
                    'name': name,
                    'value': value,
                    'description': desc,
                    'context': stripped[:120]
                })
            continue
        
        # Pattern 5: Key orbit/speed values
        m = re.search(r'(ORBIT_SPEED|STAR_ORBIT_SPEED|MOON_ORBIT_SPEED)\s*=\s*([0-9.]+)', stripped)
        if m:
            name = m.group(1)
            value = m.group(2)
            desc = f"Orbit speed constant"
            results.append({
                'file': filepath,
                'line': line_num,
                'type': 'orbit_speed',
                'name': name,
                'value': value,
                'description': desc,
                'context': stripped[:120]
            })
            continue
        
        # Pattern 6: Lod divisions
        m = re.search(r'(STAR_DIVISIONS|MOON_DIVISIONS)\s*=\s*(\d+)', stripped)
        if m:
            name = m.group(1)
            value = m.group(2)
            desc = f"LOD divisions"
            results.append({
                'file': filepath,
                'line': line_num,
                'type': 'lod_divisions',
                'name': name,
                'value': value,
                'description': desc,
                'context': stripped[:120]
            })
            continue
        
        # Pattern 7: Color values
        m = re.search(r'Color::srgb\(([^)]+)\)', stripped)
        if m:
            value = m.group(1)
            results.append({
                'file': filepath,
                'line': line_num,
                'type': 'color_srgb',
                'name': 'Color',
                'value': value,
                'description': 'RGB color',
                'context': stripped[:120]
            })
            # Don't continue - there might be more patterns
        
        m = re.search(r'Color::srgba\(([^)]+)\)', stripped)
        if m:
            value = m.group(1)
            results.append({
                'file': filepath,
                'line': line_num,
                'type': 'color_srgba',
                'name': 'Color',
                'value': value,
                'description': 'RGBA color',
                'context': stripped[:120]
            })
    
    return results

def get_short_desc(name, value, full_line):
    """Generate a short descriptive text for the variable."""
    # Clean the value
    v = value.strip()
    
    # Remove type annotations if present
    if 'f32' in v or 'f64' in v:
        # Extract just the number
        num_match = re.search(r'([0-9]+\.?[0-9]*)', v)
        if num_match:
            v = num_match.group(1)
    
    if not v or v == '0.0' or v == '0':
        return None
    
    # Generate description based on name
    desc_map = {
        'orbit_distance': 'orbite distance',
        'radius': 'rayon',
        'seed': 'graine',
        'temperature': 'température',
        'mouse_sensitivity': 'sensibilité souris',
        'scroll_speed': 'vitesse zoom',
        'keyboard_speed': 'vitesse clavier',
        'invert_y': 'inverser Y',
        'show_light_indicator': 'afficher éclairage',
        'show_orbits': 'afficher orbites',
        'planet_chunk_divisions': 'divisions planète',
        'orbit_speed': 'vitesse orbite',
        'star_orbit_speed': 'vitesse orbite étoile',
        'moon_orbit_speed': 'vitesse orbite lune',
        'star_divisions': 'divisions étoile',
        'moon_divisions': 'divisions lune',
        'cloud_density': 'densité nuages',
        'cloud_altitude': 'altitude nuages',
        'cloud_speed': 'vitesse nuages',
        'flare_count': 'nombre éruptions',
        'flare_height': 'hauteur éruptions',
        'flare_speed': 'vitesse éruptions',
        'flare_size': 'taille éruptions',
        'flare_distance': 'distance éruptions',
        'belt_distance': 'distance ceinture',
        'belt_width': 'écartement ceinture',
        'belt_min_size': 'taille min ceinture',
        'belt_max_size': 'taille max ceinture',
        'belt_count': 'nombre ceintures',
        'moon_orbit_distance': 'distance orbite lune',
        'moon_radius': 'rayon lune',
        'moon_seed': 'graine lune',
    }
    
    name_lower = name.lower()
    for key, desc in desc_map.items():
        if key in name_lower:
            return desc
    
    # Generic descriptions based on value type
    if '.' in v:
        try:
            fval = float(v)
            if fval > 1000.0:
                return 'grande valeur'
            elif fval > 1.0:
                return 'valeur moyenne'
            else:
                return 'petite valeur'
        except:
            pass
    
    if v.isdigit():
        ival = int(v)
        if ival > 100:
            return 'nombre élevé'
        elif ival > 10:
            return 'nombre moyen'
        else:
            return 'petit nombre'
    
    return None

def main():
    # All .rs files to analyze
    RS_FILES = [
        "src/main.rs",
        "src/settings.rs",
        "src/planet.rs",
        "src/lod.rs",
        "src/mesher.rs",
        "src/ui.rs",
        "src/astre/mod.rs",
        "src/astre/black_hole.rs",
        "src/astre/comet.rs",
        "src/astre/meteoroid.rs",
        "src/astre/planet.rs",
        "src/astre/supernova.rs",
        "src/astre/etoile/mod.rs",
        "src/astre/etoile/dwarf_star.rs",
        "src/astre/etoile/giant_star.rs",
        "src/astre/etoile/hypergiant_star.rs",
        "src/astre/etoile/main_sequence_star.rs",
        "src/astre/etoile/protostar.rs",
        "src/astre/etoile/star.rs",
        "src/astre/etoile/supergiant_star.rs",
        "src/astre/planete/comet.rs",
        "src/astre/planete/gas_planet.rs",
        "src/astre/planete/meteoroid.rs",
        "src/astre/planete/mod.rs",
        "src/astre/Remnant_stellaire/black_hole.rs",
        "src/astre/Remnant_stellaire/magnetar.rs",
        "src/astre/Remnant_stellaire/mod.rs",
        "src/astre/Remnant_stellaire/nebula.rs",
        "src/astre/Remnant_stellaire/neutron_star.rs",
        "src/astre/Remnant_stellaire/pulsar.rs",
        "src/astre/Remnant_stellaire/supernova.rs",
    ]
    
    all_results = []
    
    for rs_file in RS_FILES:
        full_path = ROOT / rs_file
        if full_path.exists():
            print(f"Processing {rs_file}...")
            results = extract_from_file(full_path)
            all_results.extend(results)
        else:
            print(f"File not found: {rs_file}")
    
    # Write results to file
    output_path = ROOT / "variables_important.txt"
    with open(output_path, 'w', encoding='utf-8') as f:
        f.write("=== SPACESPORE - Important Variables/Constants/Values ===\n")
        f.write(f"Generated from {len(RS_FILES)} Rust source files\n\n")
        
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
                desc = r.get('description', '')
                desc_str = f" | Description: {desc}" if desc else ""
                value = r.get('value', '')
                f.write(f"  {r['type']:12s} | {r['name']:30s} = {value}{desc_str}\n")
                if 'context' in r and r['context']:
                    f.write(f"           Line {r['line']}: {r['context'][:80]}\n")
    
    print(f"\nDone! Results written to {output_path}")
    print(f"Total entries: {len(all_results)}")

if __name__ == "__main__":
    main()