//! Procedural tileable textures for the surface finishes (phase 13).
//!
//! Pure and deterministic: the pixels come from integer hash noise (splitmix64 finalizer) of
//! `(finish, x, y)`, with no floats in the noise, no global state and no platform dependence.
//! Every texture is 256 x 256, opaque sRGB and tiles left/right and top/bottom.

use cadcraft_doc::arch::Finish;

/// Width and height of every generated texture, in pixels.
pub const TEXTURE_SIZE: u32 = 256;

/// A generated texture.
#[derive(Debug, Clone, PartialEq)]
pub struct Texture {
    /// The finish this texture shows.
    pub finish: Finish,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// sRGB pixels, 4 bytes each (opaque); row 0 is v at the top of a tile.
    pub rgba: Vec<u8>,
    /// Metres one repeat covers along u and v.
    pub tile: [f64; 2],
}

const N: usize = TEXTURE_SIZE as usize;

/// Splitmix64 finalizer.
fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn finish_salt(f: Finish) -> u64 {
    match f {
        Finish::Siding => 1,
        Finish::Shingles => 2,
        Finish::Brick => 3,
        Finish::Wood => 4,
        Finish::Concrete => 5,
        Finish::Tile => 6,
        Finish::Carpet => 7,
        Finish::Drywall => 8,
        Finish::Plaster => 9,
    }
}

/// Hash of `(salt, a, b, c)`.
fn hash(salt: u64, a: u64, b: u64, c: u64) -> u64 {
    mix(mix(mix(mix(salt) ^ a) ^ b.wrapping_add(0x1234_5678)) ^ c.wrapping_add(0x9876_5432))
}

/// A signed jitter in `-amp..=amp`.
fn jitter(h: u64, amp: i32) -> i32 {
    let span = (2 * amp + 1) as u64;
    (h % span) as i32 - amp
}

fn clamp8(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

fn shade(c: [i32; 3], d: i32) -> [i32; 3] {
    [c[0] + d, c[1] + d, c[2] + d]
}

/// The pixel of `finish` at `(x, y)` as an sRGB triple.
fn pixel(finish: Finish, x: usize, y: usize) -> [i32; 3] {
    let salt = finish_salt(finish);
    let noise = |amp: i32| jitter(hash(salt, x as u64, y as u64, 0), amp);
    match finish {
        Finish::Siding => {
            let board = y / 32;
            let row = y % 32;
            let mut d = jitter(hash(salt, board as u64, 1, 1), 4);
            if row >= 29 {
                d -= 38; // shadow line at the board's bottom edge
            } else if row < 3 {
                d += 14; // lighter just below the board above
            }
            // Faint horizontal grain: constant along u, varying with v.
            d += jitter(hash(salt, y as u64, 2, 2), 3);
            shade([150, 168, 184], d + noise(1))
        }
        Finish::Shingles => {
            let course = y / 32;
            let row = y % 32;
            let u6 = (x * 6 + if course % 2 == 1 { 256 } else { 0 }) % 1536;
            let tab = u6 / 512;
            let in_gap = u6 % 512 < 12;
            if row >= 30 || in_gap {
                return shade([28, 25, 24], noise(2));
            }
            let h = hash(salt, course as u64, tab as u64, 3);
            let d = jitter(h, 16);
            let warm = jitter(h >> 20, 5);
            shade([84 + warm, 78, 74 - warm], d + noise(3))
        }
        Finish::Brick => {
            let course = y / 32;
            let row = y % 32;
            let u = (x + if course % 2 == 1 { 32 } else { 0 }) % 256;
            let col = u % 64;
            if row < 4 || col < 3 {
                return shade([190, 190, 186], noise(4));
            }
            let brick = u / 64;
            let h = hash(salt, course as u64, brick as u64, 4);
            let d = jitter(h, 14);
            [168 + d + noise(4), 74 + d / 2 + noise(2), 58 + d / 2 + noise(2)]
        }
        Finish::Wood => {
            let plank = y * 6 / 256;
            let edge = (y * 6) % 256 < 6;
            let joint = (hash(salt, plank as u64, 9, 9) % 256) as usize;
            let rel = (x + 256 - joint) % 256;
            if edge || rel % 128 < 2 {
                return shade([70, 46, 28], noise(2));
            }
            let seg = (rel / 128) as u64;
            let h = hash(salt, plank as u64, seg, 5);
            let tone = jitter(h, 14);
            let g = hash(salt, plank as u64, y as u64, 6) % 5;
            let grain = if g == 0 { -14 } else { 0 };
            shade([168, 122, 78], tone + grain + noise(2))
        }
        Finish::Concrete => {
            let noise = |amp: i32| jitter(hash(salt, x as u64, y as u64, 0), amp);
            let fleck = (hash(salt, (x * 7 + y * 13) as u64, 11, 11) % 16) == 0;
            let base = if fleck { [132, 132, 126] } else { [150, 150, 146] };
            shade(base, noise(10))
        }
        Finish::Tile => {
            if x % 128 < 4 || y % 128 < 4 {
                return shade([160, 158, 152], jitter(hash(salt, 0, 0, 11), 2));
            }
            shade([226, 223, 216], jitter(hash(salt, x as u64, y as u64, 0), 3))
        }
        Finish::Carpet => shade([176, 160, 138], jitter(hash(salt, x as u64, y as u64, 0), 8)),
        Finish::Drywall => [232, 231, 228],
        Finish::Plaster => [234, 234, 232],
    }
}

/// Generate the texture for `finish`: the same bytes on every call and platform.
pub fn texture(finish: Finish) -> Texture {
    let mut rgba = Vec::with_capacity(N * N * 4);
    for y in 0..N {
        for x in 0..N {
            let p = pixel(finish, x, y);
            rgba.extend_from_slice(&[clamp8(p[0]), clamp8(p[1]), clamp8(p[2]), 255]);
        }
    }
    let tile = match finish {
        Finish::Siding => [1.2, 1.2],
        Finish::Shingles => [1.0, 1.0],
        Finish::Brick => [0.84, 0.6],
        Finish::Wood => [1.2, 0.84],
        Finish::Concrete => [2.0, 2.0],
        Finish::Tile => [0.6, 0.6],
        Finish::Carpet => [1.0, 1.0],
        Finish::Drywall => [1.0, 1.0],
        Finish::Plaster => [1.0, 1.0],
    };
    Texture { finish, width: TEXTURE_SIZE, height: TEXTURE_SIZE, rgba, tile }
}

/// The finish the boards of a deck show.
pub const DECK_BOARD_FINISH: Finish = Finish::Wood;

/// The finish a material shows without a `wall-` / `roof-` / `floor-` prefix: [`DECK_BOARD_FINISH`]
/// for the deck boards' material `deck` (any case), none for anything else (the deck's structure
/// stays untextured).
pub fn board_finish(material: &str) -> Option<Finish> {
    material.eq_ignore_ascii_case("deck").then_some(DECK_BOARD_FINISH)
}

/// Mean sRGB colour of the texture's pixels.
pub fn mean_srgb(t: &Texture) -> [u8; 3] {
    let n = (t.rgba.len() / 4) as u64;
    if n == 0 {
        return [0; 3];
    }
    let mut sum = [0u64; 3];
    for px in t.rgba.chunks_exact(4) {
        for (s, v) in sum.iter_mut().zip(px) {
            *s += u64::from(*v);
        }
    }
    [(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8]
}

fn srgb_to_linear(v: u8) -> f32 {
    let c = f32::from(v) / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// Mean linear-light colour of the texture's pixels.
pub fn mean_linear(t: &Texture) -> [f32; 3] {
    let n = t.rgba.len() / 4;
    if n == 0 {
        return [0.0; 3];
    }
    let mut sum = [0f64; 3];
    for px in t.rgba.chunks_exact(4) {
        for (s, v) in sum.iter_mut().zip(px) {
            *s += f64::from(srgb_to_linear(*v));
        }
    }
    let n = n as f64;
    [(sum[0] / n) as f32, (sum[1] / n) as f32, (sum[2] / n) as f32]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_finish_is_wood_for_deck_only() {
        assert_eq!(board_finish("deck"), Some(Finish::Wood));
        assert_eq!(board_finish("DECK"), Some(Finish::Wood));
        for other in ["deck-joist", "deck-post", "deck-footing", "floor", "wall", "", "decking"] {
            assert_eq!(board_finish(other), None, "{other}");
        }
    }

    fn lum(t: &Texture, x: usize, y: usize) -> i32 {
        let i = (y * N + x) * 4;
        let g = |k: usize| t.rgba.get(i + k).copied().map_or(0, i32::from);
        g(0) + g(1) + g(2)
    }

    /// Mean absolute luminance step to the neighbour at `(+dx, +dy)` (wrapping) over all pixels.
    fn mean_step(t: &Texture, dx: usize, dy: usize) -> f64 {
        let mut s = 0i64;
        for y in 0..N {
            for x in 0..N {
                s += i64::from((lum(t, x, y) - lum(t, (x + dx) % N, (y + dy) % N)).abs());
            }
        }
        s as f64 / (N * N) as f64
    }

    #[test]
    fn textures_are_deterministic_and_256_square() {
        for f in Finish::ALL {
            let a = texture(f);
            let b = texture(f);
            assert_eq!(a, b);
            assert_eq!((a.width, a.height), (256, 256));
            assert_eq!(a.rgba.len(), 256 * 256 * 4);
            assert!(a.rgba.chunks_exact(4).all(|p| p[3] == 255));
        }
        assert_ne!(texture(Finish::Brick).rgba, texture(Finish::Wood).rgba);
        assert_eq!(texture(Finish::Brick).tile, [0.84, 0.6]);
    }

    #[test]
    fn textures_tile_seamlessly() {
        for f in Finish::ALL {
            let t = texture(f);
            let mut seam_u = 0f64;
            let mut seam_v = 0f64;
            for i in 0..N {
                seam_u += f64::from((lum(&t, N - 1, i) - lum(&t, 0, i)).abs());
                seam_v += f64::from((lum(&t, i, N - 1) - lum(&t, i, 0)).abs());
            }
            seam_u /= N as f64;
            seam_v /= N as f64;
            // The wrap seam must be no worse than the worst interior column/row step.
            let mut worst_u = 0f64;
            let mut worst_v = 0f64;
            for k in 0..N - 1 {
                let (mut su, mut sv) = (0f64, 0f64);
                for i in 0..N {
                    su += f64::from((lum(&t, k, i) - lum(&t, k + 1, i)).abs());
                    sv += f64::from((lum(&t, i, k) - lum(&t, i, k + 1)).abs());
                }
                worst_u = worst_u.max(su / N as f64);
                worst_v = worst_v.max(sv / N as f64);
            }
            assert!(seam_u <= worst_u + 1.0, "{f:?} u seam {seam_u} vs {worst_u}");
            assert!(seam_v <= worst_v + 1.0, "{f:?} v seam {seam_v} vs {worst_v}");
        }
    }

    #[test]
    fn siding_varies_along_v_more_than_u() {
        let t = texture(Finish::Siding);
        let du = mean_step(&t, 1, 0);
        let dv = mean_step(&t, 0, 1);
        assert!(dv > 2.0 * du, "du {du} dv {dv}");
    }

    #[test]
    fn brick_has_mortar_rows_and_joints() {
        let t = texture(Finish::Brick);
        for course in 0..8 {
            let mortar = lum(&t, 30, course * 32 + 1);
            let brick = lum(&t, 30, course * 32 + 16);
            assert!(mortar > brick + 100, "course {course}: mortar {mortar} brick {brick}");
        }
        // Course 0 has its head joint at x = 0..3, course 1 (offset half a brick) at x = 32.
        assert!(lum(&t, 1, 16) > lum(&t, 30, 16) + 100);
        assert!(lum(&t, 33, 48) > lum(&t, 1, 48) + 100);
    }

    #[test]
    fn mean_colours_are_in_range() {
        for f in Finish::ALL {
            let t = texture(f);
            let s = mean_srgb(&t);
            assert!(s.iter().all(|&c| (20..=235).contains(&c)), "{f:?} {s:?}");
            let l = mean_linear(&t);
            assert!(l.iter().all(|&c| c > 0.0 && c < 1.0), "{f:?} {l:?}");
        }
        let b = mean_srgb(&texture(Finish::Brick));
        assert!(b[0] > b[2], "brick is red-ish: {b:?}");
    }

    #[test]
    fn all_four_finishes_are_pairwise_distinct() {
        let textures: Vec<Texture> = Finish::ALL.iter().copied().map(texture).collect();
        for i in 0..textures.len() {
            for j in i + 1..textures.len() {
                assert_ne!(textures[i].rgba, textures[j].rgba, "finishes {} and {} are identical", i, j);
            }
        }
    }

    #[test]
    fn tile_values_match_the_spec() {
        assert_eq!(texture(Finish::Siding).tile, [1.2, 1.2]);
        assert_eq!(texture(Finish::Shingles).tile, [1.0, 1.0]);
        assert_eq!(texture(Finish::Brick).tile, [0.84, 0.6]);
        assert_eq!(texture(Finish::Wood).tile, [1.2, 0.84]);
        assert_eq!(texture(Finish::Concrete).tile, [2.0, 2.0]);
        assert_eq!(texture(Finish::Tile).tile, [0.6, 0.6]);
        assert_eq!(texture(Finish::Carpet).tile, [1.0, 1.0]);
        assert_eq!(texture(Finish::Drywall).tile, [1.0, 1.0]);
        assert_eq!(texture(Finish::Plaster).tile, [1.0, 1.0]);
    }

    #[test]
    fn mean_of_empty_texture_returns_zero() {
        let empty = Texture { finish: Finish::Siding, width: 0, height: 0, rgba: Vec::new(), tile: [1.0, 1.0] };
        assert_eq!(mean_srgb(&empty), [0, 0, 0]);
        assert_eq!(mean_linear(&empty), [0.0, 0.0, 0.0]);
    }

    #[test]
    fn shingles_have_dark_gaps_between_courses() {
        let t = texture(Finish::Shingles);
        // 8 courses of 32 pixels; the gap is at the bottom of each course (row >= 30).
        for course in 0..8 {
            let gap_y = course * 32 + 31;
            let tab_y = course * 32 + 15;
            let gap_lum = lum(&t, 100, gap_y);
            let tab_lum = lum(&t, 100, tab_y);
            assert!(gap_lum < tab_lum, "gap at y={} should be darker than tab at y={}", gap_y, tab_y);
        }
    }

    #[test]
    fn wood_has_staggered_end_joints() {
        let t = texture(Finish::Wood);
        // 6 planks, each about 42.67 pixels high. Each plank has a dark end joint.
        let plank_height = N / 6;
        for plank in 0..6 {
            let y = plank * plank_height + plank_height / 2;
            let has_joint = (0..N).any(|x| {
                let i = (y * N + x) * 4;
                let r = t.rgba[i] as i32;
                let g = t.rgba[i + 1] as i32;
                let b = t.rgba[i + 2] as i32;
                r + g + b < 200
            });
            assert!(has_joint, "plank {} should have a dark end joint", plank);
        }
    }

    #[test]
    fn tile_has_grout_lines_every_128_px() {
        let t = texture(Finish::Tile);
        // Grout runs at the first 4 pixels of every 128 px tile: x = 0..=3 and x = 128..=131,
        // and likewise for y. Grout is the dark grey (160, 158, 152); glaze is off-white
        // (226, 223, 216).
        for x in [0u32, 1, 2, 3, 128, 129, 130, 131] {
            let i = (128 * N + x as usize) * 4;
            let (r, g, b) = (t.rgba[i] as i32, t.rgba[i + 1] as i32, t.rgba[i + 2] as i32);
            assert!((r - 160).abs() < 20, "grout row x={} r={}", x, r);
            assert!((g - 158).abs() < 20, "grout row x={} g={}", x, g);
            assert!((b - 152).abs() < 20, "grout row x={} b={}", x, b);
        }
        // Non-grout pixels are off-white.
        for (x, y) in [(64, 64), (192, 64), (64, 192), (192, 192)] {
            let i = (y * N + x) * 4;
            let (r, g, b) = (t.rgba[i] as i32, t.rgba[i + 1] as i32, t.rgba[i + 2] as i32);
            assert!((r - 226).abs() < 8, "glaze x={}y={} r={}", x, y, r);
            assert!((g - 223).abs() < 8, "glaze x={}y={} g={}", x, y, g);
            assert!((b - 216).abs() < 8, "glaze x={}y={} b={}", x, y, b);
        }
    }

    #[test]
    fn carpet_and_concrete_are_noisy() {
        // Carpet and concrete have per-pixel noise (±8 and ±10).
        for f in [Finish::Carpet, Finish::Concrete] {
            let t = texture(f);
            let mut min_lum = i32::MAX;
            let mut max_lum = i32::MIN;
            for y in 0..N {
                for x in 0..N {
                    let i = (y * N + x) * 4;
                    let l = t.rgba[i] as i32 + t.rgba[i + 1] as i32 + t.rgba[i + 2] as i32;
                    min_lum = min_lum.min(l);
                    max_lum = max_lum.max(l);
                }
            }
            // The spread must be wider than a flat texture would produce.
            let spread = max_lum - min_lum;
            assert!(spread > 12, "{f:?} luminance spread {spread}");
        }
        // Drywall and plaster are flat (single colour, no noise).
        for f in [Finish::Drywall, Finish::Plaster] {
            let t = texture(f);
            let first = t.rgba[0];
            assert!(t.rgba.iter().all(|&v| v == first), "{f:?} is not flat");
        }
    }

    #[test]
    fn drywall_and_plaster_are_flat() {
        // Drywall: exactly (232, 231, 228, 255) every pixel.
        let d = texture(Finish::Drywall);
        for (y, x) in [(0u32, 0u32), (1, 1), (128, 128), (255, 255), (42, 200)] {
            let i = (y as usize * N + x as usize) * 4;
            assert_eq!([d.rgba[i], d.rgba[i + 1], d.rgba[i + 2], d.rgba[i + 3]], [232, 231, 228, 255], "{y},{x}");
        }
        // Plaster: exactly (234, 234, 232, 255) every pixel.
        let p = texture(Finish::Plaster);
        for (y, x) in [(0u32, 0u32), (1, 1), (128, 128), (255, 255), (42, 200)] {
            let i = (y as usize * N + x as usize) * 4;
            assert_eq!([p.rgba[i], p.rgba[i + 1], p.rgba[i + 2], p.rgba[i + 3]], [234, 234, 232, 255], "{y},{x}");
        }
    }
}
