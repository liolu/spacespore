#!/usr/bin/env python3
"""
Script to extract all variables/constants/values from Rust files in the spacespore project.
Outputs a .txt file with: value, short description, and file location.
"""

import os
import re
from pathlib import Path

# Project root
ROOT = Path(".")

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

# Patterns to find constants, static values, default values, etc.
PATTERNS = [
    # const NAME: TYPE = VALUE;
    (r'const\s+(\w+)\s*:\s*[\w:<>\[\]]+\s*=\s*([^;]+);', 'const'),
    # static NAME: TYPE = VALUE;
    (r'static\s+(\w+)\s*:\s*[\w:<>\[\]]+\s*=\s*([^;]+);', 'static'),
    # let NAME = VALUE; (in function bodies, but we'll catch some)
    (r'let\s+(?:mut\s+)?(\w+)\s*=\s*([^;]+);', 'let'),
    # Default impl values: field: VALUE,
    (r'(\w+)\s*:\s*([0-9]+\.?[0-9]*[f]?(?:[eE][+-]?\d+)?)\s*,', 'default_field'),
    # fn default() -> Self { Self { field: VALUE, ... } }
    (r'(\w+)\s*:\s*([0-9]+\.?[0-9]*[f]?(?:[eE][+-]?\d+)?)\s*,', 'default_impl'),
    # Color::srgb(r, g, b) or Color::srgba(r, g, b, a)
    (r'Color::srgb\(([^)]+)\)', 'color_srgb'),
    (r'Color::srgba\(([^)]+)\)', 'color_srgba'),
    # Vec3::new(x, y, z)
    (r'Vec3::new\(([^)]+)\)', 'vec3_new'),
    # Val::Px(VALUE)
    (r'Val::Px\(([^)]+)\)', 'val_px'),
    # Val::Percent(VALUE)
    (r'Val::Percent\(([^)]+)\)', 'val_percent'),
    # f32::consts::TAU
    (r'f32::consts::TAU', 'f32_tau'),
    # std::f32::consts::TAU
    (r'std::f32::consts::TAU', 'std_f32_tau'),
    # Default values in struct definitions
    (r'#\[serde\(default\s*=\s*"([^"]+)"\)\]', 'serde_default_fn'),
    # Default trait impl
    (r'fn\s+(\w+)\s*\(\s*\)\s*->\s*f32\s*\{\s*([0-9]+\.?[0-9]*[f]?(?:[eE][+-]?\d+)?)\s*\}', 'default_fn'),
    # impl Default for Struct { fn default() -> Self { Self { field: VALUE, ... } } }
    (r'Self\s*\{\s*([^}]+)\s*\}', 'self_default'),
]

def extract_from_file(filepath):
    """Extract variables/values from a single Rust file."""
    results = []
    try:
        with open(filepath, 'r', encoding='utf-8') as f:
            content = f.read()
    except Exception as e:
        print(f"Error reading {filepath}: {e}")
        return results
    
    lines = content.split('\n')
    
    for line_num, line in enumerate(lines, 1):
        line = line.strip()
        if not line or line.startswith('//'):
            continue
            
        # Check each pattern
        for pattern, ptype in PATTERNS:
            matches = re.finditer(pattern, line)
            for match in matches:
                if ptype in ('const', 'static', 'let'):
                    name = match.group(1)
                    value = match.group(2).strip()
                    results.append({
                        'file': filepath,
                        'line': line_num,
                        'type': ptype,
                        'name': name,
                        'value': value,
                        'context': line[:100]
                    })
                elif ptype in ('default_field', 'default_impl'):
                    name = match.group(1)
                    value = match.group(2)
                    # Filter out common non-value patterns
                    if name not in ['pub', 'fn', 'struct', 'enum', 'impl', 'mod', 'use', 'let', 'mut', 'ref', 'async', 'await', 'match', 'if', 'else', 'for', 'while', 'loop', 'return', 'break', 'continue', 'type', 'const', 'static', 'macro', 'trait', 'where', 'as', 'in', 'move', 'dyn', 'Box', 'Rc', 'Arc', 'Vec', 'HashMap', 'BTreeMap', 'Option', 'Result', 'String', 'str', 'bool', 'char', 'u8', 'u16', 'u32', 'u64', 'u128', 'i8', 'i16', 'i32', 'i64', 'i128', 'f32', 'f64', 'usize', 'isize']:
                        results.append({
                            'file': filepath,
                            'line': line_num,
                            'type': ptype,
                            'name': name,
                            'value': value,
                            'context': line[:100]
                        })
                elif ptype in ('color_srgb', 'color_srgba'):
                    value = match.group(1)
                    results.append({
                        'file': filepath,
                        'line': line_num,
                        'type': ptype,
                        'name': 'Color',
                        'value': value,
                        'context': line[:100]
                    })
                elif ptype == 'vec3_new':
                    value = match.group(1)
                    results.append({
                        'file': filepath,
                        'line': line_num,
                        'type': ptype,
                        'name': 'Vec3',
                        'value': value,
                        'context': line[:100]
                    })
                elif ptype in ('val_px', 'val_percent'):
                    value = match.group(1)
                    results.append({
                        'file': filepath,
                        'line': line_num,
                        'type': ptype,
                        'name': 'Val',
                        'value': value,
                        'context': line[:100]
                    })
                elif ptype in ('f32_tau', 'std_f32_tau'):
                    results.append({
                        'file': filepath,
                        'line': line_num,
                        'type': ptype,
                        'name': 'TAU',
                        'value': '6.283185307179586',
                        'context': line[:100]
                    })
                elif ptype == 'serde_default_fn':
                    fn_name = match.group(1)
                    results.append({
                        'file': filepath,
                        'line': line_num,
                        'type': ptype,
                        'name': fn_name,
                        'value': 'default function',
                        'context': line[:100]
                    })
                elif ptype == 'default_fn':
                    fn_name = match.group(1)
                    value = match.group(2)
                    results.append({
                        'file': filepath,
                        'line': line_num,
                        'type': ptype,
                        'name': fn_name,
                        'value': value,
                        'context': line[:100]
                    })
                elif ptype == 'self_default':
                    # Parse the Self { ... } content
                    content_str = match.group(1)
                    # Extract field: value pairs
                    field_pattern = r'(\w+)\s*:\s*([^,}]+)'
                    for fm in re.finditer(field_pattern, content_str):
                        fname = fm.group(1)
                        fval = fm.group(2).strip()
                        results.append({
                            'file': filepath,
                            'line': line_num,
                            'type': 'default_impl_field',
                            'name': fname,
                            'value': fval,
                            'context': line[:100]
                        })
    
    return results

def main():
    all_results = []
    
    for rs_file in RS_FILES:
        full_path = ROOT / rs_file
        if full_path.exists():
            print(f"Processing {rs_file}...")
            results = extract_from_file(full_path)
            all_results.extend(results)
        else:
            print(f"File not found: {rs_file}")
    
    # Also search for specific important constants across all files
    # Let's do a more comprehensive search for numeric literals in key contexts
    
    # Write results to file
    output_path = ROOT / "variables_list.txt"
    with open(output_path, 'w', encoding='utf-8') as f:
        f.write("=== SPACESPORE - Variables/Constants/Values List ===\n")
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
                f.write(f"  Line {r['line']:4d} | {r['type']:20s} | {r['name']:30s} = {r['value']}\n")
                f.write(f"           Context: {r['context']}\n\n")
    
    print(f"\nDone! Results written to {output_path}")
    print(f"Total entries: {len(all_results)}")

if __name__ == "__main__":
    main()