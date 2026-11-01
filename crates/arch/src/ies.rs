//! IES LM-63 photometric files (phase 11): the text format lighting makers publish for a
//! fixture's measured light distribution. [`parse`] reads one into a
//! `cadcraft_doc::arch::IesProfile` (type C photometry), [`candela_at`] interpolates it, and
//! [`scaled`] fits it to a fixture's lumens. Stubs filled by `p11-arch-ies` (signatures final).
//!
//! The format (LM-63-2002, -1995 and -1991 headers alike): an `IESNA:LM-63-…` first line (absent
//! in 1986 files), keyword lines `[KEY] value`, then `TILT=NONE` (`TILT=INCLUDE` / a tilt file are
//! refused: `TILT=NONE only`), then whitespace-separated numbers: lamps, lumens per lamp,
//! candela multiplier, vertical angle count, horizontal angle count, photometric type (1 = C,
//! others refused), units type, width, length, height; ballast factor, future use (ballast-lamp
//! factor in 1991), input watts; the vertical angles; the horizontal angles; then for each
//! horizontal angle, one candela per vertical angle. Numbers may wrap across lines freely.

use cadcraft_doc::arch::IesProfile;

/// Why a file was refused: the 1-based line the problem was found on (0 = the file as a whole)
/// and what was wrong (`vertical angles must ascend, 45 after 60`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IesError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for IesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.line == 0 { write!(f, "IES: {}", self.message) } else { write!(f, "IES line {}: {}", self.line, self.message) }
    }
}

/// Read an IES file's text. `file_name` names the profile when the file has no `[LUMINAIRE]` or
/// `[TEST]` keyword. Refused (never a panic): more than `MAX_IES_BYTES`, no `TILT=` line, a tilt
/// other than `NONE`, a photometric type other than C (1), counts of 0 or above `MAX_IES_ANGLES`,
/// more than `MAX_IES_VALUES` candela values, missing or non-finite numbers, negative candela,
/// vertical angles outside 0..=180 or not ascending, horizontal angles outside 0..=360 or not
/// ascending, a first horizontal angle other than 0 (or 90 for a 90..270 file), trailing numbers.
pub fn parse(text: &str, file_name: &str) -> Result<IesProfile, IesError> {
    let _ = (text, file_name);
    Err(IesError { line: 0, message: "IES files are not read yet".into() })
}

/// Candela of `p` at `vertical_deg` (0 = along the aim) and `horizontal_deg` (0 = the fixture's
/// rotation, counter-clockwise), bilinear in the two angles. The horizontal angle is folded by the
/// profile's symmetry first: one angle = the same in every direction; last 90 = mirrored into
/// 0..90 in each quadrant; last 180 = mirrored across 0–180; last 360 = as given (wrapping). A
/// vertical angle beyond the profile's range is 0 cd (a profile ending at 90 emits nothing
/// upward). 0 for an empty profile or non-finite angles.
pub fn candela_at(p: &IesProfile, vertical_deg: f64, horizontal_deg: f64) -> f64 {
    let _ = (p, vertical_deg, horizontal_deg);
    0.0
}

/// `p` with every candela value multiplied by `lumens / p.lumens` (a fixture's own lumens), or
/// unchanged when `p.lumens <= 0` (absolute photometry) or the ratio is not finite.
pub fn scaled(p: &IesProfile, lumens: f64) -> IesProfile {
    let _ = lumens;
    p.clone()
}

/// The largest candela value of `p` (0 when empty).
pub fn peak_candela(p: &IesProfile) -> f64 {
    p.candela.iter().copied().filter(|c| c.is_finite()).fold(0.0, f64::max)
}
