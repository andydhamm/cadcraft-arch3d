//! Deterministic icons rendered on the CPU (phase 14).
//!
//! Library tiles are drawn by a small software rasterizer, so an icon is the same bytes on every
//! machine and in the web build: no GPU, no threads, no files, no hash-map order. The camera is
//! orthographic and looks at the bounding-box centre of what it draws from azimuth `yaw` and
//! elevation `pitch`; the light is fixed and every face is flat-shaded with its own normal.
//! Vertices are snapped to a 1/256-pixel grid and covered with exact integer edge functions (the
//! top-left rule over pixel centres); depth and texture coordinates are interpolated in f64 and the
//! depth is kept in an f32 z-buffer.
//!
//! The arithmetic is IEEE-exact operations only (`+ - * /`, `sqrt`, `floor`, `round`, `%`, casts),
//! in a fixed order. Sines, cosines and the sRGB curve are computed here from those operations,
//! never by the platform's maths library, whose last bit differs between targets: the same input
//! gives the same bytes everywhere.

use crate::mesh::Mesh;
use cadcraft_doc::arch::{ImageRgba, SymbolMesh};

/// The library's icon size in pixels.
pub const ICON_SIZE: u32 = 128;
/// The smallest icon size [`IconView::new`] accepts.
pub const MIN_ICON_SIZE: u32 = 16;
/// The largest icon size [`IconView::new`] accepts.
pub const MAX_ICON_SIZE: u32 = 512;
/// The default camera azimuth in degrees.
pub const ICON_YAW: f64 = -30.0;
/// The default camera elevation in degrees.
pub const ICON_PITCH: f64 = 25.0;
/// The largest camera elevation either way, in degrees (straight down or straight up).
pub const MAX_PITCH: f64 = 90.0;
/// The smallest zoom [`IconView::new`] accepts.
pub const MIN_ZOOM: f64 = 0.25;
/// The largest zoom [`IconView::new`] accepts.
pub const MAX_ZOOM: f64 = 8.0;
/// Segments around the material sphere.
pub const SPHERE_SEGMENTS: u32 = 48;
/// Rings of the material sphere, from the top.
pub const SPHERE_RINGS: u32 = 24;

/// Towards the light, before normalising.
const LIGHT: [f64; 3] = [-0.4, -0.6, 0.7];
/// Shade of a face turned away from the light.
const AMBIENT: f64 = 0.35;
/// Shade added by a face turned fully to the light.
const DIFFUSE: f64 = 0.65;
/// The bounding sphere's diameter as a fraction of the square, at zoom 1.
const FILL: f64 = 0.9;
/// Sub-pixel steps per pixel of the fixed-point grid.
const SUB: i64 = 256;
/// A vertex whose fixed-point coordinate is beyond this is dropped, so the edge products stay far
/// inside `i64` (a checked view never gets near it).
const MAX_FIXED: f64 = 16_777_216.0;

/// How an icon is looked at.
///
/// Build it with [`IconView::new`], which refuses bad values, or take `IconView::default()`.
/// Every renderer here draws an empty (0 x 0) image for a view `IconView::new` would refuse.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IconView {
    /// Width and height in pixels, 16..=512 (default 128).
    pub size: u32,
    /// Camera azimuth in degrees: 0 looks from -y (a symbol's front), counter-clockwise positive;
    /// any finite angle, taken modulo 360 (default -30).
    pub yaw: f64,
    /// Camera elevation above the horizon in degrees, -90..=90 (default 25).
    pub pitch: f64,
    /// 0.25..=8 (default 1): the bounding sphere's diameter is 90 % of the square divided by `zoom`.
    pub zoom: f64,
}

impl Default for IconView {
    fn default() -> Self {
        IconView { size: ICON_SIZE, yaw: ICON_YAW, pitch: ICON_PITCH, zoom: 1.0 }
    }
}

impl IconView {
    /// A view, or the reason the values are refused: a size outside 16..=512, a non-finite yaw, a
    /// pitch outside -90..=90 or a zoom outside 0.25..=8 (NaN and infinities included).
    pub fn new(size: u32, yaw: f64, pitch: f64, zoom: f64) -> Result<IconView, String> {
        if !(MIN_ICON_SIZE..=MAX_ICON_SIZE).contains(&size) {
            return Err(format!("icon size {size} is out of range ({MIN_ICON_SIZE}..={MAX_ICON_SIZE} pixels)"));
        }
        if !yaw.is_finite() {
            return Err(format!("icon yaw {yaw} is not a finite angle"));
        }
        if !(-MAX_PITCH..=MAX_PITCH).contains(&pitch) {
            return Err(format!("icon pitch {pitch} is out of range (-{MAX_PITCH}..={MAX_PITCH} degrees)"));
        }
        if !(MIN_ZOOM..=MAX_ZOOM).contains(&zoom) {
            return Err(format!("icon zoom {zoom} is out of range ({MIN_ZOOM}..={MAX_ZOOM})"));
        }
        Ok(IconView { size, yaw, pitch, zoom })
    }

    /// `self` when `IconView::new` accepts its values.
    fn checked(&self) -> Option<IconView> {
        IconView::new(self.size, self.yaw, self.pitch, self.zoom).ok()
    }
}

/// Render `meshes` (world space, z up) into a transparent square, the view's size each way.
///
/// `colour_of` gives a mesh's linear RGB colour and `texture_of` its texture, if any: a textured
/// mesh shows each texel (decoded from sRGB) times its colour, sampled nearest at its UVs, which
/// wrap; a mesh without UVs for a triangle draws that triangle in its colour. Each face is shaded
/// `colour x (0.35 + 0.65 * max(0, n . L))` with `n` its normal (turned towards the camera when the
/// face is seen from behind) and `L` the fixed light (-0.4, -0.6, 0.7) normalised. Covered pixels
/// are opaque sRGB; texture alpha is not used. Non-finite vertices, out-of-range indices and
/// degenerate triangles are skipped; no meshes, or nothing with a size, gives a transparent image.
///
/// The textures live for `'t`, not for the mesh borrow, so `texture_of` can hand out textures from
/// elsewhere, such as `|m| scene_textures.get(&m.material)`.
pub fn render_meshes<'t>(
    meshes: &[Mesh],
    colour_of: &dyn Fn(&Mesh) -> [f32; 3],
    texture_of: &dyn Fn(&Mesh) -> Option<&'t ImageRgba>,
    view: &IconView,
) -> ImageRgba {
    let draws: Vec<Draw<'_>> = meshes
        .iter()
        .map(|m| Draw { positions: &m.positions, uvs: &m.uvs, indices: &m.indices, colour: widen(colour_of(m)), texture: texture_of(m) })
        .collect();
    render(&draws, view)
}

/// Render a library symbol with its parts' base colours and textures (as [`render_meshes`]).
/// A part's texture index that names no texture draws the part untextured.
pub fn render_symbol(symbol: &SymbolMesh, view: &IconView) -> ImageRgba {
    let draws: Vec<Draw<'_>> = symbol
        .parts
        .iter()
        .map(|p| Draw {
            positions: &p.positions,
            uvs: &p.uvs,
            indices: &p.indices,
            colour: [f64::from(p.base_color[0]), f64::from(p.base_color[1]), f64::from(p.base_color[2])],
            texture: p.texture.and_then(|t| symbol.textures.get(t as usize)),
        })
        .collect();
    render(&draws, view)
}

/// Render a material on a unit UV sphere: 48 segments around (u = 0 at +x, growing
/// counter-clockwise) and 24 rings (v = 0 at the top). `texture` is multiplied by `colour`, so
/// `[1, 1, 1]` shows it as it is; without a texture the sphere is `colour` (linear RGB).
pub fn render_sphere(texture: Option<&ImageRgba>, colour: [f32; 3], view: &IconView) -> ImageRgba {
    let sphere = UvSphere::new();
    let draw = Draw { positions: &sphere.positions, uvs: &sphere.uvs, indices: &sphere.indices, colour: widen(colour), texture };
    render(&[draw], view)
}

/// `image` scaled to fit a `size` x `size` square, centred, with transparent margins (letterboxed
/// or pillarboxed); every output pixel is the exact area average of the source pixels it covers,
/// alpha-weighted, in sRGB bytes. An invalid image gives a transparent square, and a `size`
/// outside 16..=512 an empty (0 x 0) image.
pub fn thumbnail(image: &ImageRgba, size: u32) -> ImageRgba {
    let Some(n) = side(size) else {
        return ImageRgba::default();
    };
    let mut rgba = vec![0u8; n * n * 4];
    if image.is_valid() {
        let (w, h) = (image.width as usize, image.height as usize);
        let (cw, ch) = if w >= h { (n, ((h * n + w / 2) / w).max(1)) } else { (((w * n + h / 2) / h).max(1), n) };
        let (x0, y0) = ((n - cw) / 2, (n - ch) / 2);
        let cols = spans(w, cw);
        let rows = spans(h, ch);
        for (oy, row) in rows.iter().enumerate() {
            for (ox, col) in cols.iter().enumerate() {
                let px = area_average(image, row, col, (w * h) as u64);
                let i = ((y0 + oy) * n + x0 + ox) * 4;
                if let Some(dst) = rgba.get_mut(i..i + 4) {
                    dst.copy_from_slice(&px);
                }
            }
        }
    }
    square(size, rgba)
}

/// An opaque `size` x `size` square shading from `top` (row 0) to `bottom` (the last row), mixed in
/// sRGB bytes and rounded. A `size` outside 16..=512 gives an empty (0 x 0) image.
pub fn gradient(top: [u8; 3], bottom: [u8; 3], size: u32) -> ImageRgba {
    let Some(n) = side(size) else {
        return ImageRgba::default();
    };
    let span = size - 1;
    let mut rgba = Vec::with_capacity(n * n * 4);
    for y in 0..size {
        let mix = |t: u8, b: u8| u8::try_from((u32::from(t) * (span - y) + u32::from(b) * y + span / 2) / span).unwrap_or(u8::MAX);
        let px = [mix(top[0], bottom[0]), mix(top[1], bottom[1]), mix(top[2], bottom[2]), 255];
        rgba.extend_from_slice(&px.repeat(n));
    }
    square(size, rgba)
}

/// An opaque `size` x `size` square of `colour` (sRGB). A `size` outside 16..=512 gives an empty
/// (0 x 0) image.
pub fn solid(colour: [u8; 3], size: u32) -> ImageRgba {
    let Some(n) = side(size) else {
        return ImageRgba::default();
    };
    let px = [colour[0], colour[1], colour[2], 255];
    square(size, px.repeat(n * n))
}

/// The side in pixels when `size` is an accepted icon size.
fn side(size: u32) -> Option<usize> {
    (MIN_ICON_SIZE..=MAX_ICON_SIZE).contains(&size).then_some(size as usize)
}

/// A `size` x `size` image of `rgba`.
fn square(size: u32, rgba: Vec<u8>) -> ImageRgba {
    let header = ImageRgba { width: size, height: size, ..Default::default() };
    ImageRgba { rgba, ..header }
}

fn widen(c: [f32; 3]) -> [f64; 3] {
    [f64::from(c[0]), f64::from(c[1]), f64::from(c[2])]
}

/// The source pixels one output pixel covers along one axis: the first index and the overlap of
/// each, in units of 1 / `out` source pixel (the overlaps of one output pixel sum to `src`).
struct Span {
    first: usize,
    weights: Vec<u64>,
}

/// Spans of the `out` output pixels over `src` source pixels.
fn spans(src: usize, out: usize) -> Vec<Span> {
    (0..out)
        .map(|o| {
            let (lo, hi) = (o * src, (o + 1) * src);
            let first = lo / out;
            let weights = (first..hi.div_ceil(out)).map(|s| hi.min((s + 1) * out).saturating_sub(lo.max(s * out)) as u64).collect();
            Span { first, weights }
        })
        .collect()
}

/// The alpha-weighted area average of the source pixels under `row` x `col`; `total` is the sum
/// of their weights.
fn area_average(image: &ImageRgba, row: &Span, col: &Span, total: u64) -> [u8; 4] {
    let w = image.width as usize;
    let mut alpha = 0u64;
    let mut colour = [0u64; 3];
    for (dy, wy) in row.weights.iter().enumerate() {
        for (dx, wx) in col.weights.iter().enumerate() {
            let i = ((row.first + dy) * w + col.first + dx) * 4;
            let Some(&[r, g, b, a]) = image.rgba.get(i..i + 4) else {
                continue;
            };
            let weight = wy * wx * u64::from(a);
            alpha += weight;
            for (sum, v) in colour.iter_mut().zip([r, g, b]) {
                *sum += weight * u64::from(v);
            }
        }
    }
    let round = |num: u64, den: u64| (num + den / 2).checked_div(den).map_or(0, |q| u8::try_from(q).unwrap_or(u8::MAX));
    [round(colour[0], alpha), round(colour[1], alpha), round(colour[2], alpha), round(alpha, total)]
}

/// One material-uniform batch of triangles.
struct Draw<'a> {
    positions: &'a [[f32; 3]],
    uvs: &'a [[f32; 2]],
    indices: &'a [u32],
    /// Linear RGB.
    colour: [f64; 3],
    texture: Option<&'a ImageRgba>,
}

/// Draw `draws` with `view`.
fn render(draws: &[Draw<'_>], view: &IconView) -> ImageRgba {
    let Some(view) = view.checked() else {
        return ImageRgba::default();
    };
    let mut canvas = Canvas::new(view.size);
    if let Some(camera) = Camera::fit(draws, &view) {
        let srgb = Srgb::new();
        for draw in draws {
            canvas.draw(draw, &camera, &srgb);
        }
    }
    square(view.size, canvas.rgba)
}

/// A finite vertex in f64.
fn finite(p: &[f32; 3]) -> Option<[f64; 3]> {
    p.iter().all(|v| v.is_finite()).then(|| widen(*p))
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// `v` scaled to unit length; `None` for a zero or non-finite vector.
fn normalise(v: [f64; 3]) -> Option<[f64; 3]> {
    let len = dot(v, v).sqrt();
    (len.is_finite() && len > 0.0).then(|| [v[0] / len, v[1] / len, v[2] / len])
}

/// Sine and cosine of `deg` degrees from exact operations: reduced modulo 360, then to within 45
/// degrees of a quarter turn, then a Taylor series (error below 1e-17). Non-finite angles give
/// (0, 1). Equal angles modulo 360 (-30 and 330) give the same bits.
fn sin_cos_deg(deg: f64) -> (f64, f64) {
    if !deg.is_finite() {
        return (0.0, 1.0);
    }
    let mut d = deg % 360.0;
    if d < 0.0 {
        d += 360.0;
    }
    let quadrant = ((d + 45.0) / 90.0).floor();
    let x = (d - 90.0 * quadrant) * (std::f64::consts::PI / 180.0);
    let x2 = x * x;
    let (mut s, mut c) = (1.0, 1.0);
    for k in (1..=8u32).rev() {
        let k = f64::from(k);
        s = 1.0 - x2 / ((2.0 * k) * (2.0 * k + 1.0)) * s;
        c = 1.0 - x2 / ((2.0 * k - 1.0) * (2.0 * k)) * c;
    }
    let s = x * s;
    match (quadrant as i64).rem_euclid(4) {
        0 => (s, c),
        1 => (c, -s),
        2 => (-s, -c),
        _ => (-c, s),
    }
}

/// `y^2.4` for `y` in 0..=1 from multiplications and comparisons only: the `t` in 0..=1 with
/// `t^5 = y^12`, by bisection.
fn pow_2_4(y: f64) -> f64 {
    let y2 = y * y;
    let y4 = y2 * y2;
    let y12 = y4 * y4 * y4;
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    for _ in 0..64 {
        let mid = 0.5 * (lo + hi);
        let m2 = mid * mid;
        if m2 * m2 * mid < y12 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// The sRGB transfer function, sRGB to linear (`s` in 0..=1).
fn srgb_to_linear(s: f64) -> f64 {
    if s <= 0.04045 { s / 12.92 } else { pow_2_4((s + 0.055) / 1.055) }
}

/// The sRGB curve as tables built with exact arithmetic.
struct Srgb {
    /// The linear value of each sRGB byte.
    decode: [f64; 256],
    /// `steps[b]`: the linear value half-way between bytes `b` and `b + 1`, where the exact
    /// piecewise linear-to-sRGB formula reaches `(b + 0.5) / 255`.
    steps: [f64; 255],
}

impl Srgb {
    fn new() -> Srgb {
        let mut decode = [0.0; 256];
        for (b, d) in decode.iter_mut().enumerate() {
            *d = srgb_to_linear(b as f64 / 255.0);
        }
        let mut steps = [0.0; 255];
        for (b, s) in steps.iter_mut().enumerate() {
            *s = srgb_to_linear((b as f64 + 0.5) / 255.0);
        }
        Srgb { decode, steps }
    }

    /// The linear value of sRGB byte `b`.
    fn linear(&self, b: u8) -> f64 {
        self.decode[usize::from(b)]
    }

    /// Linear `c` as an sRGB byte, rounded to nearest (half up); 0 below 0 and for NaN, 255 from 1.
    fn encode(&self, c: f64) -> u8 {
        u8::try_from(self.steps.partition_point(|s| *s <= c)).unwrap_or(u8::MAX)
    }
}

/// An orthographic camera fitted to what it draws.
struct Camera {
    centre: [f64; 3],
    right: [f64; 3],
    up: [f64; 3],
    /// Towards the camera.
    back: [f64; 3],
    /// Pixels per metre.
    scale: f64,
    /// Half the image side in pixels.
    half: f64,
    /// Towards the light, unit length.
    light: [f64; 3],
}

impl Camera {
    /// The camera of `view` around the finite vertices of `draws`; `None` when there are none or
    /// they are all one point.
    fn fit(draws: &[Draw<'_>], view: &IconView) -> Option<Camera> {
        let points = || draws.iter().flat_map(|d| d.positions.iter()).filter_map(finite);
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        let mut any = false;
        for p in points() {
            any = true;
            for ((l, h), v) in lo.iter_mut().zip(hi.iter_mut()).zip(p) {
                if v < *l {
                    *l = v;
                }
                if v > *h {
                    *h = v;
                }
            }
        }
        if !any {
            return None;
        }
        let centre = [0.5 * (lo[0] + hi[0]), 0.5 * (lo[1] + hi[1]), 0.5 * (lo[2] + hi[2])];
        let mut r2 = 0.0f64;
        for p in points() {
            let d = sub(p, centre);
            let d2 = dot(d, d);
            if d2 > r2 {
                r2 = d2;
            }
        }
        let radius = r2.sqrt();
        let sized = radius.is_finite() && radius > 0.0;
        if !sized {
            return None;
        }
        let size = f64::from(view.size);
        let (sy, cy) = sin_cos_deg(view.yaw);
        let (sp, cp) = sin_cos_deg(view.pitch);
        Some(Camera {
            centre,
            right: [cy, sy, 0.0],
            up: [-sp * sy, sp * cy, cp],
            back: [sy * cp, -cy * cp, sp],
            scale: FILL * size / (2.0 * radius * view.zoom),
            half: 0.5 * size,
            light: normalise(LIGHT).unwrap_or([0.0, 0.0, 1.0]),
        })
    }

    /// `p` on the fixed-point pixel grid (x right, y down) and its depth (larger is farther).
    fn project(&self, p: [f64; 3]) -> Option<(i64, i64, f64)> {
        let d = sub(p, self.centre);
        let x = ((self.half + dot(d, self.right) * self.scale) * SUB as f64).round();
        let y = ((self.half - dot(d, self.up) * self.scale) * SUB as f64).round();
        let on_grid = x.abs() <= MAX_FIXED && y.abs() <= MAX_FIXED;
        if !on_grid {
            return None;
        }
        Some((x as i64, y as i64, -dot(d, self.back)))
    }
}

/// A projected triangle corner.
#[derive(Clone, Copy)]
struct Corner {
    x: i64,
    y: i64,
    depth: f64,
    uv: [f64; 2],
}

/// Twice the signed area of `(a, b, p)` on the fixed-point grid; positive when `p` is to the right
/// of `a -> b` on screen.
fn edge(a: &Corner, b: &Corner, p: (i64, i64)) -> i64 {
    (b.x - a.x) * (p.1 - a.y) - (b.y - a.y) * (p.0 - a.x)
}

/// Whether `a -> b` is a top or a left edge of a triangle with positive area.
fn top_left(a: &Corner, b: &Corner) -> bool {
    b.y < a.y || (b.y == a.y && b.x > a.x)
}

/// How a triangle's pixels get their colour.
enum Fill<'a> {
    /// One sRGB colour.
    Flat([u8; 3]),
    /// Texels (sRGB, decoded) times a linear colour that includes the shade.
    Textured(&'a ImageRgba, [f64; 3]),
}

/// The colour and depth buffers.
struct Canvas {
    size: i64,
    rgba: Vec<u8>,
    depth: Vec<f32>,
}

impl Canvas {
    fn new(size: u32) -> Canvas {
        let n = size as usize;
        Canvas { size: i64::from(size), rgba: vec![0; n * n * 4], depth: vec![f32::INFINITY; n * n] }
    }

    /// Draw every triangle of `draw`.
    fn draw(&mut self, draw: &Draw<'_>, camera: &Camera, srgb: &Srgb) {
        let texture = draw.texture.filter(|t| t.is_valid());
        for &[i0, i1, i2] in draw.indices.as_chunks::<3>().0 {
            let point = |i: u32| draw.positions.get(i as usize).and_then(finite);
            let (Some(a), Some(b), Some(c)) = (point(i0), point(i1), point(i2)) else {
                continue;
            };
            let Some(mut n) = normalise(cross(sub(b, a), sub(c, a))) else {
                continue;
            };
            if dot(n, camera.back) < 0.0 {
                n = [-n[0], -n[1], -n[2]];
            }
            let lit = dot(n, camera.light);
            let shade = AMBIENT + DIFFUSE * if lit > 0.0 { lit } else { 0.0 };
            let lin = [draw.colour[0] * shade, draw.colour[1] * shade, draw.colour[2] * shade];
            let uv = |i: u32| draw.uvs.get(i as usize).map(|t| [f64::from(t[0]), f64::from(t[1])]);
            let uvs = (uv(i0), uv(i1), uv(i2));
            let (fill, uvs) = match (texture, uvs) {
                (Some(t), (Some(u0), Some(u1), Some(u2))) => (Fill::Textured(t, lin), [u0, u1, u2]),
                _ => (Fill::Flat([srgb.encode(lin[0]), srgb.encode(lin[1]), srgb.encode(lin[2])]), [[0.0; 2]; 3]),
            };
            let corner = |p: [f64; 3], uv: [f64; 2]| camera.project(p).map(|(x, y, depth)| Corner { x, y, depth, uv });
            let (Some(ca), Some(cb), Some(cc)) = (corner(a, uvs[0]), corner(b, uvs[1]), corner(c, uvs[2])) else {
                continue;
            };
            self.fill([ca, cb, cc], &fill, srgb);
        }
    }

    /// Cover the pixel centres inside the triangle (top-left rule), nearest depth wins.
    fn fill(&mut self, corners: [Corner; 3], fill: &Fill<'_>, srgb: &Srgb) {
        let [a, b, c] = corners;
        let area = edge(&a, &b, (c.x, c.y));
        let (b, c, area) = if area < 0 { (c, b, -area) } else { (b, c, area) };
        if area == 0 {
            return;
        }
        let last = self.size - 1;
        let x_lo = (a.x.min(b.x).min(c.x) - SUB / 2).div_euclid(SUB).max(0);
        let x_hi = (a.x.max(b.x).max(c.x) - SUB / 2).div_euclid(SUB).min(last);
        let y_lo = (a.y.min(b.y).min(c.y) - SUB / 2).div_euclid(SUB).max(0);
        let y_hi = (a.y.max(b.y).max(c.y) - SUB / 2).div_euclid(SUB).min(last);
        let rules = [top_left(&b, &c), top_left(&c, &a), top_left(&a, &b)];
        let area = area as f64;
        for py in y_lo..=y_hi {
            for px in x_lo..=x_hi {
                let p = (px * SUB + SUB / 2, py * SUB + SUB / 2);
                let w = [edge(&b, &c, p), edge(&c, &a, p), edge(&a, &b, p)];
                if !w.iter().zip(rules).all(|(w, rule)| *w > 0 || (*w == 0 && rule)) {
                    continue;
                }
                let l = [w[0] as f64 / area, w[1] as f64 / area, w[2] as f64 / area];
                let depth = (l[0] * a.depth + l[1] * b.depth + l[2] * c.depth) as f32;
                let i = (py * self.size + px) as usize;
                let Some(z) = self.depth.get_mut(i) else {
                    continue;
                };
                let nearer = depth < *z;
                if !nearer {
                    continue;
                }
                *z = depth;
                let rgb = match fill {
                    Fill::Flat(rgb) => *rgb,
                    Fill::Textured(texture, lin) => {
                        let u = l[0] * a.uv[0] + l[1] * b.uv[0] + l[2] * c.uv[0];
                        let v = l[0] * a.uv[1] + l[1] * b.uv[1] + l[2] * c.uv[1];
                        let t = texel(texture, srgb, u, v);
                        [srgb.encode(t[0] * lin[0]), srgb.encode(t[1] * lin[1]), srgb.encode(t[2] * lin[2])]
                    }
                };
                if let Some(dst) = self.rgba.get_mut(i * 4..i * 4 + 4) {
                    dst.copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
                }
            }
        }
    }
}

/// The linear colour of the texel nearest `(u, v)` (wrapping; v = 0 is the top row) of a valid
/// texture.
fn texel(texture: &ImageRgba, srgb: &Srgb, u: f64, v: f64) -> [f64; 3] {
    let (w, h) = (i64::from(texture.width), i64::from(texture.height));
    let x = ((u * w as f64).floor() as i64).rem_euclid(w);
    let y = ((v * h as f64).floor() as i64).rem_euclid(h);
    let i = ((y * w + x) * 4) as usize;
    match texture.rgba.get(i..i + 3) {
        Some(&[r, g, b]) => [srgb.linear(r), srgb.linear(g), srgb.linear(b)],
        _ => [1.0; 3],
    }
}

/// The material sphere's vertex data.
struct UvSphere {
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

impl UvSphere {
    /// A unit sphere at the origin with a seam column (u = 0 and u = 1 at +x) and outward
    /// counter-clockwise triangles; the pole rows have one triangle per segment.
    fn new() -> UvSphere {
        let (segs, rings) = (SPHERE_SEGMENTS, SPHERE_RINGS);
        let mut positions = Vec::new();
        let mut uvs = Vec::new();
        for i in 0..=rings {
            let (st, ct) = sin_cos_deg(180.0 * f64::from(i) / f64::from(rings));
            for j in 0..=segs {
                let (sp, cp) = sin_cos_deg(360.0 * f64::from(j) / f64::from(segs));
                positions.push([(st * cp) as f32, (st * sp) as f32, ct as f32]);
                uvs.push([(f64::from(j) / f64::from(segs)) as f32, (f64::from(i) / f64::from(rings)) as f32]);
            }
        }
        let mut indices = Vec::new();
        let row = segs + 1;
        for i in 0..rings {
            for j in 0..segs {
                let (a, b) = (i * row + j, i * row + j + 1);
                let (c, d) = (a + row, b + row);
                if i + 1 < rings {
                    indices.extend_from_slice(&[a, c, d]);
                }
                if i > 0 {
                    indices.extend_from_slice(&[a, d, b]);
                }
            }
        }
        UvSphere { positions, uvs, indices }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::ArchId;
    use cadcraft_doc::arch::{SymbolPart, fnv1a64_hex};

    /// An axis-aligned box, four vertices per face, counter-clockwise outward, UVs 0..1 per face.
    fn cuboid(material: &str, lo: [f32; 3], hi: [f32; 3]) -> Mesh {
        let ([x0, y0, z0], [x1, y1, z1]) = (lo, hi);
        let faces = [
            [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]],
            [[x1, y1, z0], [x0, y1, z0], [x0, y1, z1], [x1, y1, z1]],
            [[x0, y1, z0], [x0, y0, z0], [x0, y0, z1], [x0, y1, z1]],
            [[x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]],
            [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]],
            [[x0, y1, z0], [x1, y1, z0], [x1, y0, z0], [x0, y0, z0]],
        ];
        let mut m = Mesh::new(material, ArchId(0));
        for face in faces {
            let base = m.positions.len() as u32;
            m.positions.extend_from_slice(&face);
            m.uvs.extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
            m.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        m
    }

    /// The unit cube standing on the origin.
    fn cube() -> Mesh {
        cuboid("cube", [-0.5, -0.5, 0.0], [0.5, 0.5, 1.0])
    }

    fn white(_: &Mesh) -> [f32; 3] {
        [1.0; 3]
    }

    fn raw(w: u32, h: u32, rgba: Vec<u8>) -> ImageRgba {
        let header = ImageRgba { width: w, height: h, ..Default::default() };
        ImageRgba { rgba, ..header }
    }

    fn image(w: u32, h: u32, px: impl Fn(u32, u32) -> [u8; 4]) -> ImageRgba {
        let mut rgba = Vec::new();
        for y in 0..h {
            for x in 0..w {
                rgba.extend_from_slice(&px(x, y));
            }
        }
        raw(w, h, rgba)
    }

    fn checker() -> ImageRgba {
        image(8, 8, |x, y| if (x + y) % 2 == 0 { [230, 40, 30, 255] } else { [20, 90, 220, 255] })
    }

    fn pixel(img: &ImageRgba, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * img.width + x) * 4) as usize;
        [img.rgba[i], img.rgba[i + 1], img.rgba[i + 2], img.rgba[i + 3]]
    }

    fn opaque(img: &ImageRgba) -> Vec<(u32, u32, [u8; 4])> {
        let mut v = Vec::new();
        for y in 0..img.height {
            for x in 0..img.width {
                let p = pixel(img, x, y);
                if p[3] > 0 {
                    v.push((x, y, p));
                }
            }
        }
        v
    }

    /// (min x, min y, max x, max y) of the opaque pixels.
    fn extent(img: &ImageRgba) -> (u32, u32, u32, u32) {
        let o = opaque(img);
        let xs = o.iter().map(|p| p.0);
        let ys = o.iter().map(|p| p.1);
        (xs.clone().min().unwrap(), ys.clone().min().unwrap(), xs.max().unwrap(), ys.max().unwrap())
    }

    fn symbol_of(meshes: &[Mesh], colours: &[[f32; 4]], textures: Vec<ImageRgba>, texture_of: &[Option<u32>]) -> SymbolMesh {
        let parts = meshes
            .iter()
            .zip(colours)
            .zip(texture_of)
            .map(|((m, c), t)| SymbolPart {
                positions: m.positions.clone(),
                uvs: m.uvs.clone(),
                indices: m.indices.clone(),
                base_color: *c,
                texture: *t,
                ..Default::default()
            })
            .collect();
        SymbolMesh { parts, textures, ..Default::default() }
    }

    fn is_transparent(img: &ImageRgba, size: u32) -> bool {
        img.width == size && img.height == size && img.rgba.len() == (size * size * 4) as usize && img.rgba.iter().all(|b| *b == 0)
    }

    #[test]
    fn icons_are_byte_identical_twice() {
        let tex = checker();
        let scene = [cube(), cuboid("side", [0.6, -0.3, 0.0], [1.2, 0.3, 0.6])];
        let colour_of = |m: &Mesh| -> [f32; 3] { if m.material == "cube" { [0.8, 0.3, 0.2] } else { [1.0, 1.0, 1.0] } };
        let texture_of = |m: &Mesh| (m.material == "side").then_some(&tex);
        let view = IconView::default();
        let a = render_meshes(&scene, &colour_of, &texture_of, &view);
        let b = render_meshes(&scene, &colour_of, &texture_of, &view);
        assert_eq!(a, b);
        assert!(opaque(&a).len() > 1000);

        // A symbol is drawn exactly as the same parts given as meshes.
        let symbol = symbol_of(&scene, &[[0.8, 0.3, 0.2, 1.0], [1.0, 1.0, 1.0, 1.0]], vec![tex.clone()], &[None, Some(0)]);
        assert_eq!(render_symbol(&symbol, &view), render_symbol(&symbol, &view));
        assert_eq!(render_symbol(&symbol, &view), a);

        let turned = IconView::new(200, 75.0, -10.0, 1.5).unwrap();
        assert_eq!(render_sphere(Some(&tex), [1.0; 3], &turned), render_sphere(Some(&tex), [1.0; 3], &turned));
        assert_eq!(render_sphere(None, [0.2, 0.6, 0.3], &view), render_sphere(None, [0.2, 0.6, 0.3], &view));
        let wide = image(300, 70, |x, y| [(x % 256) as u8, (y * 3) as u8, 7, 255]);
        assert_eq!(thumbnail(&wide, 128), thumbnail(&wide, 128));
        assert_eq!(gradient([74, 128, 196], [214, 228, 240], 128), gradient([74, 128, 196], [214, 228, 240], 128));

        // The bytes themselves are fixed: only IEEE-exact arithmetic reaches them, so every
        // platform (and the web build) gives these hashes. A deliberate change to the renderer
        // updates them.
        let cube_icon = render_meshes(&[cube()], &white, &|_| None, &view);
        assert_eq!(fnv1a64_hex(&cube_icon.rgba), "06c63c221a71a1fd");
        assert_eq!(fnv1a64_hex(&render_sphere(Some(&tex), [1.0; 3], &view).rgba), "9979553391373a19");
    }

    #[test]
    fn icon_is_size_by_size_rgba() {
        let tex = checker();
        let symbol = symbol_of(&[cube()], &[[0.5, 0.5, 0.5, 1.0]], vec![], &[None]);
        let wide = image(90, 30, |_, _| [10, 20, 30, 255]);
        for size in [MIN_ICON_SIZE, 17, 64, ICON_SIZE, 200, MAX_ICON_SIZE] {
            let view = IconView::new(size, ICON_YAW, ICON_PITCH, 1.0).unwrap();
            let rendered = [
                render_meshes(&[cube()], &white, &|_| None, &view),
                render_meshes(&[], &white, &|_| None, &view),
                render_symbol(&symbol, &view),
                render_sphere(None, [0.5; 3], &view),
                render_sphere(Some(&tex), [1.0; 3], &view),
            ];
            for img in rendered.iter().chain([thumbnail(&wide, size), gradient([0; 3], [255; 3], size), solid([1, 2, 3], size)].iter()) {
                assert_eq!((img.width, img.height), (size, size));
                assert_eq!(img.rgba.len(), (size * size * 4) as usize);
                assert!(img.is_valid());
            }
            // Coverage is all or nothing: a pixel is either untouched or opaque.
            for img in &rendered {
                assert!(img.rgba.as_chunks::<4>().0.iter().all(|p| p[3] == 0 || p[3] == 255));
            }
        }
    }

    #[test]
    fn an_empty_scene_is_transparent() {
        let view = IconView::default();
        assert!(is_transparent(&render_meshes(&[], &white, &|_| None, &view), ICON_SIZE));
        assert!(is_transparent(&render_meshes(&[Mesh::new("none", ArchId(0))], &white, &|_| None, &view), ICON_SIZE));
        assert!(is_transparent(&render_symbol(&SymbolMesh::default(), &view), ICON_SIZE));

        // Nothing with a size, nothing finite and nothing indexable draw nothing either.
        let point = Mesh { positions: vec![[1.0, 2.0, 3.0]; 3], indices: vec![0, 1, 2], ..Mesh::new("point", ArchId(0)) };
        let nan = Mesh {
            positions: vec![[f32::NAN, 0.0, 0.0], [1.0, f32::INFINITY, 0.0], [0.0, 1.0, f32::NEG_INFINITY]],
            indices: vec![0, 1, 2],
            ..Mesh::new("nan", ArchId(0))
        };
        let flat =
            Mesh { positions: vec![[0.0, 0.0, 0.0], [1.0, 1.0, 1.0], [2.0, 2.0, 2.0]], indices: vec![0, 1, 2], ..Mesh::new("line", ArchId(0)) };
        let stray = Mesh {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
            indices: vec![0, 1, 7, 2, 1],
            ..Mesh::new("stray", ArchId(0))
        };
        let size = IconView::new(64, 10.0, 80.0, 2.0).unwrap();
        for m in [point, nan, flat, stray] {
            assert!(is_transparent(&render_meshes(std::slice::from_ref(&m), &white, &|_| None, &view), ICON_SIZE), "{}", m.material);
            assert!(is_transparent(&render_meshes(&[m], &white, &|_| None, &size), 64));
        }

        // A symbol whose parts have no triangles, and a texture index that names nothing.
        let hollow =
            SymbolMesh { parts: vec![SymbolPart { positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 1.0]], ..Default::default() }], ..Default::default() };
        assert!(is_transparent(&render_symbol(&hollow, &view), ICON_SIZE));
        let missing = symbol_of(&[cube()], &[[1.0; 4]], vec![], &[Some(9)]);
        assert_eq!(render_symbol(&missing, &view), render_meshes(&[cube()], &white, &|_| None, &view));

        // An invalid image thumbnails to a transparent square.
        let broken = raw(4, 4, vec![255; 7]);
        assert!(is_transparent(&thumbnail(&broken, ICON_SIZE), ICON_SIZE));
        assert!(is_transparent(&thumbnail(&ImageRgba::default(), 32), 32));
    }

    #[test]
    fn the_cube_fills_the_icon() {
        let img = render_meshes(&[cube()], &white, &|_| None, &IconView::default());
        let (x0, y0, x1, y1) = extent(&img);
        // The bounding sphere (radius sqrt(3) / 2) is 90 % of the square: the silhouette of the
        // unit cube at yaw -30, pitch 25 is about 91 x 99 pixels of 128, centred.
        let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
        assert!((88..=94).contains(&w), "width {w}");
        assert!((95..=102).contains(&h), "height {h}");
        assert!((x0 + x1) / 2 >= 61 && (x0 + x1) / 2 <= 66, "x centre {}", (x0 + x1) / 2);
        assert!((y0 + y1) / 2 >= 61 && (y0 + y1) / 2 <= 66, "y centre {}", (y0 + y1) / 2);
        assert!(x0 >= 6 && y0 >= 6 && x1 <= 121 && y1 <= 121, "a 5 % margin is left");
        // Its projected area is |e.x| + |e.y| + |e.z| = 1.66 m^2 of the 1.73 m wide square: 45 %.
        let covered = opaque(&img).len() as f64 / (128.0 * 128.0);
        assert!((0.42..0.48).contains(&covered), "coverage {covered}");

        // The camera sees the front (-y), the left (-x) and the top: three flat shades of the
        // light (-0.4, -0.6, 0.7), the top brightest. Expected bytes are the exact sRGB formula of
        // 0.35 + 0.65 n.L: top 0.8027 -> 231, front 0.7381 -> 223, left 0.6087 -> 205.
        let mut shades: Vec<[u8; 4]> = opaque(&img).into_iter().map(|p| p.2).collect();
        shades.sort();
        shades.dedup();
        assert_eq!(shades, vec![[205, 205, 205, 255], [223, 223, 223, 255], [231, 231, 231, 255]]);
        assert_eq!(pixel(&img, 64, y0 + 8), [231, 231, 231, 255], "the top is at the top");
        assert_eq!(pixel(&img, x0 + 4, 64), [205, 205, 205, 255], "the -x side is on the left");
        assert_eq!(pixel(&img, x1 - 4, 64), [223, 223, 223, 255], "the front is on the right");

        // Zoom divides: at zoom 2 the cube is half as wide, at 0.5 it overflows the square.
        let small = render_meshes(&[cube()], &white, &|_| None, &IconView::new(128, ICON_YAW, ICON_PITCH, 2.0).unwrap());
        let (sx0, _, sx1, _) = extent(&small);
        assert!(((sx1 - sx0 + 1) as i64 - (w as i64) / 2).abs() <= 2, "zoom 2 width {}", sx1 - sx0 + 1);
        let big = render_meshes(&[cube()], &white, &|_| None, &IconView::new(128, ICON_YAW, ICON_PITCH, 0.5).unwrap());
        assert_eq!(pixel(&big, 64, 64)[3], 255);
        assert_eq!(pixel(&big, 64, 0)[3], 255, "zoom 0.5 reaches the edge");

        // Seen from straight above the cube is a square of one shade (the top).
        let plan = render_meshes(&[cube()], &white, &|_| None, &IconView::new(128, 0.0, 90.0, 1.0).unwrap());
        let (px0, py0, px1, py1) = extent(&plan);
        assert_eq!(px1 - px0, py1 - py0);
        assert!(opaque(&plan).iter().all(|p| p.2 == [231, 231, 231, 255]));
        assert_eq!(opaque(&plan).len() as u32, (px1 - px0 + 1) * (py1 - py0 + 1));
    }

    #[test]
    fn yaw_turns_the_view() {
        // A red block on -x and a blue one on +x.
        let scene = [cuboid("red", [-1.5, -0.5, 0.0], [-0.5, 0.5, 1.0]), cuboid("blue", [0.5, -0.5, 0.0], [1.5, 0.5, 1.0])];
        let colour_of = |m: &Mesh| -> [f32; 3] { if m.material == "red" { [1.0, 0.0, 0.0] } else { [0.0, 0.0, 1.0] } };
        let view = |yaw: f64, pitch: f64| render_meshes(&scene, &colour_of, &|_| None, &IconView::new(128, yaw, pitch, 1.0).unwrap());
        let at = |yaw: f64| view(yaw, ICON_PITCH);
        let stats = |img: &ImageRgba| {
            let (mut red, mut blue) = ((0u64, 0u64), (0u64, 0u64));
            for (x, _, p) in opaque(img) {
                if p[0] > p[2] {
                    red = (red.0 + 1, red.1 + u64::from(x));
                } else {
                    blue = (blue.0 + 1, blue.1 + u64::from(x));
                }
            }
            (red.0, blue.0, red.1 / red.0.max(1), blue.1 / blue.0.max(1))
        };

        // From the front (yaw 0, from -y) -x is on the left; from behind (yaw 180) on the right.
        let (r, b, rx, bx) = stats(&at(0.0));
        assert!(r > 500 && b > 500 && rx < 60 && bx > 68, "front: {r} {b} {rx} {bx}");
        let (r, b, rx, bx) = stats(&at(180.0));
        assert!(r > 500 && b > 500 && rx > 68 && bx < 60, "back: {r} {b} {rx} {bx}");

        // Counter-clockwise is positive: level, yaw 90 looks from +x, so the blue block hides the
        // red one; yaw -90 looks from -x and the red block hides the blue one.
        let (r, b, _, _) = stats(&view(90.0, 0.0));
        assert!(r == 0 && b > 500, "from +x: red {r} blue {b}");
        let (r, b, _, _) = stats(&view(-90.0, 0.0));
        assert!(b == 0 && r > 500, "from -x: red {r} blue {b}");

        // A turn changes the picture; a whole turn does not.
        assert_ne!(at(-30.0), at(60.0));
        assert_eq!(at(-30.0), at(330.0));
        assert_eq!(at(-30.0), at(-390.0));
        assert_eq!(at(0.0), at(720.0));
    }

    #[test]
    fn sphere_icon_shows_its_texture() {
        // A two-row texture: red above the equator (v < 0.5), blue below.
        let banded = image(1, 2, |_, y| if y == 0 { [255, 0, 0, 255] } else { [0, 0, 255, 255] });
        let view = IconView::default();
        let img = render_sphere(Some(&banded), [1.0; 3], &view);

        // Round: the bounding sphere is the sphere, 90 % of the square across.
        let (x0, y0, x1, y1) = extent(&img);
        assert!((113..=117).contains(&(x1 - x0 + 1)) && (113..=117).contains(&(y1 - y0 + 1)), "{x0} {y0} {x1} {y1}");
        assert_eq!(pixel(&img, 64, 64)[3], 255);
        for (x, y) in [(0, 0), (127, 0), (0, 127), (127, 127), (12, 12), (115, 115)] {
            assert_eq!(pixel(&img, x, y)[3], 0, "corner {x},{y}");
        }
        let covered = opaque(&img).len() as f64 / (128.0 * 128.0);
        assert!((0.58..0.68).contains(&covered), "coverage {covered}");

        // Looking down 25 degrees, the top shows red and the bottom blue; each texel is pure.
        let top = pixel(&img, 64, 25);
        let bottom = pixel(&img, 64, 112);
        assert!(top[0] > 100 && top[1] == 0 && top[2] == 0, "top {top:?}");
        assert!(bottom[2] > 100 && bottom[0] == 0 && bottom[1] == 0, "bottom {bottom:?}");
        assert!(opaque(&img).iter().all(|p| p.2[1] == 0 && (p.2[0] == 0) != (p.2[2] == 0)));

        // The colour tints the texture; without a texture the sphere is the colour alone.
        let tinted = render_sphere(Some(&banded), [0.5, 1.0, 1.0], &view);
        assert!(pixel(&tinted, 64, 25)[0] < top[0]);
        assert_eq!(pixel(&tinted, 64, 112), bottom);
        let grey = render_sphere(None, [0.5; 3], &view);
        assert_eq!(opaque(&grey).len(), opaque(&img).len());
        assert!(opaque(&grey).iter().all(|p| p.2[0] == p.2[1] && p.2[1] == p.2[2]));

        // u runs around from +x, counter-clockwise: with red for u < 0.5 and green after, the
        // front (-y, u = 0.75) is green, the back (+y, u = 0.25) red, and seen from -x (u = 0.5 in
        // the middle) +y is on the left, so red is on the left and green on the right.
        let halves = image(2, 1, |x, _| if x == 0 { [255, 0, 0, 255] } else { [0, 255, 0, 255] });
        let from = |yaw: f64| render_sphere(Some(&halves), [1.0; 3], &IconView::new(128, yaw, 0.0, 1.0).unwrap());
        let (front, back, side) = (from(0.0), from(180.0), from(-90.0));
        let green = |p: [u8; 4]| p[0] == 0 && p[1] > 100 && p[3] == 255;
        let red = |p: [u8; 4]| p[1] == 0 && p[0] > 100 && p[3] == 255;
        assert!(green(pixel(&front, 64, 64)) && red(pixel(&back, 64, 64)), "{:?} {:?}", pixel(&front, 64, 64), pixel(&back, 64, 64));
        assert!(red(pixel(&side, 40, 64)) && green(pixel(&side, 88, 64)), "{:?} {:?}", pixel(&side, 40, 64), pixel(&side, 88, 64));
    }

    #[test]
    fn thumbnail_letterboxes_a_wide_image() {
        // 400 x 100 into 128: 128 x 32, rows 48..80, transparent above and below.
        let wide = image(400, 100, |_, _| [200, 40, 10, 255]);
        let t = thumbnail(&wide, 128);
        assert_eq!((t.width, t.height), (128, 128));
        for y in 0..128 {
            for x in [0, 1, 64, 127] {
                let expected = if (48..80).contains(&y) { [200, 40, 10, 255] } else { [0, 0, 0, 0] };
                assert_eq!(pixel(&t, x, y), expected, "{x},{y}");
            }
        }

        // Area averaging: alternate black and white columns average to grey (127.5 rounds up),
        // where a nearest-pixel scale would keep only one of them.
        let stripes = image(256, 128, |x, _| if x % 2 == 0 { [0, 0, 0, 255] } else { [255, 255, 255, 255] });
        let s = thumbnail(&stripes, 128);
        assert!((32..96).all(|y| (0..128).all(|x| pixel(&s, x, y) == [128, 128, 128, 255])));
        assert!((0..32).chain(96..128).all(|y| pixel(&s, 5, y) == [0, 0, 0, 0]));

        // Uneven ratios cover every source pixel by its share: a 3-pixel-wide ramp into 16.
        let ramp = image(48, 3, |x, _| [(x * 5) as u8, 0, 0, 255]);
        let r = thumbnail(&ramp, 16);
        let row: Vec<u8> = (0..16).map(|x| pixel(&r, x, 7)[0]).collect();
        let expected: Vec<u8> = (0..16).map(|x| (x * 3 * 5 + 5) as u8).collect();
        assert_eq!(row, expected);
        assert_eq!(pixel(&r, 3, 6), [0, 0, 0, 0]);
        assert_eq!(pixel(&r, 3, 8), [0, 0, 0, 0]);

        // Transparent source pixels do not darken their neighbours; alpha averages.
        let holes = image(256, 128, |x, _| if x % 2 == 0 { [250, 100, 50, 255] } else { [0, 0, 0, 0] });
        let h = thumbnail(&holes, 128);
        assert_eq!(pixel(&h, 10, 64), [250, 100, 50, 128]);

        // A tall image is pillarboxed; a small one is scaled up to fit.
        let tall = image(10, 40, |_, _| [9, 9, 9, 255]);
        let p = thumbnail(&tall, 64);
        assert_eq!((pixel(&p, 23, 30), pixel(&p, 24, 30), pixel(&p, 39, 30), pixel(&p, 40, 30)), ([0; 4], [9, 9, 9, 255], [9, 9, 9, 255], [0; 4]));
        let tiny = image(2, 1, |x, _| if x == 0 { [255, 0, 0, 255] } else { [0, 0, 255, 255] });
        let u = thumbnail(&tiny, 32);
        assert_eq!(
            (pixel(&u, 0, 16), pixel(&u, 31, 16), pixel(&u, 0, 7), pixel(&u, 0, 8)),
            ([255, 0, 0, 255], [0, 0, 255, 255], [0; 4], [255, 0, 0, 255])
        );
    }

    #[test]
    fn icon_view_refuses_bad_values() {
        assert_eq!(IconView::new(128, -30.0, 25.0, 1.0), Ok(IconView::default()));
        for (size, yaw, pitch, zoom) in [(16, 0.0, -90.0, 0.25), (512, 1e6, 90.0, 8.0), (100, -721.5, 0.0, 3.0)] {
            assert!(IconView::new(size, yaw, pitch, zoom).is_ok(), "{size} {yaw} {pitch} {zoom}");
        }
        let bad = [
            (0, 0.0, 0.0, 1.0, "size"),
            (15, 0.0, 0.0, 1.0, "size"),
            (513, 0.0, 0.0, 1.0, "size"),
            (u32::MAX, 0.0, 0.0, 1.0, "size"),
            (128, f64::NAN, 0.0, 1.0, "yaw"),
            (128, f64::INFINITY, 0.0, 1.0, "yaw"),
            (128, f64::NEG_INFINITY, 0.0, 1.0, "yaw"),
            (128, 0.0, 90.001, 1.0, "pitch"),
            (128, 0.0, -91.0, 1.0, "pitch"),
            (128, 0.0, f64::NAN, 1.0, "pitch"),
            (128, 0.0, f64::INFINITY, 1.0, "pitch"),
            (128, 0.0, 0.0, 0.0, "zoom"),
            (128, 0.0, 0.0, 0.2499, "zoom"),
            (128, 0.0, 0.0, 8.01, "zoom"),
            (128, 0.0, 0.0, -1.0, "zoom"),
            (128, 0.0, 0.0, f64::NAN, "zoom"),
            (128, 0.0, 0.0, f64::INFINITY, "zoom"),
        ];
        for (size, yaw, pitch, zoom, field) in bad {
            let err = IconView::new(size, yaw, pitch, zoom).unwrap_err();
            assert!(err.contains(field), "{size} {yaw} {pitch} {zoom}: {err}");
        }

        // A view built by hand around the constructor is not clamped: it draws an empty image,
        // and so does an icon size outside 16..=512.
        let tex = checker();
        for view in [
            IconView { size: 100_000, ..Default::default() },
            IconView { size: 0, ..Default::default() },
            IconView { yaw: f64::NAN, ..Default::default() },
            IconView { pitch: 135.0, ..Default::default() },
            IconView { zoom: 0.0, ..Default::default() },
            IconView { zoom: f64::INFINITY, ..Default::default() },
        ] {
            assert_eq!(render_meshes(&[cube()], &white, &|_| None, &view), ImageRgba::default());
            assert_eq!(render_symbol(&symbol_of(&[cube()], &[[1.0; 4]], vec![], &[None]), &view), ImageRgba::default());
            assert_eq!(render_sphere(Some(&tex), [1.0; 3], &view), ImageRgba::default());
        }
        for size in [0, 15, 513, u32::MAX] {
            assert_eq!(thumbnail(&tex, size), ImageRgba::default());
            assert_eq!(gradient([0; 3], [255; 3], size), ImageRgba::default());
            assert_eq!(solid([9; 3], size), ImageRgba::default());
        }
    }

    #[test]
    fn gradient_and_solid_fill_the_square() {
        let g = gradient([74, 128, 196], [214, 228, 240], 128);
        assert!((0..128).all(|x| pixel(&g, x, 0) == [74, 128, 196, 255] && pixel(&g, x, 127) == [214, 228, 240, 255]));
        // Row 63 of 127 steps: 74 + (214 - 74) * 63 / 127 = 143.4.
        assert_eq!(pixel(&g, 40, 63), [143, 178, 218, 255]);
        let rows: Vec<u8> = (0..128).map(|y| pixel(&g, 0, y)[0]).collect();
        assert!(rows.windows(2).all(|w| w[0] <= w[1]));
        let s = solid([128, 160, 200], 64);
        assert!(s.rgba.as_chunks::<4>().0.iter().all(|p| *p == [128, 160, 200, 255]));
    }

    #[test]
    fn the_srgb_curve_and_the_angles_match_their_formulas() {
        // The bisection-built curve against the formula through the platform's powf (fine in a
        // test: the comparison allows for its last bit).
        let srgb = Srgb::new();
        let formula = |c: f64| if c <= 0.003_130_8 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
        for k in 0..=200_000u32 {
            let c = f64::from(k) / 200_000.0;
            let exact = formula(c) * 255.0;
            if (exact - exact.floor() - 0.5).abs() < 1e-9 {
                continue;
            }
            assert_eq!(f64::from(srgb.encode(c)), (exact + 0.5).floor(), "{c}");
        }
        assert_eq!((srgb.encode(-1.0), srgb.encode(f64::NAN), srgb.encode(1.0), srgb.encode(7.0)), (0, 0, 255, 255));
        for b in 0..=255u8 {
            let s = f64::from(b) / 255.0;
            let reference = if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) };
            assert!((srgb.linear(b) - reference).abs() < 1e-14, "{b}");
            assert_eq!(srgb.encode(srgb.linear(b)), b);
        }
        for k in -1440..=1440 {
            let deg = f64::from(k) * 0.75;
            let (s, c) = sin_cos_deg(deg);
            assert!((s - deg.to_radians().sin()).abs() < 1e-14 && (c - deg.to_radians().cos()).abs() < 1e-14, "{deg}");
        }
        assert_eq!(sin_cos_deg(90.0).0, 1.0);
        assert_eq!(sin_cos_deg(0.0), (0.0, 1.0));
        assert_eq!(sin_cos_deg(-30.0), sin_cos_deg(330.0));
    }

    #[test]
    fn the_sphere_is_closed_and_textured_all_round() {
        let s = UvSphere::new();
        assert_eq!(s.positions.len(), 49 * 25);
        assert_eq!(s.indices.len(), 3 * 48 * 46);
        assert_eq!(s.positions[0], [0.0, 0.0, 1.0]);
        assert_eq!(s.positions[49 * 24], [0.0, 0.0, -1.0]);
        let m = Mesh { positions: s.positions.clone(), indices: s.indices.clone(), ..Mesh::new("sphere", ArchId(0)) };
        assert!(m.is_manifold());
        for p in &s.positions {
            let r = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
            assert!((r - 1.0).abs() < 1e-6);
        }
        // Outward: every triangle's normal points away from the centre.
        for t in s.indices.as_chunks::<3>().0 {
            let [a, b, c] = t.map(|i| widen(s.positions[i as usize]));
            let n = cross(sub(b, a), sub(c, a));
            assert!(dot(n, [a[0] + b[0] + c[0], a[1] + b[1] + c[1], a[2] + b[2] + c[2]]) > 0.0);
        }
        assert_eq!((s.uvs[0], s.uvs[48], s.uvs[49 * 25 - 1]), ([0.0, 0.0], [1.0, 0.0], [1.0, 1.0]));
    }
}
