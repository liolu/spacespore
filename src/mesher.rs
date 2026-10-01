use bevy::math::Vec3;
use bevy::render::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use noise::{Fbm, NoiseFn, Perlin};

use crate::lod::LodLevel;
use crate::planet::{CubeFace, VoxelType};
fn max_greedy_for_lod(lod: LodLevel) -> usize {
    match lod {
        LodLevel::Lod0 | LodLevel::Lod1 => 1,
        LodLevel::Lod2 => 2,
        _ => 6,
    }
}

#[inline]
fn emit_quad(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    colors: &mut Vec<[f32; 4]>,
    indices: &mut Vec<u32>,
    corners: [Vec3; 4],
    normal: [f32; 3],
    color: [f32; 4],
    flip: bool,
) {
    let base = positions.len() as u32;
    for c in &corners {
        positions.push(c.to_array());
        normals.push(normal);
        colors.push(color);
    }
    if flip {
        indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    } else {
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

pub fn build_celestial_chunk_mesh(
    face: CubeFace,
    grid_x: usize,
    grid_y: usize,
    divisions: usize,
    radius: f32,
    terrain_height: f32,
    seed: u32,
    noise_scale: f32,
    color_low: [f32; 4],
    color_high: [f32; 4],
    lod: LodLevel,
) -> Mesh {
    let res = lod.resolution();
    let max_greedy = 4usize;

    let chunk_u_start = grid_x as f32 / divisions as f32;
    let chunk_v_start = grid_y as f32 / divisions as f32;
    let chunk_size = 1.0 / divisions as f32;

    let layers: usize = 10;
    let r_min = radius - terrain_height - 1.0;
    let r_max = radius + terrain_height + 1.0;

    let mut fbm: Fbm<Perlin> = Fbm::new(seed);
    fbm.octaves = 6;
    let mut detail_fbm: Fbm<Perlin> = Fbm::new(seed.wrapping_add(50));
    detail_fbm.octaves = 3;
    let color_perlin = Perlin::new(seed.wrapping_add(300));
    let spot_perlin = Perlin::new(seed.wrapping_add(500));

    let mut terrain_heights = vec![vec![0.0_f32; res]; res];
    let mut height_blend = vec![vec![0.0_f32; res]; res];
    let mut cell_dirs = vec![vec![Vec3::ZERO; res]; res];
    let mut cell_color_var = vec![vec![0.0_f32; res]; res];
    let mut cell_spot = vec![vec![0.0_f32; res]; res];

    for ix in 0..res {
        for iy in 0..res {
            let u_mid = chunk_u_start + ((ix as f32 + 0.5) / res as f32) * chunk_size;
            let v_mid = chunk_v_start + ((iy as f32 + 0.5) / res as f32) * chunk_size;
            let dir = face.to_sphere_pos(u_mid, v_mid);
            cell_dirs[ix][iy] = dir;

            cell_color_var[ix][iy] = color_perlin.get([
                dir.x as f64 * 14.0,
                dir.y as f64 * 14.0,
                dir.z as f64 * 14.0,
            ]) as f32
                * 0.08;

            cell_spot[ix][iy] = spot_perlin.get([
                dir.x as f64 * 6.0,
                dir.y as f64 * 6.0,
                dir.z as f64 * 6.0,
            ]) as f32;

            let s = dir * noise_scale;
            let n_val = fbm.get([s.x as f64, s.y as f64, s.z as f64]) as f32;
            let det = detail_fbm.get([
                s.x as f64 * 3.0,
                s.y as f64 * 3.0,
                s.z as f64 * 3.0,
            ]) as f32 * 0.2;
            let height_val = ((n_val + det + 1.0) * 0.5).clamp(0.0, 1.0);
            terrain_heights[ix][iy] = radius + (height_val - 0.5) * terrain_height;
            height_blend[ix][iy] = height_val;
        }
    }

    let mut voxels = vec![vec![vec![false; res]; res]; layers];
    for layer in 0..layers {
        let r_mid = r_min + ((layer as f32 + 0.5) / layers as f32) * (r_max - r_min);
        for ix in 0..res {
            for iy in 0..res {
                voxels[layer][ix][iy] = r_mid <= terrain_heights[ix][iy];
            }
        }
    }

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let get_voxel = |layer: i32, ix: i32, iy: i32| -> bool {
        if layer < 0
            || layer >= layers as i32
            || ix < 0
            || ix >= res as i32
            || iy < 0
            || iy >= res as i32
        {
            false
        } else {
            voxels[layer as usize][ix as usize][iy as usize]
        }
    };

    let corner_pos = |ix: usize, iy: usize, r: f32| -> Vec3 {
        let u = chunk_u_start + (ix as f32 / res as f32) * chunk_size;
        let v = chunk_v_start + (iy as f32 / res as f32) * chunk_size;
        face.to_sphere_pos(u, v) * r
    };

    let cell_col = |ix: usize, iy: usize| -> [f32; 4] {
        let cix = ix.min(res - 1);
        let ciy = iy.min(res - 1);
        let t = height_blend[cix][ciy];
        let var = cell_color_var[cix][ciy];
        let spot = cell_spot[cix][ciy] * 0.06;
        [
            (color_low[0] + (color_high[0] - color_low[0]) * t + var + spot).clamp(0.03, 1.0),
            (color_low[1] + (color_high[1] - color_low[1]) * t + var * 0.8 + spot * 0.5).clamp(0.03, 1.0),
            (color_low[2] + (color_high[2] - color_low[2]) * t + var * 0.4).clamp(0.03, 1.0),
            color_low[3],
        ]
    };

    for layer in 0..layers {
        let r_lo = r_min + (layer as f32 / layers as f32) * (r_max - r_min);
        let r_hi = r_min + ((layer + 1) as f32 / layers as f32) * (r_max - r_min);

        // Top faces
        {
            let mut visited = vec![vec![false; res]; res];
            for ix in 0..res {
                for iy in 0..res {
                    if visited[ix][iy] || !voxels[layer][ix][iy] || get_voxel(layer as i32 + 1, ix as i32, iy as i32) {
                        visited[ix][iy] = true;
                        continue;
                    }

                    let mut w = 1usize;
                    while ix + w < res && w < max_greedy && !visited[ix + w][iy] && voxels[layer][ix + w][iy] && !get_voxel(layer as i32 + 1, (ix + w) as i32, iy as i32) {
                        w += 1;
                    }
                    let mut h = 1usize;
                    'top_h: while iy + h < res && h < max_greedy {
                        for dx in 0..w {
                            if visited[ix + dx][iy + h] || !voxels[layer][ix + dx][iy + h] || get_voxel(layer as i32 + 1, (ix + dx) as i32, (iy + h) as i32) {
                                break 'top_h;
                            }
                        }
                        h += 1;
                    }
                    for dx in 0..w { for dy in 0..h { visited[ix + dx][iy + dy] = true; } }

                    let mid_ix = ix + w / 2;
                    let mid_iy = iy + h / 2;
                    let n = cell_dirs[mid_ix.min(res - 1)][mid_iy.min(res - 1)].to_array();
                    let col = cell_col(mid_ix, mid_iy);
                    emit_quad(&mut positions, &mut normals, &mut colors, &mut indices,
                        [corner_pos(ix, iy, r_hi), corner_pos(ix + w, iy, r_hi), corner_pos(ix + w, iy + h, r_hi), corner_pos(ix, iy + h, r_hi)],
                        n, col, false);
                }
            }
        }

        // Bottom faces
        {
            let mut visited = vec![vec![false; res]; res];
            for ix in 0..res {
                for iy in 0..res {
                    if visited[ix][iy] || !voxels[layer][ix][iy] || get_voxel(layer as i32 - 1, ix as i32, iy as i32) {
                        visited[ix][iy] = true;
                        continue;
                    }
                    let mut w = 1usize;
                    while ix + w < res && w < max_greedy && !visited[ix + w][iy] && voxels[layer][ix + w][iy] && !get_voxel(layer as i32 - 1, (ix + w) as i32, iy as i32) {
                        w += 1;
                    }
                    let mut h = 1usize;
                    'bot_h: while iy + h < res && h < max_greedy {
                        for dx in 0..w {
                            if visited[ix + dx][iy + h] || !voxels[layer][ix + dx][iy + h] || get_voxel(layer as i32 - 1, (ix + dx) as i32, (iy + h) as i32) {
                                break 'bot_h;
                            }
                        }
                        h += 1;
                    }
                    for dx in 0..w { for dy in 0..h { visited[ix + dx][iy + dy] = true; } }

                    let mid_ix = ix + w / 2;
                    let mid_iy = iy + h / 2;
                    let n = (-cell_dirs[mid_ix.min(res - 1)][mid_iy.min(res - 1)]).to_array();
                    let col = cell_col(mid_ix, mid_iy);
                    emit_quad(&mut positions, &mut normals, &mut colors, &mut indices,
                        [corner_pos(ix, iy, r_lo), corner_pos(ix + w, iy, r_lo), corner_pos(ix + w, iy + h, r_lo), corner_pos(ix, iy + h, r_lo)],
                        n, col, true);
                }
            }
        }

        // Side faces
        for ix in 0..res {
            for iy in 0..res {
                if !voxels[layer][ix][iy] { continue; }
                let col = cell_col(ix, iy);
                let dir = cell_dirs[ix][iy];
                let sides: [(i32, i32, usize, usize, usize, usize); 4] = [
                    (0, -1, ix, iy, ix + 1, iy),
                    (1, 0, ix + 1, iy, ix + 1, iy + 1),
                    (0, 1, ix + 1, iy + 1, ix, iy + 1),
                    (-1, 0, ix, iy + 1, ix, iy),
                ];
                for (dx, dy, e0u, e0v, e1u, e1v) in sides {
                    if get_voxel(layer as i32, ix as i32 + dx, iy as i32 + dy) { continue; }
                    let p0_lo = corner_pos(e0u, e0v, r_lo);
                    let p1_lo = corner_pos(e1u, e1v, r_lo);
                    let p0_hi = corner_pos(e0u, e0v, r_hi);
                    let p1_hi = corner_pos(e1u, e1v, r_hi);
                    let tangent = (p1_lo - p0_lo).normalize();
                    let n = tangent.cross(dir).normalize().to_array();
                    emit_quad(&mut positions, &mut normals, &mut colors, &mut indices,
                        [p0_lo, p1_lo, p1_hi, p0_hi], n, col, false);
                }
            }
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

pub fn build_chunk_mesh(
    face: CubeFace,
    grid_x: usize,
    grid_y: usize,
    divisions: usize,
    radius: f32,
    sea_level: f32,
    terrain_height: f32,
    seed: u32,
    noise_scale: f32,
    detail_scale: f32,
    lod: LodLevel,
    temperature: f32,
) -> Mesh {
    let res = lod.resolution();
    let max_greedy = max_greedy_for_lod(lod);

    let chunk_u_start = grid_x as f32 / divisions as f32;
    let chunk_v_start = grid_y as f32 / divisions as f32;
    let chunk_size = 1.0 / divisions as f32;

    let th = terrain_height;
    let sl = sea_level;
    let layers: usize = 12;
    let r_min = radius - sl * th - 2.0;
    let r_max = radius + (1.0 - sl) * th + 2.0;

    let frozen = temperature < -50.0;
    let snow_lat = if temperature > 200.0 {
        2.0
    } else if temperature > 60.0 {
        0.95
    } else if temperature > 20.0 {
        0.78
    } else if temperature > -20.0 {
        0.50
    } else {
        0.15
    };

    let mut fbm: Fbm<Perlin> = Fbm::new(seed);
    fbm.octaves = 6;
    let mut detail_fbm: Fbm<Perlin> = Fbm::new(seed.wrapping_add(81));
    detail_fbm.octaves = 4;
    let color_perlin = Perlin::new(seed.wrapping_add(200));

    let mut terrain_heights = vec![vec![0.0f32; res]; res];
    let mut surface_types = vec![vec![VoxelType::Air; res]; res];
    let mut cell_dirs = vec![vec![Vec3::ZERO; res]; res];
    let mut cell_color_var = vec![vec![0.0f32; res]; res];

    for ix in 0..res {
        for iy in 0..res {
            let u_mid = chunk_u_start + ((ix as f32 + 0.5) / res as f32) * chunk_size;
            let v_mid = chunk_v_start + ((iy as f32 + 0.5) / res as f32) * chunk_size;
            let dir = face.to_sphere_pos(u_mid, v_mid);
            cell_dirs[ix][iy] = dir;

            cell_color_var[ix][iy] = color_perlin.get([
                dir.x as f64 * 12.0,
                dir.y as f64 * 12.0,
                dir.z as f64 * 12.0,
            ]) as f32
                * 0.10;

            let s = dir * noise_scale;
            let continent = fbm.get([s.x as f64, s.y as f64, s.z as f64]) as f32;
            let ds = detail_scale as f64;
            let det =
                detail_fbm.get([s.x as f64 * ds, s.y as f64 * ds, s.z as f64 * ds]) as f32
                    * 0.15;

            let height_val = ((continent + det + 1.0) * 0.5).clamp(0.0, 1.0);
            terrain_heights[ix][iy] = radius + (height_val - sl) * th;

            let rh = height_val - sl;
            let lat = dir.y.abs();
            surface_types[ix][iy] = if temperature > 300.0 {
                if rh < 0.0 { VoxelType::Stone } else { VoxelType::Sand }
            } else if temperature > 100.0 {
                if rh < 0.0 { VoxelType::Sand }
                else if rh < 0.30 { VoxelType::Sand }
                else { VoxelType::Stone }
            } else if rh < 0.0 {
                VoxelType::Sand
            } else if rh < 0.02 {
                VoxelType::Sand
            } else if rh < 0.12 {
                if lat > snow_lat { VoxelType::Snow } else { VoxelType::Grass }
            } else if rh < 0.28 {
                if lat > (snow_lat - 0.04) { VoxelType::Snow } else { VoxelType::Grass }
            } else if rh < 0.42 {
                if lat > snow_lat { VoxelType::Snow } else { VoxelType::Stone }
            } else {
                VoxelType::Snow
            };
        }
    }

    let mut voxels = vec![vec![vec![VoxelType::Air; res]; res]; layers];
    for layer in 0..layers {
        let r_mid = r_min + ((layer as f32 + 0.5) / layers as f32) * (r_max - r_min);
        for ix in 0..res {
            for iy in 0..res {
                let terrain_h = terrain_heights[ix][iy];
                voxels[layer][ix][iy] = if r_mid > terrain_h {
                    if r_mid <= radius {
                        if frozen { VoxelType::Snow } else { VoxelType::Water }
                    } else {
                        VoxelType::Air
                    }
                } else {
                    surface_types[ix][iy]
                };
            }
        }
    }

    // Mesh output
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let get_voxel = |layer: i32, ix: i32, iy: i32| -> VoxelType {
        if layer < 0
            || layer >= layers as i32
            || ix < 0
            || ix >= res as i32
            || iy < 0
            || iy >= res as i32
        {
            VoxelType::Air
        } else {
            voxels[layer as usize][ix as usize][iy as usize]
        }
    };

    let corner_pos = |ix: usize, iy: usize, r: f32| -> Vec3 {
        let u = chunk_u_start + (ix as f32 / res as f32) * chunk_size;
        let v = chunk_v_start + (iy as f32 / res as f32) * chunk_size;
        face.to_sphere_pos(u, v) * r
    };

    // Compute per-cell color with noise variation + water depth
    let cell_color = |vtype: VoxelType, ix: usize, iy: usize, r_hi: f32| -> [f32; 4] {
        let base = vtype.color();
        let var = cell_color_var[ix.min(res - 1)][iy.min(res - 1)];

        // Water depth darkening
        let depth_darken = if vtype == VoxelType::Water {
            ((radius - r_hi) / 12.0).clamp(0.0, 0.45)
        } else {
            0.0
        };

        [
            (base[0] * (1.0 - depth_darken) + var * 0.7).clamp(0.03, 1.0),
            (base[1] * (1.0 - depth_darken * 0.4) + var * 0.9).clamp(0.03, 1.0),
            (base[2] * (1.0 - depth_darken * 0.2) + var * 0.4).clamp(0.03, 1.0),
            base[3],
        ]
    };

    for layer in 0..layers {
        let r_lo = r_min + (layer as f32 / layers as f32) * (r_max - r_min);
        let r_hi = r_min + ((layer + 1) as f32 / layers as f32) * (r_max - r_min);

        // ========== TOP FACES — greedy (size 1 at close LOD = individual cubes) ==========
        {
            let mut visited = vec![vec![false; res]; res];
            for ix in 0..res {
                for iy in 0..res {
                    if visited[ix][iy] {
                        continue;
                    }
                    let vtype = voxels[layer][ix][iy];
                    if !vtype.is_solid() {
                        visited[ix][iy] = true;
                        continue;
                    }
                    if get_voxel(layer as i32 + 1, ix as i32, iy as i32).is_solid() {
                        visited[ix][iy] = true;
                        continue;
                    }

                    let mut w = 1usize;
                    while ix + w < res
                        && w < max_greedy
                        && !visited[ix + w][iy]
                        && voxels[layer][ix + w][iy] == vtype
                        && !get_voxel(layer as i32 + 1, (ix + w) as i32, iy as i32).is_solid()
                    {
                        w += 1;
                    }

                    let mut h = 1usize;
                    'top_h: while iy + h < res && h < max_greedy {
                        for dx in 0..w {
                            if visited[ix + dx][iy + h]
                                || voxels[layer][ix + dx][iy + h] != vtype
                                || get_voxel(
                                    layer as i32 + 1,
                                    (ix + dx) as i32,
                                    (iy + h) as i32,
                                )
                                .is_solid()
                            {
                                break 'top_h;
                            }
                        }
                        h += 1;
                    }

                    for dx in 0..w {
                        for dy in 0..h {
                            visited[ix + dx][iy + dy] = true;
                        }
                    }

                    let mid_ix = ix + w / 2;
                    let mid_iy = iy + h / 2;
                    let n = cell_dirs[mid_ix.min(res - 1)][mid_iy.min(res - 1)].to_array();
                    let col = cell_color(vtype, mid_ix, mid_iy, r_hi);

                    emit_quad(
                        &mut positions,
                        &mut normals,
                        &mut colors,
                        &mut indices,
                        [
                            corner_pos(ix, iy, r_hi),
                            corner_pos(ix + w, iy, r_hi),
                            corner_pos(ix + w, iy + h, r_hi),
                            corner_pos(ix, iy + h, r_hi),
                        ],
                        n,
                        col,
                        false,
                    );
                }
            }
        }

        // ========== BOTTOM FACES — always greedy (rarely seen) ==========
        {
            let mg_bot = 6usize;
            let mut visited = vec![vec![false; res]; res];
            for ix in 0..res {
                for iy in 0..res {
                    if visited[ix][iy] {
                        continue;
                    }
                    let vtype = voxels[layer][ix][iy];
                    if !vtype.is_solid() {
                        visited[ix][iy] = true;
                        continue;
                    }
                    if get_voxel(layer as i32 - 1, ix as i32, iy as i32).is_solid() {
                        visited[ix][iy] = true;
                        continue;
                    }

                    let mut w = 1usize;
                    while ix + w < res
                        && w < mg_bot
                        && !visited[ix + w][iy]
                        && voxels[layer][ix + w][iy] == vtype
                        && !get_voxel(layer as i32 - 1, (ix + w) as i32, iy as i32).is_solid()
                    {
                        w += 1;
                    }

                    let mut h = 1usize;
                    'bot_h: while iy + h < res && h < mg_bot {
                        for dx in 0..w {
                            if visited[ix + dx][iy + h]
                                || voxels[layer][ix + dx][iy + h] != vtype
                                || get_voxel(
                                    layer as i32 - 1,
                                    (ix + dx) as i32,
                                    (iy + h) as i32,
                                )
                                .is_solid()
                            {
                                break 'bot_h;
                            }
                        }
                        h += 1;
                    }

                    for dx in 0..w {
                        for dy in 0..h {
                            visited[ix + dx][iy + dy] = true;
                        }
                    }

                    let mid_ix = ix + w / 2;
                    let mid_iy = iy + h / 2;
                    let n = (-cell_dirs[mid_ix.min(res - 1)][mid_iy.min(res - 1)]).to_array();
                    let col = cell_color(vtype, mid_ix, mid_iy, r_lo);

                    emit_quad(
                        &mut positions,
                        &mut normals,
                        &mut colors,
                        &mut indices,
                        [
                            corner_pos(ix, iy, r_lo),
                            corner_pos(ix + w, iy, r_lo),
                            corner_pos(ix + w, iy + h, r_lo),
                            corner_pos(ix, iy + h, r_lo),
                        ],
                        n,
                        col,
                        true,
                    );
                }
            }
        }

        // ========== SIDE FACES — per-voxel, O(1) neighbor lookup ==========
        for ix in 0..res {
            for iy in 0..res {
                let vtype = voxels[layer][ix][iy];
                if !vtype.is_solid() {
                    continue;
                }
                let col = cell_color(vtype, ix, iy, r_hi);
                let dir = cell_dirs[ix][iy];

                let sides: [(i32, i32, usize, usize, usize, usize); 4] = [
                    (0, -1, ix, iy, ix + 1, iy),
                    (1, 0, ix + 1, iy, ix + 1, iy + 1),
                    (0, 1, ix + 1, iy + 1, ix, iy + 1),
                    (-1, 0, ix, iy + 1, ix, iy),
                ];

                for (dx, dy, e0u, e0v, e1u, e1v) in sides {
                    if get_voxel(layer as i32, ix as i32 + dx, iy as i32 + dy).is_solid() {
                        continue;
                    }

                    let p0_lo = corner_pos(e0u, e0v, r_lo);
                    let p1_lo = corner_pos(e1u, e1v, r_lo);
                    let p0_hi = corner_pos(e0u, e0v, r_hi);
                    let p1_hi = corner_pos(e1u, e1v, r_hi);

                    let tangent = (p1_lo - p0_lo).normalize();
                    let n = tangent.cross(dir).normalize().to_array();

                    emit_quad(
                        &mut positions,
                        &mut normals,
                        &mut colors,
                        &mut indices,
                        [p0_lo, p1_lo, p1_hi, p0_hi],
                        n,
                        col,
                        false,
                    );
                }
            }
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

// ─────────────────────────────────────────────────────────────────────────
//  Géantes gazeuses et neptuniennes : sphère lisse à bandes (pas de relief)
// ─────────────────────────────────────────────────────────────────────────

/// Couleurs et allure d'une géante : bandes de latitude, déformées par des tourbillons.
pub struct GasLook {
    pub palette: Vec<[f32; 3]>,
    /// Nombre de bandes de l'équateur au pôle.
    pub bands: f32,
    /// Ampleur des tourbillons (radians de latitude).
    pub swirl: f32,
    /// Contraste entre bandes (0 : uni, 1 : bandes franches).
    pub contrast: f32,
}

/// Couleur de la géante dans la direction `dir` (repris de l'ancien `gas_planet.rs`).
pub fn gas_color(look: &GasLook, fbm: &Fbm<Perlin>, dir: Vec3) -> [f32; 3] {
    let lat = dir.y.clamp(-1.0, 1.0).asin();
    let lon = dir.z.atan2(dir.x);
    let swirl = fbm.get([dir.x as f64 * 3.0, dir.y as f64 * 9.0, dir.z as f64 * 3.0]) as f32 * look.swirl;
    let eff_lat = lat + swirl;
    let band_t = ((eff_lat * look.bands).sin() * 0.5 + 0.5) * look.contrast + 0.5 * (1.0 - look.contrast);
    let n = look.palette.len().max(2);
    let idx_f = band_t * (n as f32 - 1.0);
    let lo = (idx_f as usize).min(n - 2);
    let detail = fbm.get([lon as f64 * 4.0, eff_lat as f64 * 12.0, 0.5]) as f32;
    let frac = (idx_f - lo as f32 + detail * 0.15).clamp(0.0, 1.0);
    let (a, b) = (look.palette[lo], look.palette[(lo + 1).min(look.palette.len() - 1)]);
    [a[0] + (b[0] - a[0]) * frac, a[1] + (b[1] - a[1]) * frac, a[2] + (b[2] - a[2]) * frac]
}

/// Sphère-cube lisse de rayon `radius`, `res` × `res` carreaux par face, couleurs par sommet.
pub fn build_gas_giant_mesh(radius: f32, seed: u32, look: &GasLook, res: usize) -> Mesh {
    let mut fbm: Fbm<Perlin> = Fbm::new(seed);
    fbm.octaves = 4;
    let n1 = res + 1;
    let mut positions = Vec::with_capacity(6 * n1 * n1);
    let mut normals = Vec::with_capacity(6 * n1 * n1);
    let mut colors = Vec::with_capacity(6 * n1 * n1);
    let mut indices = Vec::with_capacity(6 * res * res * 6);
    for face in CubeFace::all() {
        let base = positions.len() as u32;
        for j in 0..n1 {
            for i in 0..n1 {
                let dir = face.to_sphere_pos(i as f32 / res as f32, j as f32 / res as f32).normalize();
                let c = gas_color(look, &fbm, dir);
                positions.push((dir * radius).to_array());
                normals.push(dir.to_array());
                colors.push([c[0], c[1], c[2], 1.0]);
            }
        }
        for j in 0..res as u32 {
            for i in 0..res as u32 {
                let a = base + j * n1 as u32 + i;
                let (b, c, d) = (a + 1, a + n1 as u32, a + n1 as u32 + 1);
                indices.extend_from_slice(&[a, c, b, b, c, d]);
            }
        }
    }
    // Triangles tournés vers l'extérieur, quelle que soit l'orientation de chaque face
    for tri in indices.chunks_mut(3) {
        let p = |k: u32| Vec3::from_array(positions[k as usize]);
        if (p(tri[1]) - p(tri[0])).cross(p(tri[2]) - p(tri[0])).dot(p(tri[0])) < 0.0 {
            tri.swap(1, 2);
        }
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

#[cfg(test)]
mod gas_tests {
    use super::*;
    use bevy::render::mesh::VertexAttributeValues;

    #[test]
    fn gas_giants_are_smooth_closed_spheres_facing_out() {
        let look = GasLook { palette: vec![[0.9, 0.7, 0.5], [0.6, 0.4, 0.2], [0.95, 0.9, 0.8]], bands: 10.0, swirl: 0.2, contrast: 1.0 };
        let mesh = build_gas_giant_mesh(80_000.0, 7, &look, 16);
        let Some(VertexAttributeValues::Float32x3(pos)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { panic!() };
        let Some(Indices::U32(idx)) = mesh.indices() else { panic!() };
        for v in pos {
            assert!((Vec3::from_array(*v).length() - 80_000.0).abs() < 1.0);
        }
        for tri in idx.chunks(3) {
            let v = |i: u32| Vec3::from_array(pos[i as usize]);
            let n = (v(tri[1]) - v(tri[0])).cross(v(tri[2]) - v(tri[0]));
            assert!(n.dot(v(tri[0])) > 0.0, "face tournee vers l'interieur");
        }
    }
}
