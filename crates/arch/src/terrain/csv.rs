//! Survey points from CSV text.

use cadcraft_geom::Vec3;

use super::TerrainError;
use cadcraft_doc::arch::MAX_TERRAIN_POINTS;

/// Split a row into trimmed fields: commas first, else `;` or tab.
fn fields(row: &str) -> Vec<&str> {
    let sep = if row.contains(',') {
        ','
    } else if row.contains(';') {
        ';'
    } else {
        '\t'
    };
    row.split(sep).map(str::trim).collect()
}

/// Parse a number from the start of a string, stopping at the first non-numeric character.
fn parse_number(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let end = s.find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-' && c != '+' && c != 'e' && c != 'E').unwrap_or(s.len());
    s[..end].parse().ok()
}

/// Three finite numbers, or `None`.
fn numbers(f: &[&str]) -> Option<[f64; 3]> {
    if f.len() != 3 {
        return None;
    }
    let mut out = [0.0; 3];
    for (slot, s) in out.iter_mut().zip(f) {
        let v = parse_number(s)?;
        if !v.is_finite() {
            return None;
        }
        *slot = v;
    }
    Some(out)
}

/// Parse `x,y,z` rows; `offset` (easting, northing) is subtracted from x and y when given.
pub fn parse_points_csv(text: &str, offset: Option<(f64, f64)>) -> Result<Vec<Vec3>, TerrainError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let (de, dn) = offset.unwrap_or((0.0, 0.0));
    let mut points = Vec::new();
    let mut first = true;
    for (i, raw) in text.lines().enumerate() {
        let row = raw.trim();
        if row.is_empty() || row.starts_with('#') {
            continue;
        }
        let f = fields(row);
        let parsed = numbers(&f);
        if first {
            first = false;
            // A header: none of its three fields is a number.
            if f.len() == 3 && f.iter().all(|s| parse_number(s).is_none()) {
                continue;
            }
        }
        let Some([x, y, z]) = parsed else {
            let shown: String = row.chars().take(60).collect();
            return Err(TerrainError(format!("CSV line {}: expected x,y,z numbers, got `{shown}`", i + 1)));
        };
        if points.len() >= MAX_TERRAIN_POINTS {
            return Err(TerrainError(format!("CSV has more than {MAX_TERRAIN_POINTS} points")));
        }
        let (px, py) = (x - de, y - dn);
        if !px.is_finite() || !py.is_finite() {
            return Err(TerrainError(format!("CSV line {}: coordinates out of range", i + 1)));
        }
        points.push(Vec3::new(px, py, z));
    }
    if points.is_empty() {
        return Err(TerrainError("CSV has no points".into()));
    }
    Ok(points)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_reads_rows_and_skips_a_header() {
        let p = parse_points_csv("x,y,z\n1,2,3\n 4 ; 5 ; 6 \n", None);
        // second row has no comma, uses ';'
        let p = p.unwrap_or_default();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0], Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(p[1], Vec3::new(4.0, 5.0, 6.0));
        let t = parse_points_csv("1\t2\t3", None).unwrap_or_default();
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn csv_subtracts_the_reference_offset() {
        let p = parse_points_csv("100,200,5", Some((90.0, 150.0))).unwrap_or_default();
        assert_eq!(p, vec![Vec3::new(10.0, 50.0, 5.0)]);
    }

    #[test]
    fn csv_reports_the_bad_line_number() {
        let e = parse_points_csv("x,y,z\n1,2,3\n1,2\n", None).err().map(|e| e.0);
        assert_eq!(e.as_deref(), Some("CSV line 3: expected x,y,z numbers, got `1,2`"));
        let long = format!("1,2,{}", "z".repeat(100));
        let e = parse_points_csv(&format!("1,2,3\n{long}"), None).err().map(|e| e.0).unwrap_or_default();
        assert!(e.starts_with("CSV line 2:"));
        assert!(e.chars().count() < 140);
    }

    #[test]
    fn csv_refuses_non_finite_and_too_many_rows() {
        assert!(parse_points_csv("1,2,3\n1,2,NaN", None).is_err());
        assert!(parse_points_csv("1,2,3\n1,inf,3", None).is_err());
        let many = "1,2,3\n".repeat(MAX_TERRAIN_POINTS + 1);
        let e = parse_points_csv(&many, None).err().map(|e| e.0);
        assert_eq!(e.as_deref(), Some("CSV has more than 20000 points"));
        let ok = "1,2,3\n".repeat(MAX_TERRAIN_POINTS);
        assert_eq!(parse_points_csv(&ok, None).map(|v| v.len()).unwrap_or(0), MAX_TERRAIN_POINTS);
        let e = parse_points_csv("# only\n\n", None).err().map(|e| e.0);
        assert_eq!(e.as_deref(), Some("CSV has no points"));
    }

    #[test]
    fn csv_accepts_crlf_bom_and_comments() {
        let p = parse_points_csv("\u{feff}# survey\r\nx,y,z\r\n\r\n1,2,3\r\n# mid\r\n4,5,6\r\n", None).unwrap_or_default();
        assert_eq!(p.len(), 2);
        assert_eq!(p[1], Vec3::new(4.0, 5.0, 6.0));
    }

    #[test]
    fn csv_rejects_four_field_row() {
        let e = parse_points_csv("1,2,3,4", None).err().map(|e| e.0);
        assert_eq!(e.as_deref(), Some("CSV line 1: expected x,y,z numbers, got `1,2,3,4`"));
    }

    #[test]
    fn csv_parses_single_point() {
        let p = parse_points_csv("1,2,3", None).unwrap_or_default();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0], Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn csv_parses_negative_and_scientific() {
        let p = parse_points_csv("-1.5,2e3,3.14", None).unwrap_or_default();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0], Vec3::new(-1.5, 2000.0, 3.14));
    }

    #[test]
    fn csv_parses_large_finite_values() {
        let p = parse_points_csv("1e308,1e308,1e308", None).unwrap_or_default();
        assert_eq!(p.len(), 1);
        assert!(p[0].x.is_finite());
        assert!(p[0].y.is_finite());
        assert!(p[0].z.is_finite());
    }

    #[test]
    fn csv_offset_can_produce_non_finite() {
        // x = 1e308, offset = (-1e308, 0) -> px = 2e308 = inf
        let e = parse_points_csv("1e308,1,1", Some((-1e308, 0.0))).err().map(|e| e.0);
        assert_eq!(e.as_deref(), Some("CSV line 1: coordinates out of range"));
    }

    #[test]
    fn csv_four_field_header_is_not_skipped() {
        // A header with 4 fields is not detected as a header (spec requires 3 fields)
        let e = parse_points_csv("id,x,y,z\n1,2,3", None).err().map(|e| e.0);
        assert_eq!(e.as_deref(), Some("CSV line 1: expected x,y,z numbers, got `id,x,y,z`"));
    }

    #[test]
    fn csv_empty_string_is_error() {
        let e = parse_points_csv("", None).err().map(|e| e.0);
        assert_eq!(e.as_deref(), Some("CSV has no points"));
    }

    #[test]
    fn csv_only_whitespace_is_error() {
        let e = parse_points_csv("   \n  \n  ", None).err().map(|e| e.0);
        assert_eq!(e.as_deref(), Some("CSV has no points"));
    }

    #[test]
    fn csv_only_comments_is_error() {
        let e = parse_points_csv("# comment\n# another\n", None).err().map(|e| e.0);
        assert_eq!(e.as_deref(), Some("CSV has no points"));
    }

    #[test]
    fn csv_multiple_consecutive_blank_lines() {
        let p = parse_points_csv("1,2,3\n\n\n\n4,5,6\n", None).unwrap_or_default();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0], Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(p[1], Vec3::new(4.0, 5.0, 6.0));
    }

    #[test]
    fn csv_trailing_comma_is_error() {
        let e = parse_points_csv("1,2,3,\n", None).err().map(|e| e.0);
        assert_eq!(e.as_deref(), Some("CSV line 1: expected x,y,z numbers, got `1,2,3,`"));
    }

    #[test]
    fn csv_leading_comma_is_error() {
        let e = parse_points_csv(",1,2\n", None).err().map(|e| e.0);
        assert_eq!(e.as_deref(), Some("CSV line 1: expected x,y,z numbers, got `,1,2`"));
    }

    #[test]
    fn csv_whitespace_only_fields_are_error() {
        let e = parse_points_csv("1, ,3\n", None).err().map(|e| e.0);
        assert_eq!(e.as_deref(), Some("CSV line 1: expected x,y,z numbers, got `1, ,3`"));
    }

    #[test]
    fn csv_very_long_line_truncated_in_error() {
        let long_val = "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdef";
        let e = parse_points_csv(&format!("1,2,{long_val}"), None).err().map(|e| e.0).unwrap_or_default();
        assert!(e.starts_with("CSV line 1:"));
        // The echoed row should be truncated to 60 chars
        assert!(e.chars().count() <= 105);
    }

    #[test]
    fn csv_unicode_in_values_is_error() {
        let e = parse_points_csv("1,2,三\n", None).err().map(|e| e.0);
        assert_eq!(e.as_deref(), Some("CSV line 1: expected x,y,z numbers, got `1,2,三`"));
    }

    #[test]
    fn csv_duplicate_points_allowed() {
        let p = parse_points_csv("1,2,3\n1,2,3\n1,2,3\n", None).unwrap_or_default();
        assert_eq!(p.len(), 3);
    }

    #[test]
    fn csv_zero_values_allowed() {
        let p = parse_points_csv("0,0,0\n", None).unwrap_or_default();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0], Vec3::new(0.0, 0.0, 0.0));
    }

    #[test]
    fn csv_mixed_separators_per_row() {
        // Each row uses its own separator
        let p = parse_points_csv("1,2,3\n4;5;6\n7\t8\t9\n", None).unwrap_or_default();
        assert_eq!(p.len(), 3);
        assert_eq!(p[0], Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(p[1], Vec3::new(4.0, 5.0, 6.0));
        assert_eq!(p[2], Vec3::new(7.0, 8.0, 9.0));
    }

    #[test]
    fn csv_header_with_numbers_is_not_skipped() {
        // If the first row has 3 numbers, it's data, not a header
        let p = parse_points_csv("1,2,3\n4,5,6\n", None).unwrap_or_default();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0], Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn csv_header_with_two_numbers_one_word_is_skipped() {
        let p = parse_points_csv("x,y,elev\n1,2,3\n", None).unwrap_or_default();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0], Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn csv_header_with_two_words_one_number_is_skipped() {
        let p = parse_points_csv("x,y,z\n1,2,3\n", None).unwrap_or_default();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0], Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn csv_offset_with_zero_is_identity() {
        let p = parse_points_csv("1,2,3", Some((0.0, 0.0))).unwrap_or_default();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0], Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn csv_offset_with_negative_values() {
        let p = parse_points_csv("10,20,30", Some((-5.0, -10.0))).unwrap_or_default();
        assert_eq!(p.len(), 1);
        // 10 - (-5) = 15, 20 - (-10) = 30
        assert_eq!(p[0], Vec3::new(15.0, 30.0, 30.0));
    }

    #[test]
    fn csv_line_numbers_count_blank_and_comment_lines() {
        // Line 1: blank, Line 2: comment, Line 3: bad data
        let e = parse_points_csv("\n# comment\n1,2\n", None).err().map(|e| e.0);
        assert_eq!(e.as_deref(), Some("CSV line 3: expected x,y,z numbers, got `1,2`"));
    }

    #[test]
    fn csv_header_line_counts_in_line_numbers() {
        // Line 1: header, Line 2: good, Line 3: bad
        let e = parse_points_csv("x,y,z\n1,2,3\n1,2\n", None).err().map(|e| e.0);
        assert_eq!(e.as_deref(), Some("CSV line 3: expected x,y,z numbers, got `1,2`"));
    }

    #[test]
    fn csv_max_points_exact_is_ok() {
        let lines: Vec<String> = (0..MAX_TERRAIN_POINTS).map(|i| format!("{i},0,0")).collect();
        let text = lines.join("\n");
        let p = parse_points_csv(&text, None).unwrap_or_default();
        assert_eq!(p.len(), MAX_TERRAIN_POINTS);
    }

    #[test]
    fn csv_max_points_plus_one_is_error() {
        let lines: Vec<String> = (0..=MAX_TERRAIN_POINTS).map(|i| format!("{i},0,0")).collect();
        let text = lines.join("\n");
        let e = parse_points_csv(&text, None).err().map(|e| e.0);
        assert_eq!(e.as_deref(), Some("CSV has more than 20000 points"));
    }

    #[test]
    fn csv_semicolon_separator_with_spaces() {
        let p = parse_points_csv("1 ; 2 ; 3\n4;5;6\n", None).unwrap_or_default();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0], Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(p[1], Vec3::new(4.0, 5.0, 6.0));
    }

    #[test]
    fn csv_tab_separator_with_spaces() {
        let p = parse_points_csv("1\t2\t3\n4\t5\t6\n", None).unwrap_or_default();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0], Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(p[1], Vec3::new(4.0, 5.0, 6.0));
    }

    #[test]
    fn csv_comma_takes_precedence_over_semicolon() {
        // Row has both comma and semicolon - comma wins
        let p = parse_points_csv("1,2;3,4\n", None).unwrap_or_default();
        // Split by comma: ["1", "2;3", "4"] -> 3 fields
        assert_eq!(p.len(), 1);
        assert_eq!(p[0], Vec3::new(1.0, 2.0, 4.0));
    }
}
