/// The built `cadcraft-cli` binary. Under `cargo test` the integration test executable
/// lives in `<root>/target/<profile>/deps/<hash>`; the CLI binary sits at
/// `<root>/target/<profile>/cadcraft-cli`.
fn binary() -> PathBuf {
    match std::env::current_exe() {
        Ok(exe) => {
            let base = exe.parent().and_then(Path::parent);
            base.map_or_else(|| PathBuf::from("target/debug/cadcraft-cli"), |b| b.join("cadcraft-cli"))
        }
        Err(_) => PathBuf::from("target/debug/cadcraft-cli"),
    }
}

/// Run the CLI binary with the given arguments and return its full output.
fn run_cli(args: &[String]) -> Output {
    let mut cmd = Command::new(binary());
    for a in args {
        cmd.arg(a);
    }
    cmd.output().expect("run cadcraft-cli")
}

/// A temp directory unique to this test process and test name (no cross-test clobbering).
fn temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cadcraft-cli-{}-{}", std::process::id(), test_name));
    fs.create_dir_all(&dir).expect("create temp dir");
    dir
}

/// True when `path` is a valid PNG (magic bytes `89 50 4E 47`).
fn is_png(path: &Path) -> bool {
    match fs::read(path) {
        Ok(bytes) => bytes.len() >= 4 && bytes[0..4] == [0x89, 0x50, 0x4e, 0x47],
        Err(_) => false,
    }
}

/// The camera `level_cam` in tests/acceptance/phase-19/p19lib.inc, as a `--cmd` camera value:
/// `kind full`, `eye` unchanged, `target = eye + 10*(cos p cos y, cos p sin y, sin p)`.
fn level_cam(eye: [f64; 3], yaw: f64, pitch: f64, fov: f64) -> String {
    let y = yaw * std::f64::consts::PI / 180.0;
    let p = pitch * std::f64::consts::PI / 180.0;
    let tx = eye[0] + 10.0 * (f64::cos(p) * f64::cos(y));
    let ty = eye[1] + 10.0 * (f64::cos(p) * f64::sin(y));
    let tz = eye[2] + 10.0 * f64::sin(p);
    format!(
        "{{\"kind\":\"full\",\"eye\":[{},{},{}],\"target\":[{:.8},{:.8},{:.8}],\"fov\":{}}}",
        eye[0], eye[1], eye[2], tx, ty, tz, fov,
    )
}

/// One `--cmd` for a `render` in the given look (tag), mode and sky-star setting.
fn render_cmd_json(
    tmp: &str, tag: &str, mode: &str, camera: &str, angle: &str, ev: i64, stars: Option<bool>,
) -> String {
    let out = format!("{tmp}/{tag}-{mode}.png");
    let mode_frag = match mode {
        "interactive" => String::new(),
        "raster" => String::from(r#"","final":"raster"#),
        "path" => String::from(r#"","final":"path","spp":4,"bounces":0,"seed":0,"denoise":false"#),
        _ => panic!("bad mode: {mode}"),
    };
    let sky_frag = match stars {
        None => String::new(),
        Some(true) => String::from(r#","sky":{"stars":true}#),
        Some(false) => String::from(r#","sky":{"stars":false}#),
    };
    format!(
        "render {{\"out\":\"{out}\",\"size\":[60,45],\"camera\":{camera},\"sun\":{{\"angle\":\"{angle}\"}},\"exposure\":{{\"ev100\":[ev]}},\"toneMap\":\"none\"{sky_frag}{mode_frag}}}"
    )
}

/// The 8 sky looks of check 03 (tag, camera, sun angle, EV100, star flag).
fn looks(eye: [f64; 3]) -> [(
    &'static str, &'static str, &'static str, i64, Option<bool>,
)] {
    let up = level_cam(eye, 90.0, 70.0, 30.0);
    let west = level_cam(eye, 180.0, 4.0, 60.0);
    let zen = level_cam(eye, 90.0, 89.0, 60.0);
    [
        ("noon", &up, "noon", 15, None),
        ("low", &west, "low", 15, None),
        ("m10", &up, "m10", 15, None),
        ("w2", &west, "low", 8, None),
        ("w3", &west, "m3", 8, None),
        ("w10", &west, "m10", 8, None),
        ("stars", &zen, "deep", -6, Some(true)),
        ("nostars", &zen, "deep", -6, Some(false)),
    ]
}

/// The static drawing for the sky check: site, one wall and five sun angles + sky backdrop.
fn setup_cmds() -> Vec<String> {
    let cmds = [
        r#"site {"latitude":51.05,"longitude":-114.07,"trueNorth":0}"#,
        r#"wall {"points":[[2,-3],[3,-3]],"thickness":0.2,"height":1.0,"exterior":false}"#,
        r#"sunangle {"name":"noon","date":"2026-06-21","time":"solar-noon"}"#,
        r#"sunangle {"name":"low","azimuth":270,"elevation":2}"#,
        r#"sunangle {"name":"m3","azimuth":270,"elevation":-3}"#,
        r#"sunangle {"name":"m10","azimuth":270,"elevation":-10}"#,
        r#"sunangle {"name":"deep","azimuth":0,"elevation":-20}"#,
        r#"backdrop {"kind":"sky"}"#,
    ];
    cmds.into_iter().map(String::from).collect()
}

fn is_render_reply(v: &Value) -> bool {
    v.get("ok").map_or(false, Value::is_true)
        && v.get("samples").is_some()
        && v.get("finalMode").is_some()
}

/// The 24 renders of check 03 (8 sky looks x 3 modes) through the CLI in one process.
#[test]
fn eight_sky_looks_render_in_three_modes_quickly() {
    let tmp = temp_dir("sky_time");
    let eye = [0.0, 0.0, 1.6];

    // Probe: setup + one interactive render. Skip if there is no render adapter.
    let first = render_cmd_json(&tmp, "noon", "interactive", &level_cam(eye, 90.0, 70.0, 30.0), "noon", 15, None);
    let probe_args = {
        let mut a = vec!["run".to_string(), "--metric".to_string()];
        for c in &setup_cmds() { a.push("--cmd".to_string()); a.push(c); }
        a.push("--cmd".to_string());
        a.push(first);
        a
    };
    let probe = run_cli(&probe_args);
    if !probe.status.success() || String::from_utf8_lossy(&probe.stderr).contains("NO_ADAPTER") {
        eprintln!("SKIP: no render adapter (rc={})", probe.status.code());
        return;
    }

    // Time the single process that makes all 24 renders.
    let (looks, modes) = (looks(eye), ["interactive", "raster", "path"]);
    let mut args = vec!["run".to_string(), "--metric".to_string()];
    for c in &setup_cmds() { args.push("--cmd".to_string()); args.push(c); }
    for (tag, cam, angle, ev, stars) in &looks {
        for mode in &modes {
            args.push("--cmd".to_string());
            args.push(render_cmd_json(&tmp, tag, mode, cam, angle, *ev, *stars));
        }
    }

    let start = Instant::now();
    let out = run_cli(&args);
    let elapsed = start.elapsed();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();

    assert!(
        out.status.success(),
        "run failed (rc={}): {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );

    // Count the render replies (JSON rows with ok:true + "samples" + "finalMode").
    let mut replies = 0;
    for line in stdout.lines() {
        if line.trim().is_empty() { continue; }
        match serde_json::from_str::<Value>(line) {
            Ok(v) if is_render_reply(&v) => replies += 1,
            _ => {}
        }
    }
    assert_eq!(replies, 24, "expected 24 render replies, got {replies}\n{stdout}");

    // All 24 PNGs exist and are valid.
    for (tag, _, _, _, _) in &looks {
        for mode in &modes {
            let p = tmp.join(format!("{tag}-{mode}.png"));
            assert!(p.exists() && is_png(&p), "missing or invalid png {}", p.display());
        }
    }

    assert!(elapsed.as_secs() < 120, "24 renders too slow: {elapsed}");
    println!("eight sky looks in 3 modes in {elapsed:?}");
}
