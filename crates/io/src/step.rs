//! ISO 10303-21 (STEP physical file) text for the IFC export (phase 12): entity instances
//! `#n=IFCNAME(params);` numbered from 1 in the order they are added, the HEADER section, and the
//! standard's encodings of strings and reals. Pure text: no schema knowledge lives here.

use std::fmt::Write as _;

/// A reference to an entity instance a [`StepWriter`] wrote (`#n`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ref(pub u32);

/// One parameter of an entity instance.
#[derive(Clone, Debug, PartialEq)]
pub enum P {
    /// `#n`.
    Ref(Ref),
    /// A STRING, quoted and encoded by [`encode_string`].
    Str(String),
    /// An ENUMERATION value, written `.NAME.`.
    Enum(&'static str),
    /// A REAL, written by [`real`].
    Real(f64),
    /// An INTEGER.
    Int(i64),
    /// A BOOLEAN, `.T.` or `.F.`.
    Bool(bool),
    /// An omitted optional attribute, `$`.
    Null,
    /// An attribute derived in a subtype, `*`.
    Derived,
    /// An aggregate, `(a,b,…)`.
    List(Vec<P>),
    /// A typed value (a defined type in a SELECT), `IFCLABEL('x')`.
    Typed(&'static str, Box<P>),
}

impl P {
    /// A STRING parameter.
    pub fn str(s: impl Into<String>) -> P {
        P::Str(s.into())
    }

    /// An aggregate of references.
    pub fn refs(r: &[Ref]) -> P {
        P::List(r.iter().map(|&r| P::Ref(r)).collect())
    }

    /// An aggregate of reals.
    pub fn reals(v: &[f64]) -> P {
        P::List(v.iter().map(|&x| P::Real(x)).collect())
    }

    /// A typed value, e.g. `P::typed("IFCLABEL", P::str("x"))`.
    pub fn typed(name: &'static str, value: P) -> P {
        P::Typed(name, Box::new(value))
    }
}

/// The HEADER section of an exchange file (ISO 10303-21 §8).
#[derive(Clone, Debug, PartialEq)]
pub struct StepHeader {
    /// FILE_DESCRIPTION's description, e.g. `ViewDefinition [ReferenceView_V1.2]`.
    pub description: Vec<String>,
    /// FILE_DESCRIPTION's implementation level, `2;1`.
    pub implementation_level: String,
    /// FILE_NAME's name.
    pub name: String,
    /// FILE_NAME's time stamp, `YYYY-MM-DDThh:mm:ss` ([`iso_timestamp`]).
    pub time_stamp: String,
    pub author: Vec<String>,
    pub organization: Vec<String>,
    pub preprocessor_version: String,
    pub originating_system: String,
    pub authorization: String,
    /// FILE_SCHEMA, e.g. `IFC4`.
    pub schema: Vec<String>,
}

impl Default for StepHeader {
    fn default() -> Self {
        StepHeader {
            description: Vec::new(),
            implementation_level: "2;1".into(),
            name: String::new(),
            time_stamp: iso_timestamp(0),
            author: vec![String::new()],
            organization: vec![String::new()],
            preprocessor_version: String::new(),
            originating_system: String::new(),
            authorization: String::new(),
            schema: Vec::new(),
        }
    }
}

/// Writes the DATA section's entity instances in order, numbering them from 1.
#[derive(Debug, Default)]
pub struct StepWriter {
    data: Vec<String>,
}

impl StepWriter {
    pub fn new() -> StepWriter {
        StepWriter::default()
    }

    /// Add the instance `entity(params)` and return its reference. `entity` is written in upper
    /// case (`IfcWall` and `IFCWALL` are the same entity).
    pub fn add(&mut self, entity: &str, params: Vec<P>) -> Ref {
        let n = u32::try_from(self.data.len() + 1).unwrap_or(u32::MAX);
        let mut line = format!("#{n}={}(", entity.to_ascii_uppercase());
        for (i, p) in params.iter().enumerate() {
            if i > 0 {
                line.push(',');
            }
            write_param(&mut line, p);
        }
        line.push_str(");");
        self.data.push(line);
        Ref(n)
    }

    /// Instances written so far.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// The whole exchange file: `ISO-10303-21;`, the HEADER, the DATA and the end line, each line
    /// ending in `\n`.
    pub fn finish(self, header: &StepHeader) -> String {
        let list = |v: &[String]| P::List(v.iter().map(|s| P::Str(s.clone())).collect());
        let mut out = String::from("ISO-10303-21;\nHEADER;\n");
        let mut line = String::from("FILE_DESCRIPTION(");
        write_param(&mut line, &list(&header.description));
        line.push(',');
        write_param(&mut line, &P::Str(header.implementation_level.clone()));
        let _ = writeln!(out, "{line});");
        let mut line = String::from("FILE_NAME(");
        for (i, p) in [
            P::Str(header.name.clone()),
            P::Str(header.time_stamp.clone()),
            list(&header.author),
            list(&header.organization),
            P::Str(header.preprocessor_version.clone()),
            P::Str(header.originating_system.clone()),
            P::Str(header.authorization.clone()),
        ]
        .iter()
        .enumerate()
        {
            if i > 0 {
                line.push(',');
            }
            write_param(&mut line, p);
        }
        let _ = writeln!(out, "{line});");
        let mut line = String::from("FILE_SCHEMA(");
        write_param(&mut line, &list(&header.schema));
        let _ = writeln!(out, "{line});");
        out.push_str("ENDSEC;\nDATA;\n");
        for l in &self.data {
            out.push_str(l);
            out.push('\n');
        }
        out.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
        out
    }
}

fn write_param(out: &mut String, p: &P) {
    match p {
        P::Ref(r) => {
            let _ = write!(out, "#{}", r.0);
        }
        P::Str(s) => {
            out.push('\'');
            out.push_str(&encode_string(s));
            out.push('\'');
        }
        P::Enum(e) => {
            let _ = write!(out, ".{}.", e.to_ascii_uppercase());
        }
        P::Real(v) => out.push_str(&real(*v)),
        P::Int(i) => {
            let _ = write!(out, "{i}");
        }
        P::Bool(b) => out.push_str(if *b { ".T." } else { ".F." }),
        P::Null => out.push('$'),
        P::Derived => out.push('*'),
        P::List(items) => {
            out.push('(');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_param(out, item);
            }
            out.push(')');
        }
        P::Typed(name, value) => {
            out.push_str(&name.to_ascii_uppercase());
            out.push('(');
            write_param(out, value);
            out.push(')');
        }
    }
}

/// A string's content as ISO 10303-21:2016 §7.3.3 writes it between apostrophes: `'` doubled,
/// `\` doubled, printable ASCII as it is, control characters as `\X\hh`, other characters of the
/// Basic Multilingual Plane as a `\X2\hhhh…\X0\` run of UTF-16 units and characters above it as a
/// `\X4\hhhhhhhh…\X0\` run.
pub fn encode_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut run: Option<(bool, String)> = None; // (wide, hex digits) of the open \X2\ or \X4\ run
    let close = |out: &mut String, run: &mut Option<(bool, String)>| {
        if let Some((wide, hex)) = run.take() {
            out.push_str(if wide { "\\X4\\" } else { "\\X2\\" });
            out.push_str(&hex);
            out.push_str("\\X0\\");
        }
    };
    for c in s.chars() {
        let u = c as u32;
        if (0x20..=0x7e).contains(&u) {
            close(&mut out, &mut run);
            match c {
                '\'' => out.push_str("''"),
                '\\' => out.push_str("\\\\"),
                _ => out.push(c),
            }
        } else if u < 0x20 || u == 0x7f {
            close(&mut out, &mut run);
            let _ = write!(out, "\\X\\{u:02X}");
        } else {
            let wide = u > 0xffff;
            if run.as_ref().is_some_and(|(w, _)| *w != wide) {
                close(&mut out, &mut run);
            }
            let (_, hex) = run.get_or_insert_with(|| (wide, String::new()));
            if wide {
                let _ = write!(hex, "{u:08X}");
            } else {
                let _ = write!(hex, "{u:04X}");
            }
        }
    }
    close(&mut out, &mut run);
    out
}

/// A REAL as ISO 10303-21 writes it: always a decimal point (`1.`, `0.5`, `-2.25`), an exponent
/// as `E` (`1.E-06`). Negative zero is `0.`; a non-finite value (which IFC cannot hold) is `0.`.
pub fn real(v: f64) -> String {
    if !v.is_finite() || v == 0.0 {
        return "0.".into();
    }
    let s = format!("{v:?}"); // shortest round-trip form: "1.0", "0.5", "1e-6", "1.5e20"
    let (mant, exp) = match s.split_once('e') {
        Some((m, e)) => (m.to_string(), Some(e.to_string())),
        None => (s, None),
    };
    let mant = if let Some(m) = mant.strip_suffix(".0") {
        format!("{m}.")
    } else if mant.contains('.') {
        mant
    } else {
        format!("{mant}.")
    };
    match exp {
        Some(e) => {
            let (sign, digits) = e.strip_prefix('-').map_or(("", e.as_str()), |d| ("-", d));
            format!("{mant}E{sign}{digits:0>2}")
        }
        None => mant,
    }
}

/// `YYYY-MM-DDThh:mm:ss` (UTC) for seconds since 1970-01-01.
pub fn iso_timestamp(epoch_secs: i64) -> String {
    let days = epoch_secs.div_euclid(86_400);
    let secs = epoch_secs.rem_euclid(86_400);
    // Days to a civil date (H. Hinnant's algorithm), valid for every i64 day count used here.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}", secs / 3600, secs % 3600 / 60, secs % 60)
}

/// `SOURCE_DATE_EPOCH` (seconds since 1970-01-01, the reproducible-builds convention), when set to
/// a whole number.
pub fn source_date_epoch() -> Option<i64> {
    std::env::var("SOURCE_DATE_EPOCH").ok().and_then(|v| v.trim().parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_are_encoded_as_the_standard_says() {
        assert_eq!(encode_string("Wall 1"), "Wall 1");
        assert_eq!(encode_string("it's"), "it''s");
        assert_eq!(encode_string(r"a\b"), r"a\\b");
        assert_eq!(encode_string("Küche"), r"K\X2\00FC\X0\che");
        assert_eq!(encode_string("Кухня"), r"\X2\041A04430445043D044F\X0\");
        assert_eq!(encode_string("a😀b"), r"a\X4\0001F600\X0\b");
        assert_eq!(encode_string("é😀"), r"\X2\00E9\X0\\X4\0001F600\X0\");
        assert_eq!(encode_string("a\nb"), r"a\X\0Ab");
    }

    #[test]
    fn reals_always_carry_a_decimal_point() {
        assert_eq!(real(1.0), "1.");
        assert_eq!(real(0.5), "0.5");
        assert_eq!(real(-2.25), "-2.25");
        assert_eq!(real(0.0), "0.");
        assert_eq!(real(-0.0), "0.");
        assert_eq!(real(1e-6), "1.E-06");
        assert_eq!(real(1.5e20), "1.5E20");
        assert_eq!(real(f64::NAN), "0.");
        assert_eq!(real(2400.0), "2400.");
        for v in [1.0, 0.1, 1.0 / 3.0, 12345.678, 1e-300, -7.5e-5] {
            let back: f64 = real(v).replace('E', "e").trim_end_matches('.').parse().unwrap_or(f64::NAN);
            assert_eq!(back, v, "{v} round-trips through {}", real(v));
        }
    }

    #[test]
    fn instances_are_numbered_from_one_in_the_order_added() {
        let mut w = StepWriter::new();
        let a = w.add("IfcCartesianPoint", vec![P::reals(&[0.0, 0.0, 1.0])]);
        let b = w.add("IfcDirection", vec![P::reals(&[0.0, 0.0, 1.0])]);
        let c = w.add("ifcAxis2Placement3D", vec![P::Ref(a), P::Ref(b), P::Null]);
        assert_eq!((a, b, c), (Ref(1), Ref(2), Ref(3)));
        let text = w.finish(&StepHeader::default());
        assert!(text.contains("#1=IFCCARTESIANPOINT((0.,0.,1.));\n"), "{text}");
        assert!(text.contains("#3=IFCAXIS2PLACEMENT3D(#1,#2,$);\n"), "{text}");
    }

    #[test]
    fn every_parameter_kind_is_written() {
        let mut w = StepWriter::new();
        w.add(
            "X",
            vec![
                P::str("n"),
                P::Enum("element"),
                P::Int(-3),
                P::Bool(true),
                P::Bool(false),
                P::Null,
                P::Derived,
                P::List(vec![P::refs(&[Ref(1), Ref(2)]), P::List(vec![])]),
                P::typed("IfcLabel", P::str("x")),
            ],
        );
        let text = w.finish(&StepHeader::default());
        assert!(text.contains("#1=X('n',.ELEMENT.,-3,.T.,.F.,$,*,((#1,#2),()),IFCLABEL('x'));"), "{text}");
    }

    #[test]
    fn the_file_has_header_data_and_end() {
        let header = StepHeader {
            description: vec!["ViewDefinition [ReferenceView_V1.2]".into()],
            name: "house.ifc".into(),
            time_stamp: iso_timestamp(1_793_556_000),
            schema: vec!["IFC4".into()],
            ..StepHeader::default()
        };
        let mut w = StepWriter::new();
        w.add("IfcProject", vec![]);
        let text = w.finish(&header);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "ISO-10303-21;");
        assert_eq!(lines[1], "HEADER;");
        assert_eq!(lines[2], "FILE_DESCRIPTION(('ViewDefinition [ReferenceView_V1.2]'),'2;1');");
        assert_eq!(lines[3], "FILE_NAME('house.ifc','2026-11-01T18:00:00',(''),(''),'','','');");
        assert_eq!(lines[4], "FILE_SCHEMA(('IFC4'));");
        assert_eq!(&lines[5..8], ["ENDSEC;", "DATA;", "#1=IFCPROJECT();"]);
        assert_eq!(&lines[8..], ["ENDSEC;", "END-ISO-10303-21;"]);
    }

    #[test]
    fn timestamps_are_utc_civil_dates() {
        assert_eq!(iso_timestamp(0), "1970-01-01T00:00:00");
        assert_eq!(iso_timestamp(951_782_400), "2000-02-29T00:00:00");
        assert_eq!(iso_timestamp(1_793_556_000), "2026-11-01T18:00:00");
        assert_eq!(iso_timestamp(-1), "1969-12-31T23:59:59");
    }
}
