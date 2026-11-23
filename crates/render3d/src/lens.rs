//! The camera lens a render applies (phase 22): the plain-number form of a look's lens.

use cadcraft_doc::arch::LensProjection;

/// What the lens does to a frame. The default is neutral: a rectilinear pinhole with no shift.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LensOptions {
    /// Panini projection instead of rectilinear.
    pub panini: bool,
    /// Panini compression, 0..=1.
    pub compression: f32,
    /// Vertical shift as a fraction of the frame height.
    pub shift: f32,
    /// F-number; `None` = pinhole (no depth of field).
    pub aperture: Option<f32>,
    /// Focus distance in metres; `None` = the camera's target distance.
    pub focus: Option<f32>,
}

impl Default for LensOptions {
    fn default() -> Self {
        LensOptions { panini: false, compression: 0.7, shift: 0.0, aperture: None, focus: None }
    }
}

impl LensOptions {
    /// Whether the lens changes nothing: no panini, no shift, no aperture.
    /// Compression and focus do not count.
    pub fn is_neutral(&self) -> bool {
        !self.panini && self.shift == 0.0 && self.aperture.is_none()
    }

    /// Whether the lens bends the image geometry (panini or a shift).
    pub fn warps(&self) -> bool {
        self.panini || self.shift != 0.0
    }

    /// The options of a look's lens.
    pub fn from_look(l: &cadcraft_doc::arch::Lens) -> Self {
        LensOptions {
            panini: l.projection == LensProjection::Panini,
            compression: l.compression as f32,
            shift: l.shift as f32,
            aperture: l.aperture.map(|a| a as f32),
            focus: l.focus.map(|f| f as f32),
        }
    }
}

/// Encode linear RGB as a little-endian PFM: header `PF\n<w> <h>\n-1.0\n`, then rows bottom to
/// top. `rgb` rows run top to bottom (row 0 is the top). Missing pixels are written as black.
pub fn encode_pfm(width: u32, height: u32, rgb: &[[f32; 3]]) -> Vec<u8> {
    let (w, h) = (width as usize, height as usize);
    let mut out = format!("PF\n{width} {height}\n-1.0\n").into_bytes();
    out.reserve(w.saturating_mul(h).saturating_mul(12).min(1 << 30));
    for y in (0..h).rev() {
        for x in 0..w {
            let px = y.checked_mul(w).and_then(|i| i.checked_add(x)).and_then(|i| rgb.get(i)).copied().unwrap_or([0.0; 3]);
            for c in px {
                out.extend_from_slice(&c.to_le_bytes());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::Lens;

    #[test]
    fn lens_options_from_look_and_neutral() {
        let n = LensOptions::default();
        assert!(n.is_neutral() && !n.warps());
        assert!(LensOptions::from_look(&Lens::default()).is_neutral());
        let l = Lens { projection: LensProjection::Panini, compression: 0.5, shift: 0.25, aperture: Some(2.8), focus: Some(3.0), ..Lens::default() };
        let o = LensOptions::from_look(&l);
        assert!(o.panini && o.warps() && !o.is_neutral());
        assert_eq!((o.compression, o.shift, o.aperture, o.focus), (0.5, 0.25, Some(2.8), Some(3.0)));
        let shifted = LensOptions { shift: 0.1, ..n };
        assert!(shifted.warps() && !shifted.is_neutral());
        let dof = LensOptions { aperture: Some(4.0), ..n };
        assert!(!dof.warps() && !dof.is_neutral());
    }

    #[test]
    fn encode_pfm_writes_rows_bottom_to_top() {
        let rgb = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]; // 1 wide, 2 tall: top then bottom
        let b = encode_pfm(1, 2, &rgb);
        let head = b"PF\n1 2\n-1.0\n";
        assert_eq!(&b[..head.len()], head);
        let body = &b[head.len()..];
        assert_eq!(body.len(), 24);
        let f = |i: usize| f32::from_le_bytes([body[i * 4], body[i * 4 + 1], body[i * 4 + 2], body[i * 4 + 3]]);
        assert_eq!([f(0), f(1), f(2)], [4.0, 5.0, 6.0]);
        assert_eq!([f(3), f(4), f(5)], [1.0, 2.0, 3.0]);
    }
}
