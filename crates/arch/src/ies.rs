//! IES LM-63 photometric files (phase 11): the text format lighting makers publish for a
//! fixture's measured light distribution. [`parse`] reads one into a
//! `cadcraft_doc::arch::IesProfile` (type C photometry), [`candela_at`] interpolates it, and
//! [`scaled`] fits it to a fixture's lumens.
//!
//! The format (LM-63-2002, -1995 and -1991 headers alike): an `IESNA:LM-63-…` first line (absent
//! in 1986 files), keyword lines `[KEY] value`, then `TILT=NONE` (`TILT=INCLUDE` / a tilt file are
//! refused: `TILT=NONE only`), then whitespace-separated numbers: lamps, lumens per lamp,
//! candela multiplier, vertical angle count, horizontal angle count, photometric type (1 = C,
//! others refused), units type, width, length, height; ballast factor, future use (ballast-lamp
//! factor in 1991), input watts; the vertical angles; the horizontal angles; then for each
//! horizontal angle, one candela per vertical angle. Numbers may wrap across lines freely.

use cadcraft_doc::arch::{IesProfile, MAX_IES_ANGLES, MAX_IES_BYTES, MAX_IES_VALUES};

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
///
/// Each refusal carries the 1-based line of the offending token; a file that ends early reports
/// the line of its last number, and a refusal of the file as a whole (too large, no `TILT=` line)
/// reports line 0.
pub fn parse(text: &str, file_name: &str) -> Result<IesProfile, IesError> {
    if text.len() > MAX_IES_BYTES {
        return Err(whole_file(format!("file over {MAX_IES_BYTES} bytes")));
    }
    let (keywords, tilt_line) = read_keywords(text)?;
    let mut tokens = tokenize(text, tilt_line)?;
    let header = read_header(&mut tokens)?;
    let vertical = read_angles(&mut tokens, header.vertical_count, Axis::Vertical)?;
    let (horizontal, first_line) = read_horizontal(&mut tokens, header.horizontal_count)?;
    check_first_horizontal(&horizontal, first_line)?;
    let candela = read_candela(&mut tokens, &horizontal, vertical.len(), header.scale)?;
    if let Some((_, line)) = tokens.peek() {
        return Err(at(line, "trailing numbers after the candela values"));
    }
    let name = keywords.name().unwrap_or_else(|| file_name.to_string());
    Ok(IesProfile { name, vertical, horizontal, candela, lumens: header.lumens, ..Default::default() })
}

/// Candela of `p` at `vertical_deg` (0 = along the aim) and `horizontal_deg` (0 = the fixture's
/// rotation, counter-clockwise), bilinear in the two angles. The horizontal angle is folded by the
/// profile's symmetry first: one angle = the same in every direction; last 90 = mirrored into
/// 0..90 in each quadrant; last 180 = mirrored across 0–180; last 360 = as given (wrapping). A
/// vertical angle beyond the profile's range is 0 cd (a profile ending at 90 emits nothing
/// upward). 0 for an empty profile or non-finite angles.
pub fn candela_at(p: &IesProfile, vertical_deg: f64, horizontal_deg: f64) -> f64 {
    let (nv, nh) = (p.vertical.len(), p.horizontal.len());
    if nv == 0 || nh == 0 || p.candela.len() != nv * nh || !vertical_deg.is_finite() || !horizontal_deg.is_finite() {
        return 0.0;
    }
    let Some(v) = bracket_within(&p.vertical, vertical_deg) else { return 0.0 };
    let h = horizontal_bracket(&p.horizontal, horizontal_deg);
    let cell = |hi: usize, vi: usize| p.candela.get(hi * nv + vi).copied().unwrap_or(0.0);
    let along_v = |hi: usize| lerp(cell(hi, v.lo), cell(hi, v.hi), v.t);
    lerp(along_v(h.lo), along_v(h.hi), h.t)
}

/// `p` with every candela value multiplied by `lumens / p.lumens` (a fixture's own lumens), or
/// unchanged when `p.lumens <= 0` (absolute photometry) or the ratio is not finite.
pub fn scaled(p: &IesProfile, lumens: f64) -> IesProfile {
    let ratio = lumens / p.lumens;
    if p.lumens.is_nan() || p.lumens <= 0.0 || !ratio.is_finite() {
        return p.clone();
    }
    IesProfile { candela: p.candela.iter().map(|c| c * lumens / p.lumens).collect(), lumens, ..p.clone() }
}

/// The largest candela value of `p` (0 when empty).
pub fn peak_candela(p: &IesProfile) -> f64 {
    p.candela.iter().copied().filter(|c| c.is_finite()).fold(0.0, f64::max)
}

// ---------------------------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------------------------

fn at(line: usize, message: impl Into<String>) -> IesError {
    IesError { line, message: message.into() }
}

fn whole_file(message: impl Into<String>) -> IesError {
    at(0, message)
}

/// The `[LUMINAIRE]` and `[TEST]` keyword values (trimmed) of a file's header.
#[derive(Default)]
struct Keywords {
    luminaire: Option<String>,
    test: Option<String>,
}

impl Keywords {
    fn name(self) -> Option<String> {
        self.luminaire.or(self.test)
    }
}

/// The keyword lines before the `TILT=` line, and the 1-based line of that `TILT=` line (which
/// must say `NONE`).
fn read_keywords(text: &str) -> Result<(Keywords, usize), IesError> {
    let mut keywords = Keywords::default();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim_start_matches('\u{feff}').trim();
        if line.len() >= 5 && line.is_char_boundary(5) && line[..5].eq_ignore_ascii_case("TILT=") {
            let tilt = line[5..].trim();
            if !tilt.eq_ignore_ascii_case("NONE") {
                return Err(at(index + 1, format!("TILT=NONE only, got {tilt}")));
            }
            return Ok((keywords, index + 1));
        }
        keyword(line, &mut keywords);
    }
    Err(whole_file("no TILT= line"))
}

/// Record `line` in `keywords` when it is a `[LUMINAIRE] …` or `[TEST] …` line with a value.
fn keyword(line: &str, keywords: &mut Keywords) {
    let Some(rest) = line.strip_prefix('[') else { return };
    let Some((key, value)) = rest.split_once(']') else { return };
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    let slot = match key.trim().to_ascii_uppercase().as_str() {
        "LUMINAIRE" => &mut keywords.luminaire,
        "TEST" => &mut keywords.test,
        _ => return,
    };
    if slot.is_none() {
        *slot = Some(value.to_string());
    }
}

/// The numbers after the `TILT=` line, each with the line it was on.
struct Tokens {
    items: Vec<(f64, usize)>,
    pos: usize,
    /// The line of the last number in the file (or of the `TILT=` line when there is none).
    last_line: usize,
}

impl Tokens {
    fn peek(&self) -> Option<(f64, usize)> {
        self.items.get(self.pos).copied()
    }

    /// The next number, or the refusal for a file that ended while reading `what`.
    fn next(&mut self, what: &str) -> Result<(f64, usize), IesError> {
        let item = self.peek().ok_or_else(|| at(self.last_line, format!("the file ended before the {what}")))?;
        self.pos += 1;
        Ok(item)
    }
}

/// Split everything after line `tilt_line` on whitespace and commas into finite numbers.
fn tokenize(text: &str, tilt_line: usize) -> Result<Tokens, IesError> {
    let mut items = Vec::new();
    for (index, raw) in text.lines().enumerate().skip(tilt_line) {
        for word in raw.split(|c: char| c.is_whitespace() || c == ',').filter(|w| !w.is_empty()) {
            match word.parse::<f64>() {
                Ok(n) if n.is_finite() => items.push((n, index + 1)),
                _ => return Err(at(index + 1, format!("not a number: `{word}`"))),
            }
        }
    }
    let last_line = items.last().map_or(tilt_line, |&(_, line)| line);
    Ok(Tokens { items, pos: 0, last_line })
}

/// What the numbers before the angles say.
struct Header {
    /// Rated lumens (lamps x lumens per lamp), or -1 for absolute photometry.
    lumens: f64,
    /// Candela multiplier x ballast factor, applied to every candela value.
    scale: f64,
    vertical_count: usize,
    horizontal_count: usize,
}

/// A number that must be above 0.
fn positive(tokens: &mut Tokens, what: &str) -> Result<f64, IesError> {
    let (n, line) = tokens.next(what)?;
    if n > 0.0 { Ok(n) } else { Err(at(line, format!("{what} must be > 0, got {n}"))) }
}

/// An angle count: a whole number within 1..=`MAX_IES_ANGLES`.
fn angle_count(tokens: &mut Tokens, axis: Axis) -> Result<(usize, usize), IesError> {
    let label = axis.label();
    let (n, line) = tokens.next(&format!("{label} angle count"))?;
    if n.fract() != 0.0 {
        return Err(at(line, format!("{label} angles: count {n} is not a whole number")));
    }
    if n < 1.0 {
        return Err(at(line, format!("{label} angles: count {n} is under 1")));
    }
    if n > MAX_IES_ANGLES as f64 {
        return Err(at(line, format!("{label} angles: count {n} is over {MAX_IES_ANGLES}")));
    }
    Ok((n as usize, line))
}

fn read_header(tokens: &mut Tokens) -> Result<Header, IesError> {
    let lamps = positive(tokens, "number of lamps")?;
    let (per_lamp, line) = tokens.next("lumens per lamp")?;
    let lumens = if per_lamp == -1.0 {
        -1.0
    } else if per_lamp > 0.0 {
        lamps * per_lamp
    } else {
        return Err(at(line, format!("lumens per lamp must be > 0 or -1 (absolute), got {per_lamp}")));
    };
    let multiplier = positive(tokens, "candela multiplier")?;
    let (vertical_count, _) = angle_count(tokens, Axis::Vertical)?;
    let (horizontal_count, count_line) = angle_count(tokens, Axis::Horizontal)?;
    let values = vertical_count * horizontal_count;
    if values > MAX_IES_VALUES {
        return Err(at(count_line, format!("candela values: {values} is over {MAX_IES_VALUES}")));
    }
    let (photometric, line) = tokens.next("photometric type")?;
    if photometric != 1.0 {
        return Err(at(line, format!("photometric type {photometric} is not supported: type C (1) only")));
    }
    // Units type, then the luminous opening's width, length and height: read, not used.
    for what in ["units type", "luminous width", "luminous length", "luminous height"] {
        tokens.next(what)?;
    }
    let ballast = positive(tokens, "ballast factor")?;
    // The future-use (1991: ballast-lamp photometric) factor and the input watts: read, not used.
    for what in ["future-use factor", "input watts"] {
        tokens.next(what)?;
    }
    Ok(Header { lumens, scale: multiplier * ballast, vertical_count, horizontal_count })
}

#[derive(Clone, Copy)]
enum Axis {
    Vertical,
    Horizontal,
}

impl Axis {
    fn label(self) -> &'static str {
        match self {
            Axis::Vertical => "vertical",
            Axis::Horizontal => "horizontal",
        }
    }

    /// The largest angle of the axis (degrees).
    fn limit(self) -> f64 {
        match self {
            Axis::Vertical => 180.0,
            Axis::Horizontal => 360.0,
        }
    }
}

/// `count` angles of `axis`: within 0..=limit, strictly ascending. Returns them and the line of
/// the first.
fn read_axis(tokens: &mut Tokens, count: usize, axis: Axis) -> Result<(Vec<f64>, usize), IesError> {
    let label = axis.label();
    let mut angles: Vec<f64> = Vec::with_capacity(count);
    let mut first_line = tokens.last_line;
    for i in 0..count {
        let (angle, line) = tokens.next(&format!("{label} angles"))?;
        if i == 0 {
            first_line = line;
        }
        if !(0.0..=axis.limit()).contains(&angle) {
            return Err(at(line, format!("{label} angles must be within 0..={}, got {angle}", axis.limit())));
        }
        if let Some(&before) = angles.last() {
            if angle <= before {
                return Err(at(line, format!("{label} angles must ascend: {angle} after {before}")));
            }
        }
        angles.push(angle);
    }
    Ok((angles, first_line))
}

fn read_angles(tokens: &mut Tokens, count: usize, axis: Axis) -> Result<Vec<f64>, IesError> {
    read_axis(tokens, count, axis).map(|(angles, _)| angles)
}

fn read_horizontal(tokens: &mut Tokens, count: usize) -> Result<(Vec<f64>, usize), IesError> {
    read_axis(tokens, count, Axis::Horizontal)
}

/// The first horizontal angle is 0, or 90 in a file that runs 90..=270.
fn check_first_horizontal(horizontal: &[f64], first_line: usize) -> Result<(), IesError> {
    let (Some(&first), Some(&last)) = (horizontal.first(), horizontal.last()) else { return Ok(()) };
    if first == 0.0 || (first == 90.0 && last == 270.0) {
        return Ok(());
    }
    Err(at(first_line, format!("horizontal angles must start at 0 (or 90 in a file ending at 270), got {first}")))
}

/// One candela value per vertical angle for each horizontal angle, times `scale`.
fn read_candela(tokens: &mut Tokens, horizontal: &[f64], vertical_count: usize, scale: f64) -> Result<Vec<f64>, IesError> {
    let mut candela = Vec::with_capacity(horizontal.len() * vertical_count);
    for angle in horizontal {
        for _ in 0..vertical_count {
            let Some((value, line)) = tokens.peek() else {
                let message = format!("expected {vertical_count} candela values for horizontal angle {angle}, the file ended");
                return Err(at(tokens.last_line, message));
            };
            tokens.pos += 1;
            if value < 0.0 {
                return Err(at(line, format!("candela must be >= 0, got {value}")));
            }
            candela.push(value * scale);
        }
    }
    Ok(candela)
}

// ---------------------------------------------------------------------------------------------
// Interpolating
// ---------------------------------------------------------------------------------------------

/// First `n` characters of `s` as a well-formed slice (0 when `n == 0`). Used only by the
/// char-aware mutation fuzz test so a prefix is always valid UTF-8.
fn prefix(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

/// Two neighbouring entries of an angle list and how far between them (0 = `lo`, 1 = `hi`).
struct Bracket {
    lo: usize,
    hi: usize,
    t: f64,
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// The fraction of `x` between `a` and `b` (0 when they coincide).
fn fraction(a: f64, b: f64, x: f64) -> f64 {
    let span = b - a;
    if span > 0.0 { ((x - a) / span).clamp(0.0, 1.0) } else { 0.0 }
}

/// The neighbours of `x` in the ascending `angles`, or `None` when `x` lies outside them.
fn bracket_within(angles: &[f64], x: f64) -> Option<Bracket> {
    let (&first, &last) = (angles.first()?, angles.last()?);
    if x < first || x > last {
        return None;
    }
    Some(bracket_clamped(angles, x))
}

/// The neighbours of `x` in the ascending `angles`, an `x` outside them taking the nearest end.
fn bracket_clamped(angles: &[f64], x: f64) -> Bracket {
    let upper = angles.partition_point(|&a| a <= x);
    if upper == 0 {
        return Bracket { lo: 0, hi: 0, t: 0.0 };
    }
    if upper >= angles.len() {
        let end = angles.len() - 1;
        return Bracket { lo: end, hi: end, t: 0.0 };
    }
    let (lo, hi) = (upper - 1, upper);
    let t = fraction(angles.get(lo).copied().unwrap_or(0.0), angles.get(hi).copied().unwrap_or(0.0), x);
    Bracket { lo, hi, t }
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

/// The columns of `horizontal` that `horizontal_deg` falls between once folded by the profile's
/// symmetry (see [`candela_at`]).
fn horizontal_bracket(horizontal: &[f64], horizontal_deg: f64) -> Bracket {
    let (Some(&first), Some(&last)) = (horizontal.first(), horizontal.last()) else {
        return Bracket { lo: 0, hi: 0, t: 0.0 };
    };
    let mut h = horizontal_deg.rem_euclid(360.0);
    if horizontal.len() == 1 {
        return Bracket { lo: 0, hi: 0, t: 0.0 };
    }
    if near(last, 90.0) {
        if h > 180.0 {
            h = 360.0 - h;
        }
        if h > 90.0 {
            h = 180.0 - h;
        }
    } else if near(last, 180.0) {
        if h > 180.0 {
            h = 360.0 - h;
        }
    } else if near(first, 90.0) && near(last, 270.0) {
        if h < 90.0 {
            h = 180.0 - h;
        } else if h > 270.0 {
            h = 540.0 - h;
        }
    } else {
        return full_circle_bracket(horizontal, first, last, h);
    }
    bracket_clamped(horizontal, h)
}

/// A profile that covers the whole circle: past the last angle the light comes back round to the
/// first (`first` + 360).
fn full_circle_bracket(horizontal: &[f64], first: f64, last: f64, h: f64) -> Bracket {
    let h = if h < first { h + 360.0 } else { h };
    if h <= last {
        return bracket_clamped(horizontal, h);
    }
    Bracket { lo: horizontal.len() - 1, hi: 0, t: fraction(last, first + 360.0, h) }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    /// The file the phase-11 acceptance check writes.
    const DOWNLIGHT: &str = "IESNA:LM-63-2002\n[TEST] cadcraft phase-11 test downlight\n[LUMINAIRE] Test downlight 1000 lm\nTILT=NONE\n1 1000 1 5 1 1 2 0.15 0.15 0.1\n1.0 1.0 12\n0 22.5 45 67.5 90\n0\n1000 900 600 200 0\n";

    /// A type C file with the given horizontal angles and one candela row per horizontal angle
    /// (vertical angles 0, 45, 90).
    fn file(horizontal: &[f64], rows: &[[f64; 3]]) -> String {
        let angles = horizontal.iter().map(|a| a.to_string()).collect::<Vec<_>>().join(" ");
        let body = rows.iter().map(|r| format!("{} {} {}", r[0], r[1], r[2])).collect::<Vec<_>>().join("\n");
        format!("TILT=NONE\n1 1000 1 3 {} 1 1 0 0 0\n1 1 10\n0 45 90\n{angles}\n{body}\n", horizontal.len())
    }

    fn profile(horizontal: &[f64], rows: &[[f64; 3]]) -> IesProfile {
        parse(&file(horizontal, rows), "t.ies").unwrap()
    }

    #[test]
    fn ies_reads_a_type_c_file_and_names_it() {
        let p = parse(DOWNLIGHT, "downlight.ies").unwrap();
        assert_eq!(p.name, "Test downlight 1000 lm");
        assert_eq!(p.vertical, vec![0.0, 22.5, 45.0, 67.5, 90.0]);
        assert_eq!(p.horizontal, vec![0.0]);
        assert_eq!(p.candela, vec![1000.0, 900.0, 600.0, 200.0, 0.0]);
        assert_eq!(p.lumens, 1000.0);

        // [TEST] names a file without [LUMINAIRE]; the file name names one with neither; the
        // keywords are trimmed.
        let test_only = DOWNLIGHT.replace("[LUMINAIRE] Test downlight 1000 lm\n", "");
        assert_eq!(parse(&test_only, "x.ies").unwrap().name, "cadcraft phase-11 test downlight");
        let neither = test_only.replace("[TEST] cadcraft phase-11 test downlight\n", "");
        assert_eq!(parse(&neither, "x.ies").unwrap().name, "x.ies");
        let padded = DOWNLIGHT.replace("[LUMINAIRE] Test downlight 1000 lm", "[LUMINAIRE]   Spaced out \t ");
        assert_eq!(parse(&padded, "x.ies").unwrap().name, "Spaced out");

        // 1986 files have no IESNA line; keys and TILT are case-insensitive; CRLF is fine.
        let old = DOWNLIGHT.replace("IESNA:LM-63-2002\n", "").replace("TILT=NONE", "tilt=none").replace('\n', "\r\n");
        assert_eq!(parse(&old, "x.ies").unwrap(), p);

        // Numbers wrap across lines and may be comma separated; lamps multiply the lumens; the
        // candela multiplier and the ballast factor scale the candela values; -1 = absolute.
        let wrapped = "TILT=NONE\n2,500,0.5,3,1,1,1\n0,0,0 0.8\n0.5 9\n0,45,90 0\n100,\n80\n20\n";
        let w = parse(wrapped, "w.ies").unwrap();
        assert_eq!(w.lumens, 1000.0);
        assert_eq!(w.candela, vec![100.0 * 0.5 * 0.8, 80.0 * 0.5 * 0.8, 20.0 * 0.5 * 0.8]);
        let absolute = parse(&DOWNLIGHT.replace("1 1000 1 5", "1 -1 1 5"), "a.ies").unwrap();
        assert_eq!(absolute.lumens, -1.0);

        // A 90..270 file starts at 90; every shape of the horizontal list reads.
        let p = profile(&[90.0, 180.0, 270.0], &[[3.0, 2.0, 1.0], [6.0, 5.0, 4.0], [9.0, 8.0, 7.0]]);
        assert_eq!(p.horizontal, vec![90.0, 180.0, 270.0]);
        assert_eq!(p.candela.len(), 9);
        assert_eq!(peak_candela(&p), 9.0);
        assert_eq!(peak_candela(&IesProfile::default()), 0.0);
    }

    fn refusal(text: &str) -> IesError {
        parse(text, "t.ies").expect_err("must be refused")
    }

    #[test]
    fn ies_refusals_name_the_line_and_reason() {
        let cases: Vec<(String, usize, &str)> = vec![
            ("x".repeat(MAX_IES_BYTES + 1), 0, "file over 1000000 bytes"),
            ("IESNA:LM-63-2002\n[TEST] a\n1 1 1\n".into(), 0, "no TILT= line"),
            (DOWNLIGHT.replace("TILT=NONE", "TILT=INCLUDE"), 4, "TILT=NONE only, got INCLUDE"),
            (DOWNLIGHT.replace("0 22.5 45 67.5 90", "0 22.5 nine 67.5 90"), 7, "not a number: `nine`"),
            (DOWNLIGHT.replace("1000 900 600 200 0", "1000 900 600 inf 0"), 9, "not a number: `inf`"),
            (DOWNLIGHT.replace("1 1000 1 5", "0 1000 1 5"), 5, "number of lamps must be > 0"),
            (DOWNLIGHT.replace("1 1000 1 5", "1 0 1 5"), 5, "lumens per lamp must be > 0 or -1"),
            (DOWNLIGHT.replace("1 1000 1 5", "1 1000 0 5"), 5, "candela multiplier must be > 0"),
            (DOWNLIGHT.replace("1 1000 1 5", "1 1000 1 5000"), 5, "vertical angles: count 5000 is over 1024"),
            (DOWNLIGHT.replace("1 1000 1 5 1 1", "1 1000 1 5 5000 1"), 5, "horizontal angles: count 5000 is over 1024"),
            (DOWNLIGHT.replace("1 1000 1 5", "1 1000 1 0"), 5, "vertical angles: count 0 is under 1"),
            (DOWNLIGHT.replace("1 1000 1 5", "1 1000 1 2.5"), 5, "vertical angles: count 2.5 is not a whole number"),
            (DOWNLIGHT.replace("1 1000 1 5 1 1", "1 1000 1 1000 1000 1"), 5, "candela values: 1000000 is over 65536"),
            (DOWNLIGHT.replace("5 1 1 2", "5 1 2 2"), 5, "photometric type 2 is not supported: type C (1) only"),
            (DOWNLIGHT.replace("1.0 1.0 12", "0 1.0 12"), 6, "ballast factor must be > 0"),
            (DOWNLIGHT.replace("0 22.5 45 67.5 90", "0 45 22.5 67.5 90"), 7, "vertical angles must ascend: 22.5 after 45"),
            (DOWNLIGHT.replace("0 22.5 45 67.5 90", "0 22.5 45 67.5 190"), 7, "vertical angles must be within 0..=180, got 190"),
            (DOWNLIGHT.replace("\n0\n", "\n10\n"), 8, "horizontal angles must start at 0"),
            (DOWNLIGHT.replace("1000 900 600 200 0", "1000 900 -600 200 0"), 9, "candela must be >= 0, got -600"),
            (DOWNLIGHT.replace("1000 900 600 200 0", "1000 900 600 200"), 9, "expected 5 candela values for horizontal angle 0, the file ended"),
            (format!("{DOWNLIGHT}7 8\n"), 10, "trailing numbers after the candela values"),
            ("TILT=NONE\n1 1000 1 3 3 1 1 0 0 0\n1 1 10\n0 45 90\n0 90 45\n1 2 3\n".into(), 5, "horizontal angles must ascend: 45 after 90"),
            ("TILT=NONE\n1 1000 1 3 2 1 1 0 0 0\n1 1 10\n0 45 90\n0 400\n".into(), 5, "horizontal angles must be within 0..=360, got 400"),
            ("TILT=NONE\n1 1000 1 3 2 1 1 0 0 0\n1 1 10\n0 45 90\n90 180\n".into(), 5, "horizontal angles must start at 0"),
        ];
        for (text, line, message) in cases {
            let err = refusal(&text);
            assert_eq!(err.line, line, "{message}: {err}");
            assert!(err.message.contains(message), "wanted `{message}`, got `{}`", err.message);
        }
        // The Display wording: a whole-file refusal has no line.
        assert_eq!(refusal(&"x".repeat(MAX_IES_BYTES + 1)).to_string(), "IES: file over 1000000 bytes");
        assert_eq!(refusal(&DOWNLIGHT.replace("TILT=NONE", "TILT=INCLUDE")).to_string(), "IES line 4: TILT=NONE only, got INCLUDE");
        // A file that ends inside the header names the line of its last number.
        let short = refusal("TILT=NONE\n1 1000 1 5\n");
        assert_eq!(short.line, 2);
        assert!(short.message.contains("the file ended"), "{short}");
    }

    #[test]
    fn ies_never_panics_on_garbage() {
        let mut state = 0x2545_f491_4f6c_dd1d_u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let pieces = ["TILT=NONE", "\n", " ", ",", "1", "0", "-1", "90", "180", "360", "1e999", "nan", "[TEST]", "é", "5000", "22.5", "\r", "IESNA"];
        for _ in 0..400 {
            let text: String = (0..(next() % 60)).map(|_| pieces[(next() % pieces.len() as u64) as usize]).collect();
            drop(parse(&text, "fuzz.ies"));
        }
        // Mutations of a good file, slicing over complete chars only (DOWNLIGHT holds the
        // multi-byte `é` in [TEST] cadcraft phase-11 test downlight; indexing by character
        // keeps every prefix a valid slice instead of a byte offset).
        for (cut, _) in DOWNLIGHT.chars().enumerate() {
            drop(parse(prefix(DOWNLIGHT, cut + 1), "cut.ies"));
        }
        let wild = IesProfile { vertical: vec![0.0, 90.0], horizontal: vec![0.0, 180.0], candela: vec![1.0], lumens: 1.0, ..Default::default() };
        assert_eq!(candela_at(&wild, 10.0, 10.0), 0.0);
    }

    #[test]
    fn candela_at_interpolates_and_folds_symmetry() {
        let p = parse(DOWNLIGHT, "d.ies").unwrap();
        assert_eq!(candela_at(&p, 0.0, 37.0), 1000.0);
        assert!((candela_at(&p, 22.62, 0.0) - 898.4).abs() < 0.1, "{}", candela_at(&p, 22.62, 0.0));
        assert_eq!(candela_at(&p, 95.0, 0.0), 0.0);
        assert_eq!(candela_at(&p, -1.0, 0.0), 0.0);
        assert_eq!(candela_at(&p, 90.0, 123.0), 0.0);
        assert_eq!(candela_at(&p, 45.0, -200.0), 600.0);
        assert_eq!(candela_at(&p, f64::NAN, 0.0), 0.0);
        assert_eq!(candela_at(&p, 0.0, f64::INFINITY), 0.0);
        assert_eq!(candela_at(&IesProfile::default(), 0.0, 0.0), 0.0);

        // A single vertical angle answers only at that angle.
        let one = IesProfile { vertical: vec![30.0], horizontal: vec![0.0], candela: vec![50.0], lumens: 1.0, ..Default::default() };
        assert_eq!(candela_at(&one, 30.0, 0.0), 50.0);
        assert_eq!(candela_at(&one, 31.0, 0.0), 0.0);

        // Quadrant symmetry (last angle 90): the two-column file [0, 90] reads column 0 at 180
        // and column 90 at 270, and mirrors into each quadrant.
        let quadrant = profile(&[0.0, 90.0], &[[100.0, 100.0, 100.0], [300.0, 300.0, 300.0]]);
        assert_eq!(candela_at(&quadrant, 0.0, 0.0), 100.0);
        assert_eq!(candela_at(&quadrant, 0.0, 180.0), 100.0);
        assert_eq!(candela_at(&quadrant, 0.0, 90.0), 300.0);
        assert_eq!(candela_at(&quadrant, 0.0, 270.0), 300.0);
        assert_eq!(candela_at(&quadrant, 0.0, 450.0), 300.0);
        assert_eq!(candela_at(&quadrant, 0.0, 45.0), 200.0);
        assert_eq!(candela_at(&quadrant, 0.0, 135.0), 200.0);
        assert_eq!(candela_at(&quadrant, 0.0, 225.0), 200.0);
        assert_eq!(candela_at(&quadrant, 0.0, 315.0), 200.0);

        // Bilateral (last 180): mirrored across 0-180.
        let bilateral = profile(&[0.0, 90.0, 180.0], &[[0.0, 0.0, 0.0], [90.0, 90.0, 90.0], [180.0, 180.0, 180.0]]);
        assert_eq!(candela_at(&bilateral, 0.0, 45.0), 45.0);
        assert_eq!(candela_at(&bilateral, 0.0, 315.0), 45.0);
        assert_eq!(candela_at(&bilateral, 0.0, 270.0), 90.0);
        assert_eq!(candela_at(&bilateral, 0.0, 180.0), 180.0);

        // Bilateral about the 90-270 plane (a file from 90 to 270).
        let plane = profile(&[90.0, 180.0, 270.0], &[[10.0; 3], [20.0; 3], [30.0; 3]]);
        assert_eq!(candela_at(&plane, 0.0, 90.0), 10.0);
        assert_eq!(candela_at(&plane, 0.0, 270.0), 30.0);
        assert_eq!(candela_at(&plane, 0.0, 0.0), 20.0); // mirror of 180
        assert_eq!(candela_at(&plane, 0.0, 45.0), 15.0); // mirror of 135
        assert_eq!(candela_at(&plane, 0.0, 315.0), 25.0); // mirror of 225

        // Full circle: the last angle interpolates round to the first at 360.
        let full = profile(&[0.0, 120.0, 240.0], &[[0.0; 3], [120.0; 3], [240.0; 3]]);
        assert_eq!(candela_at(&full, 0.0, 60.0), 60.0);
        assert_eq!(candela_at(&full, 0.0, 240.0), 240.0);
        assert_eq!(candela_at(&full, 0.0, 300.0), 120.0); // halfway from 240 to 360 (= 0)
        assert_eq!(candela_at(&full, 0.0, 360.0), 0.0);
        assert_eq!(candela_at(&full, 0.0, -60.0), 120.0);

        // Bilinear: halfway in both angles of a 2 x 2 corner.
        let corners = IesProfile {
            vertical: vec![0.0, 90.0],
            horizontal: vec![0.0, 90.0],
            candela: vec![0.0, 10.0, 20.0, 30.0],
            lumens: 1.0,
            ..Default::default()
        };
        assert_eq!(candela_at(&corners, 45.0, 45.0), 15.0);
        assert_eq!(candela_at(&corners, 90.0, 90.0), 30.0);

        // A hand-built profile whose candela do not fit its angles reads as dark, not a panic.
        let torn = IesProfile { vertical: vec![0.0, 90.0], horizontal: vec![0.0], candela: vec![1.0], lumens: 1.0, ..Default::default() };
        assert_eq!(candela_at(&torn, 10.0, 0.0), 0.0);
    }

    #[test]
    fn scaled_fits_the_fixture_lumens() {
        let p = parse(DOWNLIGHT, "d.ies").unwrap();
        let s = scaled(&p, 2000.0);
        assert_eq!(s.lumens, 2000.0);
        assert_eq!(s.candela, vec![2000.0, 1800.0, 1200.0, 400.0, 0.0]);
        assert_eq!(s.vertical, p.vertical);
        assert_eq!(s.horizontal, p.horizontal);
        assert_eq!(s.name, p.name);
        assert_eq!(peak_candela(&s), 2000.0);
        assert_eq!(scaled(&p, 1000.0), p);
        // The original is untouched.
        assert_eq!(p.candela[0], 1000.0);

        // Absolute photometry, an empty rating, and a ratio that is not finite stay as they are.
        let absolute = IesProfile { lumens: -1.0, ..p.clone() };
        assert_eq!(scaled(&absolute, 2000.0), absolute);
        let unrated = IesProfile { lumens: 0.0, ..p.clone() };
        assert_eq!(scaled(&unrated, 2000.0), unrated);
        assert_eq!(scaled(&p, f64::INFINITY), p);
        assert_eq!(scaled(&p, f64::NAN), p);
    }

    #[test]
    fn candela_at_exact_angles_returns_stored_values() {
        let p = parse(DOWNLIGHT, "d.ies").unwrap();
        assert_eq!(candela_at(&p, 0.0, 0.0), 1000.0);
        assert_eq!(candela_at(&p, 22.5, 0.0), 900.0);
        assert_eq!(candela_at(&p, 45.0, 0.0), 600.0);
        assert_eq!(candela_at(&p, 67.5, 0.0), 200.0);
        assert_eq!(candela_at(&p, 90.0, 0.0), 0.0);

        let multi = profile(&[0.0, 90.0, 180.0], &[[10.0, 20.0, 30.0], [40.0, 50.0, 60.0], [70.0, 80.0, 90.0]]);
        assert_eq!(candela_at(&multi, 0.0, 0.0), 10.0);
        assert_eq!(candela_at(&multi, 0.0, 90.0), 40.0);
        assert_eq!(candela_at(&multi, 0.0, 180.0), 70.0);
        assert_eq!(candela_at(&multi, 45.0, 0.0), 20.0);
        assert_eq!(candela_at(&multi, 45.0, 90.0), 50.0);
        assert_eq!(candela_at(&multi, 45.0, 180.0), 80.0);
        assert_eq!(candela_at(&multi, 90.0, 0.0), 30.0);
        assert_eq!(candela_at(&multi, 90.0, 90.0), 60.0);
        assert_eq!(candela_at(&multi, 90.0, 180.0), 90.0);
    }

    #[test]
    fn full_circle_wraparound_at_boundaries() {
        let full = profile(&[0.0, 120.0, 240.0], &[[0.0, 0.0, 0.0], [120.0, 120.0, 120.0], [240.0, 240.0, 240.0]]);
        assert_eq!(candela_at(&full, 0.0, 0.0), 0.0);
        assert_eq!(candela_at(&full, 0.0, 120.0), 120.0);
        assert_eq!(candela_at(&full, 0.0, 240.0), 240.0);
        assert_eq!(candela_at(&full, 0.0, 360.0), 0.0);
        assert_eq!(candela_at(&full, 0.0, 300.0), 120.0);
        assert_eq!(candela_at(&full, 0.0, 60.0), 60.0);
        assert_eq!(candela_at(&full, 0.0, -120.0), 240.0);
        assert_eq!(candela_at(&full, 0.0, -240.0), 120.0);
    }

    #[test]
    fn horizontal_symmetry_at_exact_boundaries() {
        let quadrant = profile(&[0.0, 90.0], &[[100.0, 100.0, 100.0], [300.0, 300.0, 300.0]]);
        assert_eq!(candela_at(&quadrant, 0.0, 0.0), 100.0);
        assert_eq!(candela_at(&quadrant, 0.0, 90.0), 300.0);
        assert_eq!(candela_at(&quadrant, 0.0, 180.0), 100.0);
        assert_eq!(candela_at(&quadrant, 0.0, 270.0), 300.0);
        assert_eq!(candela_at(&quadrant, 0.0, 360.0), 100.0);

        let bilateral = profile(&[0.0, 180.0], &[[100.0, 100.0, 100.0], [200.0, 200.0, 200.0]]);
        assert_eq!(candela_at(&bilateral, 0.0, 0.0), 100.0);
        assert_eq!(candela_at(&bilateral, 0.0, 180.0), 200.0);
        assert_eq!(candela_at(&bilateral, 0.0, 360.0), 100.0);

        let plane = profile(&[90.0, 180.0, 270.0], &[[10.0, 10.0, 10.0], [20.0, 20.0, 20.0], [30.0, 30.0, 30.0]]);
        assert_eq!(candela_at(&plane, 0.0, 90.0), 10.0);
        assert_eq!(candela_at(&plane, 0.0, 180.0), 20.0);
        assert_eq!(candela_at(&plane, 0.0, 270.0), 30.0);
        assert_eq!(candela_at(&plane, 0.0, 0.0), 20.0);
        assert_eq!(candela_at(&plane, 0.0, 360.0), 20.0);
    }

    #[test]
    fn peak_candela_ignores_non_finite() {
        let p = IesProfile {
            name: "test".into(),
            vertical: vec![0.0, 90.0],
            horizontal: vec![0.0],
            candela: vec![100.0, f64::NAN, 50.0, f64::INFINITY, 75.0],
            lumens: 1000.0,
            ..Default::default()
        };
        assert_eq!(peak_candela(&p), 100.0);

        let all_nan = IesProfile {
            name: "test".into(),
            vertical: vec![0.0],
            horizontal: vec![0.0],
            candela: vec![f64::NAN],
            lumens: 1000.0,
            ..Default::default()
        };
        assert_eq!(peak_candela(&all_nan), 0.0);
    }

    #[test]
    fn parse_rejects_non_finite_angles_and_candela() {
        let nan_v = DOWNLIGHT.replace("0 22.5 45 67.5 90", "0 nan 45 67.5 90");
        let err = refusal(&nan_v);
        assert_eq!(err.line, 7);
        assert!(err.message.contains("not a number"), "{}", err.message);

        let inf_h = DOWNLIGHT.replace("\n0\n", "\ninf\n");
        let err = refusal(&inf_h);
        assert_eq!(err.line, 8);
        assert!(err.message.contains("not a number"), "{}", err.message);

        let nan_c = DOWNLIGHT.replace("1000 900 600 200 0", "1000 nan 600 200 0");
        let err = refusal(&nan_c);
        assert_eq!(err.line, 9);
        assert!(err.message.contains("not a number"), "{}", err.message);
    }

    #[test]
    fn scaled_edge_cases() {
        let p = parse(DOWNLIGHT, "d.ies").unwrap();

        assert_eq!(scaled(&p, 1000.0), p);

        let zero = scaled(&p, 0.0);
        assert_eq!(zero.lumens, 0.0);
        assert_eq!(zero.candela, vec![0.0; 5]);

        let big = scaled(&p, 1e12);
        assert_eq!(big.lumens, 1e12);
        assert_eq!(big.candela[0], 1000.0 * 1e12 / 1000.0);

        let small = scaled(&p, 1e-12);
        assert_eq!(small.lumens, 1e-12);
        assert_eq!(small.candela[0], 1000.0 * 1e-12 / 1000.0);
    }

    #[test]
    fn candela_at_single_angle_profile() {
        let single = IesProfile {
            name: "test".into(),
            vertical: vec![45.0],
            horizontal: vec![0.0],
            candela: vec![100.0],
            lumens: 1000.0,
            ..Default::default()
        };
        assert_eq!(candela_at(&single, 45.0, 0.0), 100.0);
        assert_eq!(candela_at(&single, 46.0, 0.0), 0.0);
        assert_eq!(candela_at(&single, 44.0, 0.0), 0.0);
        assert_eq!(candela_at(&single, 45.0, 90.0), 100.0);
        assert_eq!(candela_at(&single, 45.0, 180.0), 100.0);
        assert_eq!(candela_at(&single, 45.0, 359.0), 100.0);
    }
}
