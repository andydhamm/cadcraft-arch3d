//! Tag-layer behaviour of `craft-dxf`, the DXF reader/writer shared through craft-libs.
//!
//! These tests lived in the in-tree `cadcraft-dxf` crate until it moved to craft-libs; they are
//! kept here, under their old names, against the public API, so CADCraft still guards the
//! behaviour it relies on (escapes, code pages, hostile input, tag limits).

use craft_dxf::{
    DxfError, MAX_TAGS, Tag, Version, decode_code_page, parse, parse_ascii, parse_binary, parse_with_limit, records, sections, write_ascii,
    write_ascii_as, write_binary,
};

const BINARY_SENTINEL: &[u8] = b"AutoCAD Binary DXF\r\n\x1a\0";

#[test]
fn literal_escape_text_roundtrips() {
    for utf8 in [false, true] {
        for text in ["\\U+00E9", "\\M+183A4F", "a\\nb \\P", "\\\\U+00E9", "\\U+005C"] {
            let tags = vec![Tag::s(8, text)];
            let back = parse_ascii(&write_ascii_as(&tags, utf8)).unwrap();
            assert_eq!(back, tags, "utf8={utf8} {text}");
            let back = parse_binary(&write_binary(&tags, utf8)).unwrap();
            assert_eq!(back[1..], tags[..], "binary utf8={utf8} {text}");
        }
    }
}

#[test]
fn blank_lines_do_not_consume_group_code_limit() {
    let tags = parse_with_limit(b"0\nEOF\n\n\n", 1).expect("one group code should fit a limit of one");
    assert_eq!(tags, vec![Tag::s(0, "EOF")]);
}

#[test]
fn ascii_roundtrip() {
    let tags = vec![
        Tag::s(0, "SECTION"),
        Tag::s(2, "ENTITIES"),
        Tag::s(0, "LINE"),
        Tag::s(8, "0"),
        Tag::f(10, 1.5),
        Tag::f(20, 2.0),
        Tag::i(62, 1),
        Tag::s(0, "ENDSEC"),
        Tag::s(0, "EOF"),
    ];
    let text = write_ascii(&tags);
    let back = parse(text.as_bytes()).unwrap();
    assert_eq!(back, tags);
    let secs = sections(&back);
    assert_eq!(secs.len(), 1);
    assert_eq!(secs[0].name, "ENTITIES");
    let recs = records(&secs[0].tags);
    assert_eq!(recs[0].0, "LINE");
    assert_eq!(recs[0].1.len(), 4);
}

#[test]
fn binary_parse() {
    let mut b = BINARY_SENTINEL.to_vec();
    let push_str = |b: &mut Vec<u8>, code: i16, s: &str| {
        b.extend_from_slice(&code.to_le_bytes());
        b.extend_from_slice(s.as_bytes());
        b.push(0);
    };
    push_str(&mut b, 0, "LINE");
    b.extend_from_slice(&10i16.to_le_bytes());
    b.extend_from_slice(&3.25f64.to_le_bytes());
    b.extend_from_slice(&62i16.to_le_bytes());
    b.extend_from_slice(&5i16.to_le_bytes());
    push_str(&mut b, 0, "EOF");
    let tags = parse(&b).unwrap();
    assert_eq!(tags[1], Tag::f(10, 3.25));
    assert_eq!(tags[2], Tag::i(62, 5));
}

#[test]
fn malformed_inputs() {
    assert!(parse(b"").is_err());
    assert!(parse(b"abc\nxyz\n").is_err());
    assert!(parse(b"0\n").is_err());
    let mut b = BINARY_SENTINEL.to_vec();
    b.extend_from_slice(&10i16.to_le_bytes());
    b.extend_from_slice(&[1, 2, 3]);
    assert!(parse(&b).is_err());
    // Bad numbers become 0 rather than failing.
    let t = parse(b"10\nnotanumber\n0\nEOF\n").unwrap();
    assert_eq!(t[0].f64(), 0.0);
}

#[test]
fn unicode_escapes_and_latin1() {
    let t = parse(b"1\nCaf\\U+00E9\n0\nEOF\n").unwrap();
    assert_eq!(t[0].str(), "Café");
    let t = parse(b"1\nCaf\xe9\n0\nEOF\n").unwrap();
    assert_eq!(t[0].str(), "Café");
}

#[test]
fn surrogate_pair_escapes() {
    let t = parse(b"1\n\\U+D83D\\U+DE00 \\u+56fe\\U+53f7\n0\nEOF\n").unwrap();
    assert_eq!(t[0].str(), "😀 \\u+56fe号");
}

#[test]
fn hostile_escapes_stay_literal() {
    for s in [
        "\\U+",
        "\\U",
        "\\",
        "a\\U+ZZZZ",
        "\\U+12",
        "\\U+D800",
        "\\U+DC00",
        "\\U+D800\\U+0041",
        "\\U+D83D\\U+DE",
        "\\U++123",
        "\\U+-123",
        "x\\U+12\u{e9}4",
    ] {
        let t = parse(format!("1\n{s}\n0\nEOF\n").as_bytes()).unwrap();
        let expect = if s == "\\U+D800\\U+0041" { "\\U+D800A" } else { s };
        assert_eq!(t[0].str(), expect, "{s:?}");
    }
}

#[test]
fn writes_non_ascii_as_escapes() {
    let tags = vec![Tag::s(1, "图号 Ä°ø 😀\r\nend"), Tag::s(1000, "Ø"), Tag::s(0, "EOF")];
    let text = write_ascii(&tags);
    assert!(text.is_ascii());
    assert!(text.contains("\\U+56FE\\U+53F7 \\U+00C4\\U+00B0\\U+00F8 \\U+D83D\\U+DE00  end\r\n"));
    let back = parse(text.as_bytes()).unwrap();
    assert_eq!(back[0].str(), "图号 Ä°ø 😀  end");
    assert_eq!(back[1].str(), "Ø");
}

#[test]
fn binary_strings_unescape() {
    let mut b = BINARY_SENTINEL.to_vec();
    b.extend_from_slice(&1i16.to_le_bytes());
    b.extend_from_slice(b"\\U+56FE\\U+53F7\0");
    b.extend_from_slice(&0i16.to_le_bytes());
    b.extend_from_slice(b"EOF\0");
    assert_eq!(parse(&b).unwrap()[0].str(), "图号");
}

#[test]
fn code_page_names() {
    let d = |bytes: &[u8], cp: &str| decode_code_page(bytes, cp);
    assert_eq!(d(b"\xCF\xF0\xE8\xE2\xE5\xF2", "ANSI_1251").as_deref(), Some("Привет"));
    assert_eq!(d(b"\xCF\xF0\xE8\xE2\xE5\xF2", " ansi_1251 ").as_deref(), Some("Привет"));
    assert_eq!(d(b"\x90\x7D\x96\xCA", "ANSI_932").as_deref(), Some("図面"));
    assert_eq!(d(b"\xCD\xBC\xB2\xE3", "GB2312").as_deref(), Some("图层"));
    assert_eq!(d(b"\xB9\xCF\xBC\x68", "BIG5").as_deref(), Some("圖層"));
    assert_eq!(d(b"\xB5\xB5\xB8\xE9", "ANSI_949").as_deref(), Some("도면"));
    assert_eq!(d(b"\xD3\xF7\xDD\xE4\xE9\xEF", "ANSI_1253").as_deref(), Some("Σχέδιο"));
    assert_eq!(d(b"\xC0\xD2\xBE", "ANSI_874").as_deref(), Some("ภาพ"));
    assert_eq!(d(b"\x80", "ANSI_1252").as_deref(), Some("€"));
    // DOS 437 and 850 agree on letters but differ on 0x9B (¢ / ø).
    assert_eq!(d(b"Gr\x94\xE1e \x9B", "DOS437").as_deref(), Some("Größe ¢"));
    assert_eq!(d(b"Gr\x94\xE1e \x9B", "dos850").as_deref(), Some("Größe ø"));
    assert_eq!(d(b"\xBD\xA2\x88w", "DOS852").as_deref(), Some("Żółw"));
    assert_eq!(d(b"\x91\xAB\xAE\xA9", "DOS866").as_deref(), Some("Слой"));
    assert_eq!(d("é".as_bytes(), "UTF8").as_deref(), Some("é"));
    assert_eq!(d(b"x", "ANSI_1361"), None);
    assert_eq!(d(b"x", "EBCDIC"), None);
    assert_eq!(d(b"x", ""), None);
}

#[test]
fn invalid_sequences_become_replacement_characters() {
    // A Shift-JIS lead byte cut off by the line end keeps the line end.
    assert_eq!(decode_code_page(b"\x90\n1\n", "ANSI_932").as_deref(), Some("\u{FFFD}\n1\n"));
    assert_eq!(decode_code_page(b"\x90", "ANSI_932").as_deref(), Some("\u{FFFD}"));
    assert_eq!(decode_code_page(b"\xFF\xFF", "ANSI_936").as_deref(), Some("\u{FFFD}\u{FFFD}"));
}

/// The header's `$DWGCODEPAGE` decides how a pre-2007 file's bytes read; only the HEADER counts.
#[test]
fn header_scan() {
    let head = b"0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n9\n$DWGCODEPAGE\n3\nANSI_1251\r\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nTEXT\n1\n\xCF\xF0\n0\nENDSEC\n0\nEOF\n";
    let tags = parse(head).unwrap();
    assert!(tags.iter().any(|t| t.code == 1 && t.str() == "Пр"));
    // Outside the HEADER section the code page is ignored: the bytes read as Latin-1.
    let late = b"0\nSECTION\n2\nHEADER\n0\nENDSEC\n9\n$DWGCODEPAGE\n3\nANSI_1251\n1\n\xCF\xF0\n0\nEOF\n";
    let tags = parse(late).unwrap();
    assert!(tags.iter().any(|t| t.code == 1 && t.str() == "Ïð"));
}

#[test]
fn mif_escapes() {
    let one = |s: &str| parse(format!("1\n{s}\n0\nEOF\n").as_bytes()).unwrap()[0].str();
    assert_eq!(one("\\M+5BCFE\\M+1907D\\M+2B9CF\\M+3B5B5!"), "件図圖도!");
    assert_eq!(one("\\M+5BCFE\\U+0021"), "件!");
    // Johab, unknown code pages, bad hex, short escapes and unassigned codes stay literal.
    for s in ["\\M+4ABCD", "\\M+9BCFE", "\\M+5GGGG", "\\M+5BC", "\\M+", "\\M+50041", "\\M+5FFFF"] {
        assert_eq!(one(s), s);
    }
}

#[test]
fn parses_years_and_codes() {
    assert_eq!(Version::parse("2018"), Some(Version::R2018));
    assert_eq!(Version::parse(" r2004 "), Some(Version::R2004));
    assert_eq!(Version::parse("ac1021"), Some(Version::R2007));
    assert_eq!(Version::parse("2005"), None);
    assert_eq!(Version::parse(""), None);
    for v in Version::ALL {
        assert_eq!(Version::parse(v.code()), Some(v));
        assert_eq!(Version::parse(v.year()), Some(v));
    }
}

#[test]
fn file_versions_map_to_the_nearest_writable_one() {
    assert_eq!(Version::for_acadver("AC1009"), Some(Version::R2000));
    assert_eq!(Version::for_acadver("AC1014"), Some(Version::R2000));
    assert_eq!(Version::for_acadver("AC1024"), Some(Version::R2010));
    assert_eq!(Version::for_acadver("AC1035"), Some(Version::R2018));
    assert_eq!(Version::for_acadver("MC0.0"), None);
    assert_eq!(Version::for_acadver("AC"), None);
    assert_eq!(Version::for_acadver("AC99999999999999999999"), None);
}

#[test]
fn tag_limit_defaults_and_env_override() {
    // Issue #1948: the default must admit a DXF rendition well above 50 million group codes.
    const { assert!(MAX_TAGS >= 100_000_000) };
    // A limit smaller than the file is an error that names both numbers, not a truncation.
    let e = parse_with_limit(b"0\nSECTION\n0\nENDSEC\n0\nEOF\n", 2).unwrap_err();
    assert!(matches!(e, DxfError::TooLarge { limit: 2, .. }), "{e}");
}

#[test]
fn the_tag_list_is_reserved_once_and_a_refusal_is_an_error() {
    let tags = parse_with_limit(b"0\nSECTION\n0\nENDSEC\n0\nEOF\n", 3).unwrap();
    assert_eq!(tags.len(), 3);
    // An absurd limit must never abort the process: it parses or reports an error.
    let _ = parse_with_limit(b"0\nEOF\n", usize::MAX);
}
