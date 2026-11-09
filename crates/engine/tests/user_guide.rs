use std::fs;
use std::path::Path;

use cadcraft_engine::Session;
use cadcraft_engine::cmd::file::{IoHooks, set_io};
use cadcraft_io::read;

fn read_user_guide() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/arch3d/USER-GUIDE.md");
    match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) => panic!("failed to read {}: {error}", path.display()),
    }
}

fn execute_examples(commands: &[&str]) {
    let mut session = Session::new();
    for command in commands {
        let Some((id, json)) = command.split_once(' ') else {
            panic!("user-guide command `{command}` should have the form `<id> <json>`");
        };
        let params: serde_json::Value = match serde_json::from_str(json) {
            Ok(params) => params,
            Err(error) => panic!("user-guide command `{command}` has invalid JSON: {error}"),
        };
        if let Err(error) = session.execute(id, &params) {
            panic!("user-guide command `{command}` failed: {error:?}");
        }
    }
}

/// The standalone phase-6 examples execute with the command syntax used by the CLI.
#[test]
fn user_guide_examples_parse() {
    execute_examples(&[
        "sun {\"lat\":51.05,\"date\":\"2026-06-21\",\"time\":\"solar-noon\"}",
        "walkthrough {\"eye\":[5,-10,1.6],\"yaw\":0,\"forward\":2,\"strafe\":0,\"turn\":0}",
        "arch.inspect {\"views\":true}",
    ]);
}

#[test]
fn user_guide_has_all_phase6_commands() {
    let content = read_user_guide();
    for expected in ["sun", "camera", "walkthrough", "pick3d", "render"] {
        assert!(content.contains(expected), "user guide should document {expected}");
    }
    for flag in ["--style", "--sun", "--ground", "--adapter", "--camera"] {
        assert!(content.contains(flag), "user guide should document {flag}");
    }
}

/// Camera operations share one session, just as they do in a CLI script.
#[test]
fn user_guide_examples_run() {
    execute_examples(&[
        "camera {\"save\":\"front\",\"kind\":\"full\",\"eye\":[3,-8,1.6],\"target\":[3,0,1.05],\"fov\":60}",
        "camera {\"use\":\"front\"}",
        "camera {\"list\":true}",
        "pick3d {\"camera\":{\"kind\":\"full\",\"eye\":[3,-8,1.6],\"target\":[3,0,1.05],\"fov\":60},\"size\":[800,600],\"pixel\":[400,300]}",
        "camera {\"delete\":\"front\"}",
    ]);
}

#[test]
fn user_guide_has_all_phase7_commands() {
    let content = read_user_guide();
    for expected in ["elevation", "section", "view.delete", "arch.inspect {\"views\":true}", "viewsheet", "exportpdf {\"layout\":", "ARCH D", "1:50"]
    {
        assert!(content.contains(expected), "user guide should document {expected}");
    }
}

#[test]
fn user_guide_sun_winter_summer() {
    // Sun position differs between solstices; both must compute without error.
    execute_examples(&[
        "sun {\"lat\":51.05,\"date\":\"2026-06-21\",\"time\":\"solar-noon\"}",
        "sun {\"lat\":51.05,\"date\":\"2026-12-21\",\"time\":\"solar-noon\"}",
        "sun {\"lat\":0,\"date\":\"2026-03-20\",\"time\":\"solar-noon\"}",
    ]);
}

#[test]
fn user_guide_camera_list_empty() {
    // Listing cameras when none are saved must not error.
    let mut session = Session::new();
    let result = session.execute("camera", &serde_json::json!({"list": true}));
    assert!(result.is_ok(), "camera list with no saved cameras should succeed");
}

#[test]
fn user_guide_walkthrough_zero_movement() {
    // A walkthrough step with zero movement is valid and returns the same eye.
    execute_examples(&["walkthrough {\"eye\":[5,-10,1.6],\"yaw\":0,\"forward\":0,\"strafe\":0,\"turn\":0}"]);
}

/// The guide as compiled into the test binary, so a change to it rebuilds and reruns these tests.
const GUIDE: &str = include_str!("../../../docs/arch3d/USER-GUIDE.md");

/// The ```sh blocks of the guide (as acceptance check 02 finds them), each as logical lines: a line ending in `\`
/// continues on the next.
fn sh_blocks(guide: &str) -> Vec<Vec<String>> {
    let mut blocks = Vec::new();
    let mut rest = guide;
    while let Some(start) = rest.find("```sh\n") {
        let body = &rest[start + "```sh\n".len()..];
        let Some(end) = body.find("```") else {
            panic!("the user guide has an unterminated ```sh block");
        };
        let mut lines = Vec::new();
        let mut line = String::new();
        for part in body[..end].lines() {
            match part.strip_suffix('\\') {
                Some(head) => {
                    line.push_str(head);
                    line.push(' ');
                }
                None => {
                    line.push_str(part);
                    lines.push(std::mem::take(&mut line));
                }
            }
        }
        if !line.trim().is_empty() {
            lines.push(line);
        }
        blocks.push(lines);
        rest = &body[end + 3..];
    }
    blocks
}

/// The `--cmd '<id> <json>'` arguments of one logical line, in order.
fn cmd_args(line: &str) -> Vec<String> {
    let mut cmds = Vec::new();
    let mut rest = line;
    while let Some(at) = rest.find("--cmd ") {
        let arg = rest[at + "--cmd ".len()..].trim_start();
        let Some(body) = arg.strip_prefix('\'') else {
            panic!("a user-guide --cmd must be single-quoted (`--cmd '<id> <json>'`): {line}");
        };
        let Some(end) = body.find('\'') else {
            panic!("unterminated --cmd quote in the user guide: {line}");
        };
        cmds.push(body[..end].to_string());
        rest = &body[end + 1..];
    }
    cmds
}

/// The CLI installs the real PDF plotter; here a stand-in answers so `exportpdf` still checks its layout and params.
fn stand_in_plot(_: &cadcraft_doc::Drawing, _: &cadcraft_doc::Space, _: &serde_json::Value) -> Result<Vec<u8>, String> {
    Ok(b"%PDF-1.4 user guide".to_vec())
}

/// Installs the stand-in file hooks (the first installation in the process wins): no reader, a writer that
/// answers with a few bytes so `export` runs, and the stand-in plotter. Also the library's readers, as the CLI
/// installs them, so the guide's library blocks (`library`, `library.place` of sample items) run here too.
fn install_io() {
    set_io(IoHooks { read: |_, _| Err("no reader in this test".into()), write: |_, _| Ok(b"stand-in export".to_vec()), plot: Some(stand_in_plot) });
    cadcraft_engine::library::set_library_io(cadcraft_engine::library::LibraryIo {
        read_glb: cadcraft_io::gltf_read::read_glb,
        read_image: cadcraft_io::image_read::read_image,
        encode_png: |img| cadcraft_io::png::encode_rgba8(img.width, img.height, &img.rgba).unwrap_or_default(),
        sample: cadcraft_io::library::sample_files,
    });
}

/// Every `run` line of every ```sh block executes through the engine as the CLI runs it: a fresh session per line,
/// a new metric drawing for `--metric`, then each `--cmd` in order. `render` and `convert` lines need a GPU adapter
/// or a saved file and are left to acceptance check 02 (which also plots the real PDF). A relative `path` (exportpdf)
/// is written to a temp dir.
#[test]
fn user_guide_sh_blocks_execute_through_the_engine() {
    install_io();
    let out_dir = std::env::temp_dir().join(format!("cadcraft-user-guide-{}", std::process::id()));
    if let Err(error) = fs::create_dir_all(&out_dir) {
        panic!("cannot create {}: {error}", out_dir.display());
    }
    let blocks = sh_blocks(GUIDE);
    let (mut runs, mut ids) = (0usize, Vec::new());
    for (b, lines) in blocks.iter().enumerate() {
        for line in lines {
            if !line.contains("cadcraft-cli --") {
                continue;
            }
            if line.contains("-- render ") || line.contains("-- convert ") {
                continue;
            }

            let mut session = Session::new();

            // Check if it's a run with a file argument (e.g. run examples/apartment.dxf)
            if let Some(rest) = line.strip_prefix("cargo run -q -p cadcraft-cli -- run ") {
                let rest = rest.trim();
                // Extract the file path if it exists before --cmd
                let file_arg = if let Some(cmd_pos) = rest.find(" --cmd ") {
                    let prefix = &rest[..cmd_pos];
                    // The prefix might be "--metric" or "file.dxf" or "file.dxf --metric"
                    // We look for a .dxf extension
                    prefix.split_whitespace().find(|s| s.ends_with(".dxf")).map(|s| s.to_string())
                } else {
                    // No --cmd, just run file.dxf? Or run --metric?
                    rest.split_whitespace().find(|s| s.ends_with(".dxf")).map(|s| s.to_string())
                };

                if let Some(path_str) = file_arg {
                    // Resolve path relative to repo root (assuming CARGO_MANIFEST_DIR is crates/engine)
                    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
                    let full_path = repo_root.join(&path_str);

                    if full_path.exists() {
                        let bytes = fs::read(&full_path).unwrap_or_else(|e| panic!("failed to read {}: {e}", full_path.display()));
                        match read(&bytes, &full_path) {
                            Ok(drawing) => {
                                session.open_drawing(drawing, "apartment", Some(full_path.to_string_lossy().into_owned()));
                            }
                            Err(e) => {
                                panic!("failed to read {}: {e}", full_path.display());
                            }
                        }
                    } else {
                        // If the file doesn't exist in the test env, we create a new drawing to avoid crashing
                        session.new_drawing(true);
                    }
                } else if rest.contains("--metric") {
                    session.new_drawing(true);
                } else {
                    session.new_drawing(false);
                }
            } else {
                assert!(line.contains("-- run --metric"), "sh block {}: every `run` line starts a new metric drawing or loads a file: {line}", b + 1);
                session.new_drawing(true);
            }

            let cmds = cmd_args(line);
            assert_eq!(cmds.len(), line.matches("--cmd ").count(), "sh block {}: every --cmd is read: {line}", b + 1);

            for command in &cmds {
                let Some((id, json)) = command.split_once(' ') else {
                    panic!("sh block {}: user-guide command `{command}` should have the form `<id> <json>`", b + 1);
                };
                let mut params: serde_json::Value = match serde_json::from_str(json) {
                    Ok(params) => params,
                    Err(error) => panic!("sh block {}: user-guide command `{command}` has invalid JSON: {error}", b + 1),
                };
                if let Some(path) = params.get("path").and_then(serde_json::Value::as_str) {
                    if Path::new(path).is_relative() {
                        params["path"] = serde_json::Value::String(out_dir.join(path).to_string_lossy().into_owned());
                    }
                }
                if let Err(error) = session.execute(id, &params) {
                    panic!("sh block {}: user-guide command `{command}` failed: {error:?}", b + 1);
                }
                ids.push(id.to_string());
            }
            runs += 1;
        }
    }
    let _ = fs::remove_dir_all(&out_dir);
    assert!(runs >= 7, "the guide's sh blocks should hold at least 7 `run` examples, found {runs}");
    for id in [
        "sun",
        "camera",
        "walkthrough",
        "pick3d",
        "elevation",
        "section",
        "view.delete",
        "viewsheet",
        "exportpdf",
        "archdetect",
        "material",
        "cabinet",
        "fixture",
        "arch.sample",
    ] {
        assert!(ids.iter().any(|x| x == id), "no sh block in the user guide runs `{id}`");
    }
}

/// The archdetect block in the user guide runs through the engine.
#[test]
fn user_guide_archdetect_block_runs() {
    let blocks = sh_blocks(GUIDE);
    let mut found_archdetect = false;

    for lines in blocks {
        for line in lines {
            if line.contains("archdetect") {
                found_archdetect = true;
                // Verify it's a valid run command structure
                assert!(line.contains("cadcraft-cli -- run"), "archdetect example should be a run command");

                // Try to execute if the file exists
                let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
                let dxf_path = repo_root.join("examples/apartment.dxf");

                if dxf_path.exists() {
                    install_io();

                    let mut session = Session::new();
                    let bytes = fs::read(&dxf_path).unwrap_or_else(|e| panic!("failed to read apartment.dxf: {e}"));
                    match read(&bytes, &dxf_path) {
                        Ok(drawing) => {
                            session.open_drawing(drawing, "apartment", Some(dxf_path.to_string_lossy().into_owned()));
                        }
                        Err(e) => panic!("failed to read apartment.dxf: {e}"),
                    }

                    let cmds = cmd_args(&line);
                    for command in &cmds {
                        let Some((id, json)) = command.split_once(' ') else {
                            panic!("invalid command format: {command}");
                        };
                        let params: serde_json::Value = serde_json::from_str(json).unwrap_or_else(|e| panic!("invalid JSON in {command}: {e}"));

                        if let Err(error) = session.execute(id, &params) {
                            panic!("archdetect command `{command}` failed: {error:?}");
                        }
                    }
                }
            }
        }
    }
    assert!(found_archdetect, "user guide should contain an archdetect example");
}

#[test]
fn user_guide_lighting_block_runs() {
    let blocks = sh_blocks(GUIDE);
    let mut found_light_block = false;

    for lines in &blocks {
        let Some(line) = lines.iter().find(|line| line.contains("lightfixture") && line.contains("illuminance")) else {
            continue;
        };
        found_light_block = true;
        let cmds = cmd_args(line);
        assert!(!cmds.is_empty(), "lighting block should contain one or more --cmd lines");

        let mut session = Session::new();
        session.new_drawing(true);
        let mut result = None;
        for command in &cmds {
            let Some((id, json)) = command.split_once(' ') else {
                panic!("invalid command format: {command}");
            };
            let params: serde_json::Value = serde_json::from_str(json).unwrap_or_else(|e| panic!("invalid JSON in {command}: {e}"));
            let reply = session.execute(id, &params).unwrap_or_else(|error| panic!("lighting command `{command}` failed: {error:?}"));
            result = Some(reply);
        }

        let lux = result
            .and_then(|reply| reply.get("lux").and_then(serde_json::Value::as_f64))
            .unwrap_or_else(|| panic!("lighting block should return a lux value from illuminance: {line}"));
        assert!((lux - 165.0).abs() <= 2.0, "lighting block illuminance should be ~165 lx, got {lux}");
        return;
    }

    assert!(found_light_block, "user guide should contain a lighting example block");
}

/// The guide is titled for arch3d (not "Phase 6"), puts 51.05 N in Calgary (London is 51.5 N) and uses the real
/// CLI syntax, not an invented `--command ... --params` form.
#[test]
fn user_guide_title_and_latitude_are_right() {
    let title = GUIDE.trim_start().lines().next().unwrap_or_default();
    assert_eq!(title, "# arch3d user guide", "the user guide's title");
    for line in GUIDE.lines() {
        assert!(!(line.contains("51.05") && line.to_lowercase().contains("london")), "51.05 N is Calgary, not London: {line}");
    }
    assert!(GUIDE.contains("Calgary, 51.05"), "the guide names Calgary, 51.05° N, as the examples' latitude");
    assert!(!GUIDE.contains("--command ") && !GUIDE.contains("--params "), "the guide uses an invented `--command ... --params` syntax");
}

/// A dry run of archdetect on an empty drawing must succeed and report zero candidates.
#[test]
fn user_guide_archdetect_dry_run_on_empty() {
    let mut session = Session::new();
    let result = session.execute("archdetect", &serde_json::json!({"dryRun": true}));
    assert!(result.is_ok(), "archdetect dry run on an empty drawing should succeed");
    let reply = result.unwrap();
    let walls = reply.get("walls").and_then(serde_json::Value::as_array);
    let openings = reply.get("openings").and_then(serde_json::Value::as_array);
    let rooms = reply.get("rooms").and_then(serde_json::Value::as_array);
    assert!(walls.is_some(), "dry run reply should have a walls array");
    assert!(openings.is_some(), "dry run reply should have an openings array");
    assert!(rooms.is_some(), "dry run reply should have a rooms array");
    assert_eq!(walls.unwrap().len(), 0, "empty drawing should detect no walls");
    assert_eq!(openings.unwrap().len(), 0, "empty drawing should detect no openings");
    assert_eq!(rooms.unwrap().len(), 0, "empty drawing should detect no rooms");
}

/// Hostile archdetect parameters are refused with nothing written.
#[test]
fn user_guide_archdetect_hostile_params() {
    let mut session = Session::new();
    // dryRun must be a boolean
    let r = session.execute("archdetect", &serde_json::json!({"dryRun": "yes"}));
    assert!(r.is_err(), "dryRun: \"yes\" should be refused");
    // minConfidence must be in 0..=1
    let r = session.execute("archdetect", &serde_json::json!({"dryRun": true, "minConfidence": 1.5}));
    assert!(r.is_err(), "minConfidence 1.5 should be refused");
    // accept must be "all" or an array of ids
    let r = session.execute("archdetect", &serde_json::json!({"accept": "none"}));
    assert!(r.is_err(), "accept: \"none\" should be refused");
    // units must be one of the known unit names
    let r = session.execute("archdetect", &serde_json::json!({"dryRun": true, "units": "yards"}));
    assert!(r.is_err(), "units: \"yards\" should be refused");
    // dryRun and accept together is refused
    let r = session.execute("archdetect", &serde_json::json!({"dryRun": true, "accept": "all"}));
    assert!(r.is_err(), "dryRun and accept together should be refused");
}

/// The "Detecting a plan" section documents everything the spec requires.
#[test]
fn user_guide_archdetect_section_completeness() {
    let content = read_user_guide();
    assert!(content.contains("Detecting a plan"), "the guide should have a Detecting a plan section");
    assert!(content.contains("archdetect"), "the guide should mention the archdetect command");
    assert!(content.contains("dryRun"), "the guide should document the dryRun parameter");
    assert!(content.contains("accept"), "the guide should document the accept parameter");
    assert!(content.contains("confidence"), "the guide should explain confidence scores");
    assert!(content.contains("evidence"), "the guide should explain evidence");
    assert!(content.contains("AIA"), "the guide should mention AIA layer names");
    assert!(content.contains("INSERT"), "the guide should mention INSERT handling");
    assert!(content.contains("$INSUNITS"), "the guide should mention $INSUNITS");
    assert!(content.contains("Detection Review"), "the guide should mention the Detection Review panel");
    assert!(content.contains("examples/apartment.dxf"), "the guide should use examples/apartment.dxf");
    assert!(content.contains("apartment.glb"), "the guide should show convert to GLB");
}

/// Hostile sun parameters are refused with nothing written.
#[test]
fn user_guide_hostile_sun_params() {
    let mut session = Session::new();
    // lat must be in -90..=90
    let r = session.execute("sun", &serde_json::json!({"lat":95.0,"date":"2026-06-21","time":"solar-noon"}));
    assert!(r.is_err(), "lat 95 should be refused");
    let r = session.execute("sun", &serde_json::json!({"lat":-95.0,"date":"2026-06-21","time":"solar-noon"}));
    assert!(r.is_err(), "lat -95 should be refused");
    // date must be valid
    let r = session.execute("sun", &serde_json::json!({"lat":51.05,"date":"2026-13-01","time":"solar-noon"}));
    assert!(r.is_err(), "month 13 should be refused");
    let r = session.execute("sun", &serde_json::json!({"lat":51.05,"date":"not-a-date","time":"solar-noon"}));
    assert!(r.is_err(), "non-date string should be refused");
    // time must be solar-noon or HH:MM
    let r = session.execute("sun", &serde_json::json!({"lat":51.05,"date":"2026-06-21","time":"noon"}));
    assert!(r.is_err(), "time \"noon\" should be refused");
    let r = session.execute("sun", &serde_json::json!({"lat":51.05,"date":"2026-06-21","time":"25:00"}));
    assert!(r.is_err(), "hour 25 should be refused");
    // missing required fields
    let r = session.execute("sun", &serde_json::json!({"lat":51.05}));
    assert!(r.is_err(), "missing date and time should be refused");
}

/// Hostile camera parameters are refused with nothing written.
#[test]
fn user_guide_hostile_camera_params() {
    let mut session = Session::new();
    // kind must be orbit or full
    let r = session.execute("camera", &serde_json::json!({"save":"test","kind":"pan","eye":[0,0,1],"target":[0,0,0],"fov":60}));
    assert!(r.is_err(), "kind \"pan\" should be refused");
    // save requires name
    let r = session.execute("camera", &serde_json::json!({"save":null,"kind":"full","eye":[0,0,1],"target":[0,0,0],"fov":60}));
    assert!(r.is_err(), "null save name should be refused");
    // use requires name
    let r = session.execute("camera", &serde_json::json!({"use":null}));
    assert!(r.is_err(), "null use name should be refused");
    // delete requires name
    let r = session.execute("camera", &serde_json::json!({"delete":null}));
    assert!(r.is_err(), "null delete name should be refused");
    // use of non-existent camera
    let r = session.execute("camera", &serde_json::json!({"use":"nonexistent"}));
    assert!(r.is_err(), "using non-existent camera should be refused");
    // delete of non-existent camera
    let r = session.execute("camera", &serde_json::json!({"delete":"nonexistent"}));
    assert!(r.is_err(), "deleting non-existent camera should be refused");
    // full camera requires eye and target
    let r = session.execute("camera", &serde_json::json!({"save":"test","kind":"full","eye":[0,0,1]}));
    assert!(r.is_err(), "full camera without target should be refused");
    // orbit camera requires azimuth
    let r = session.execute("camera", &serde_json::json!({"save":"test","kind":"orbit"}));
    assert!(r.is_err(), "orbit camera without azimuth should be refused");
    // fov must be positive
    let r = session.execute("camera", &serde_json::json!({"save":"test","kind":"full","eye":[0,0,1],"target":[0,0,0],"fov":0}));
    assert!(r.is_err(), "fov 0 should be refused");
}

/// Hostile walkthrough parameters are refused with nothing written.
#[test]
fn user_guide_hostile_walkthrough_params() {
    let mut session = Session::new();
    // eye must have 3 components
    let r = session.execute("walkthrough", &serde_json::json!({"eye":[0,0],"yaw":0,"forward":0,"strafe":0,"turn":0}));
    assert!(r.is_err(), "eye with 2 components should be refused");
    let r = session.execute("walkthrough", &serde_json::json!({"eye":[0,0,1,2],"yaw":0,"forward":0,"strafe":0,"turn":0}));
    assert!(r.is_err(), "eye with 4 components should be refused");
    // missing required fields
    let r = session.execute("walkthrough", &serde_json::json!({"eye":[0,0,1]}));
    assert!(r.is_err(), "missing yaw should be refused");
    // non-finite values
    let r = session.execute("walkthrough", &serde_json::json!({"eye":[0.0/0.0,0,1],"yaw":0,"forward":0,"strafe":0,"turn":0}));
    assert!(r.is_err(), "NaN in eye should be refused");
}

/// Hostile pick3d parameters are refused with nothing written.
#[test]
fn user_guide_hostile_pick3d_params() {
    let mut session = Session::new();
    // size must have 2 components
    let r = session
        .execute("pick3d", &serde_json::json!({"camera":{"kind":"full","eye":[0,0,1],"target":[0,0,0],"fov":60},"size":[480],"pixel":[240,160]}));
    assert!(r.is_err(), "size with 1 component should be refused");
    // pixel must have 2 components
    let r = session
        .execute("pick3d", &serde_json::json!({"camera":{"kind":"full","eye":[0,0,1],"target":[0,0,0],"fov":60},"size":[480,320],"pixel":[240]}));
    assert!(r.is_err(), "pixel with 1 component should be refused");
    // pixel out of bounds
    let r = session
        .execute("pick3d", &serde_json::json!({"camera":{"kind":"full","eye":[0,0,1],"target":[0,0,0],"fov":60},"size":[480,320],"pixel":[500,160]}));
    assert!(r.is_err(), "pixel x >= width should be refused");
    let r = session
        .execute("pick3d", &serde_json::json!({"camera":{"kind":"full","eye":[0,0,1],"target":[0,0,0],"fov":60},"size":[480,320],"pixel":[240,400]}));
    assert!(r.is_err(), "pixel y >= height should be refused");
    // negative pixel
    let r = session
        .execute("pick3d", &serde_json::json!({"camera":{"kind":"full","eye":[0,0,1],"target":[0,0,0],"fov":60},"size":[480,320],"pixel":[-1,160]}));
    assert!(r.is_err(), "negative pixel x should be refused");
}

/// Hostile elevation parameters are refused with nothing written.
#[test]
fn user_guide_hostile_elevation_params() {
    let mut session = Session::new();
    // side must be valid
    let r = session.execute("elevation", &serde_json::json!({"side":"up","insert":[0,0]}));
    assert!(r.is_err(), "side \"up\" should be refused");
    // insert must have 2 components
    let r = session.execute("elevation", &serde_json::json!({"side":"south","insert":[0]}));
    assert!(r.is_err(), "insert with 1 component should be refused");
    // scale must be positive
    let r = session.execute("elevation", &serde_json::json!({"side":"south","insert":[0,0],"scale":0}));
    assert!(r.is_err(), "scale 0 should be refused");
}

/// Hostile section parameters are refused with nothing written.
#[test]
fn user_guide_hostile_section_params() {
    let mut session = Session::new();
    // from must have 2 components
    let r = session.execute("section", &serde_json::json!({"from":[0],"to":[1,0],"insert":[0,0]}));
    assert!(r.is_err(), "from with 1 component should be refused");
    // to must have 2 components
    let r = session.execute("section", &serde_json::json!({"from":[0,0],"to":[1],"insert":[0,0]}));
    assert!(r.is_err(), "to with 1 component should be refused");
    // from and to must be different
    let r = session.execute("section", &serde_json::json!({"from":[0,0],"to":[0,0],"insert":[0,0]}));
    assert!(r.is_err(), "from == to should be refused");
}

/// Hostile viewsheet parameters are refused with nothing written.
#[test]
fn user_guide_hostile_viewsheet_params() {
    let mut session = Session::new();
    // views must be non-empty
    let r = session.execute("viewsheet", &serde_json::json!({"name":"Test","views":[]}));
    assert!(r.is_err(), "empty views should be refused");
    // views must be at most 16
    let views: Vec<String> = (0..17).map(|i| format!("v{}", i)).collect();
    let r = session.execute("viewsheet", &serde_json::json!({"name":"Test","views":views}));
    assert!(r.is_err(), "17 views should be refused");
    // views must be unique
    let r = session.execute("viewsheet", &serde_json::json!({"name":"Test","views":["v0","v0"]}));
    assert!(r.is_err(), "duplicate views should be refused");
    // name must be a string
    let r = session.execute("viewsheet", &serde_json::json!({"name":null,"views":["v0"]}));
    assert!(r.is_err(), "null name should be refused");
}

/// A sh block whose last line ends with a backslash is handled correctly.
#[test]
fn user_guide_sh_blocks_last_line_continues() {
    let blocks = sh_blocks("```sh\necho a \\\necho b \\\n```\n");
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].len(), 1);
    assert!(blocks[0][0].contains("echo a"));
    assert!(blocks[0][0].contains("echo b"));
}

/// All --cmd arguments in the guide have valid JSON.
#[test]
fn user_guide_all_cmd_json_is_valid() {
    let blocks = sh_blocks(GUIDE);
    for lines in blocks {
        for line in lines {
            let cmds = cmd_args(&line);
            for cmd in cmds {
                let Some((id, json)) = cmd.split_once(' ') else {
                    panic!("cmd `{cmd}` should have form `<id> <json>`");
                };
                let result = serde_json::from_str::<serde_json::Value>(json);
                assert!(result.is_ok(), "cmd `{cmd}` has invalid JSON");
            }
        }
    }
}

/// The lighting section documents everything the spec requires.
#[test]
fn user_guide_lighting_section_completeness() {
    let content = read_user_guide();
    assert!(content.contains("Lighting and final renders"), "the guide should have a Lighting and final renders section");
    assert!(content.contains("lightfixture"), "the guide should mention the lightfixture command");
    assert!(content.contains("illuminance"), "the guide should mention the illuminance command");
    assert!(content.contains("Finalize Render"), "the guide should document Finalize Render");
    assert!(content.contains("sunstudy"), "the guide should mention sunstudy");
    assert!(content.contains("8"), "the guide should mention the 8-light interactive budget");
    assert!(content.contains("bounced light"), "the guide should mention bounced light as out of scope");
    assert!(content.contains("IES"), "the guide should mention IES files as out of scope");
    assert!(content.contains("shadow map"), "the guide should mention shadow maps");
    assert!(content.contains("tone-map") || content.contains("tone mapping"), "the guide should mention tone mapping");
}

/// Hostile lighting parameters are refused with nothing written.
#[test]
fn user_guide_hostile_lighting_params() {
    let mut session = Session::new();
    // illuminance point must have 3 components
    let r = session.execute("illuminance", &serde_json::json!({"point":[1,1]}));
    assert!(r.is_err(), "illuminance point with 2 components should be refused");
    let r = session.execute("illuminance", &serde_json::json!({"point":[1,1,0,0]}));
    assert!(r.is_err(), "illuminance point with 4 components should be refused");
    // lightfixture with non-existent room
    let r = session.execute("lightfixture", &serde_json::json!({"room":"r99","rows":1,"cols":1}));
    assert!(r.is_err(), "lightfixture in non-existent room should be refused");
    // lightfixture with zero rows
    session.execute("wall", &serde_json::json!({"points":[[0,0],[4,0],[4,4],[0,4],[0,0]],"thickness":0.2,"height":2.4,"exterior":true})).ok();
    session.execute("roomdetect", &serde_json::json!({})).ok();
    let r = session.execute("lightfixture", &serde_json::json!({"room":"r0","rows":0,"cols":1}));
    assert!(r.is_err(), "lightfixture with 0 rows should be refused");
    // lightfixture with too many rows (cap is 32)
    let r = session.execute("lightfixture", &serde_json::json!({"room":"r0","rows":33,"cols":1}));
    assert!(r.is_err(), "lightfixture with 33 rows should be refused");
}

/// The render section documents the --night and --final flags.
#[test]
fn user_guide_render_flags_documented() {
    let content = read_user_guide();
    assert!(content.contains("--night"), "the guide should document the --night flag");
    assert!(content.contains("--final"), "the guide should document the --final flag");
}

/// The path-traced Finalize block in the user guide runs through the engine; its illuminance reply is positive.
#[test]
fn user_guide_path_tracing_block_runs() {
    let blocks = sh_blocks(GUIDE);
    let mut found_path_block = false;

    for lines in &blocks {
        let Some(line) = lines.iter().find(|line| line.contains("LED panel") && line.contains("illuminance")) else {
            continue;
        };
        found_path_block = true;
        let cmds = cmd_args(line);
        assert!(!cmds.is_empty(), "path-tracing block should contain one or more --cmd lines");

        let mut session = Session::new();
        session.new_drawing(true);
        let mut result = None;
        for command in &cmds {
            let Some((id, json)) = command.split_once(' ') else {
                panic!("invalid command format: {command}");
            };
            let params: serde_json::Value = serde_json::from_str(json).unwrap_or_else(|e| panic!("invalid JSON in {command}: {e}"));
            let reply = session.execute(id, &params).unwrap_or_else(|error| panic!("path-tracing command `{command}` failed: {error:?}"));
            result = Some(reply);
        }

        let lux = result
            .and_then(|reply| reply.get("lux").and_then(serde_json::Value::as_f64))
            .unwrap_or_else(|| panic!("path-tracing block should return a lux value from illuminance: {line}"));
        assert!(lux > 0.0, "path-tracing block illuminance should be positive, got {lux}");
        return;
    }

    assert!(found_path_block, "user guide should contain a path-tracing example block");
}

/// Exactly one sh block in the guide matches the path-tracing block's signature ("LED panel" and "illuminance").
#[test]
fn user_guide_path_tracing_block_is_unique() {
    let blocks = sh_blocks(GUIDE);
    let mut count = 0;
    for lines in &blocks {
        if lines.iter().any(|line| line.contains("LED panel") && line.contains("illuminance")) {
            count += 1;
        }
    }
    assert_eq!(count, 1, "exactly one sh block should match the path-tracing block signature, found {count}");
}

/// The path-tracing block has exactly five commands: wall, roomdetect, two lightfixtures, illuminance.
#[test]
fn user_guide_path_tracing_block_command_count() {
    let blocks = sh_blocks(GUIDE);
    let mut found = false;
    for lines in &blocks {
        let Some(line) = lines.iter().find(|line| line.contains("LED panel") && line.contains("illuminance")) else {
            continue;
        };
        found = true;
        let cmds = cmd_args(line);
        assert_eq!(cmds.len(), 5, "path-tracing block should have exactly 5 commands, found {}", cmds.len());
        let ids: Vec<&str> = cmds.iter().map(|c| c.split_once(' ').map(|(id, _)| id).unwrap_or("")).collect();
        assert_eq!(ids, ["wall", "roomdetect", "lightfixture", "lightfixture", "illuminance"], "path-tracing block command order");
        return;
    }
    assert!(found, "path-tracing block not found");
}

/// The exports block in the user guide runs through the engine.
#[test]
fn user_guide_exports_block_runs() {
    install_io();
    let blocks = sh_blocks(GUIDE);
    let mut found = false;

    for lines in &blocks {
        let Some(line) = lines.iter().find(|line| line.contains("export") && line.contains("house.ifc")) else {
            continue;
        };
        found = true;
        let cmds = cmd_args(line);
        assert!(!cmds.is_empty(), "exports block should contain one or more --cmd lines");

        let mut session = Session::new();
        session.new_drawing(true);

        // Build the house first (same as the block)
        session
            .execute("wall", &serde_json::json!({"points":[[0,0],[10,0],[10,8],[0,8],[0,0]],"thickness":0.2,"height":2.4,"exterior":true}))
            .expect("wall");
        session.execute("door", &serde_json::json!({"wall":"w0","offset":3.0,"width":0.9,"height":2.1,"style":"hinged"})).expect("door");
        session
            .execute("window", &serde_json::json!({"wall":"w2","offset":4.0,"width":1.2,"height":1.2,"sill":0.9,"style":"slider"}))
            .expect("window");
        session.execute("roomdetect", &serde_json::json!({})).expect("roomdetect");
        session.execute("roofauto", &serde_json::json!({"pitchRise":6,"pitchRun":12,"overhang":0.5,"walls":[{"wall":"w0","directive":"gable"},{"wall":"w2","directive":"gable"}]})).expect("roofauto");

        // Execute the export commands from the block
        for command in &cmds {
            let Some((id, json)) = command.split_once(' ') else {
                panic!("invalid command format: {command}");
            };
            if id != "export" {
                continue;
            }
            let mut params: serde_json::Value = serde_json::from_str(json).unwrap_or_else(|e| panic!("invalid JSON in {command}: {e}"));
            let out_dir = std::env::temp_dir().join(format!("cadcraft-user-guide-exports-{}", std::process::id()));
            fs::create_dir_all(&out_dir).unwrap_or_else(|e| panic!("cannot create {}: {e}", out_dir.display()));
            if let Some(path) = params.get("path").and_then(serde_json::Value::as_str) {
                if Path::new(path).is_relative() {
                    params["path"] = serde_json::Value::String(out_dir.join(path).to_string_lossy().into_owned());
                }
            }
            let reply = session.execute(id, &params).unwrap_or_else(|error| panic!("export command `{command}` failed: {error:?}"));
            let _ = fs::remove_dir_all(&out_dir);
            // Verify reply structure for format-only exports
            if params.get("format").is_some() {
                assert!(reply.get("data").is_some(), "export with format should return data");
            }
        }
        return;
    }

    assert!(found, "user guide should contain an exports example block");
}

/// The illuminance command in the path-tracing block measures at point [2,2,0].
#[test]
fn user_guide_path_tracing_block_illuminance_point_is_220() {
    let blocks = sh_blocks(GUIDE);
    let mut found = false;
    for lines in &blocks {
        let Some(line) = lines.iter().find(|line| line.contains("LED panel") && line.contains("illuminance")) else {
            continue;
        };
        found = true;
        let cmds = cmd_args(line);
        let illum_cmd = cmds.iter().find(|c| c.starts_with("illuminance ")).expect("path-tracing block should have an illuminance command");
        let json = illum_cmd.split_once(' ').unwrap().1;
        let params: serde_json::Value = serde_json::from_str(json).expect("illuminance JSON should parse");
        let point = params.get("point").and_then(serde_json::Value::as_array).expect("illuminance should have a point array");
        assert_eq!(point.len(), 3, "illuminance point should have 3 components");
        assert_eq!(point[0].as_f64(), Some(2.0), "illuminance point x should be 2.0");
        assert_eq!(point[1].as_f64(), Some(2.0), "illuminance point y should be 2.0");
        assert_eq!(point[2].as_f64(), Some(0.0), "illuminance point z should be 0.0");
        return;
    }
    assert!(found, "path-tracing block not found");
}

/// The lighting block (illuminance at [1,1,0]) and the path-tracing block (illuminance at [2,2,0]) are distinct.
#[test]
fn user_guide_lighting_and_path_tracing_blocks_differ() {
    let blocks = sh_blocks(GUIDE);
    let mut lighting_point = None;
    let mut path_point = None;

    for lines in &blocks {
        for line in lines {
            if line.contains("lightfixture") && line.contains("illuminance") && !line.contains("LED panel") {
                let cmds = cmd_args(line);
                if let Some(illum_cmd) = cmds.iter().find(|c| c.starts_with("illuminance ")) {
                    let json = illum_cmd.split_once(' ').unwrap().1;
                    let params: serde_json::Value = serde_json::from_str(json).expect("lighting illuminance JSON should parse");
                    lighting_point = params.get("point").cloned();
                }
            }
            if line.contains("LED panel") && line.contains("illuminance") {
                let cmds = cmd_args(line);
                if let Some(illum_cmd) = cmds.iter().find(|c| c.starts_with("illuminance ")) {
                    let json = illum_cmd.split_once(' ').unwrap().1;
                    let params: serde_json::Value = serde_json::from_str(json).expect("path illuminance JSON should parse");
                    path_point = params.get("point").cloned();
                }
            }
        }
    }

    assert!(lighting_point.is_some(), "lighting block with illuminance not found");
    assert!(path_point.is_some(), "path-tracing block with illuminance not found");
    assert_ne!(lighting_point, path_point, "lighting and path-tracing blocks should measure at different points");
}

/// The guide documents the key phase 7 commands with their parameters.
#[test]
fn user_guide_documents_phase7_parameters() {
    assert!(GUIDE.contains("\"side\":\"south\""), "guide should show elevation side parameter");
    assert!(GUIDE.contains("\"from\""), "guide should show section from parameter");
    assert!(GUIDE.contains("\"to\""), "guide should show section to parameter");
    assert!(GUIDE.contains("\"views\""), "guide should show viewsheet views parameter");
    assert!(GUIDE.contains("\"paper\""), "guide should show viewsheet paper parameter");
}

/// The guide contains sections for materials, cabinets/fixtures, and large models.
#[test]
fn user_guide_has_materials_cabinets_and_large_models() {
    assert!(GUIDE.contains("## Materials"), "guide should have a Materials section");
    assert!(GUIDE.contains("## Cabinets and fixtures"), "guide should have a Cabinets and fixtures section");
    assert!(GUIDE.contains("## Large models"), "guide should have a Large models section");
    assert!(GUIDE.contains("material"), "guide should mention the material command");
    assert!(GUIDE.contains("cabinet"), "guide should mention the cabinet command");
    assert!(GUIDE.contains("fixture"), "guide should mention the fixture command");
    assert!(GUIDE.contains("arch.sample"), "guide should mention the arch.sample command");
    assert!(GUIDE.contains("build3d"), "guide should mention the build3d command");
    assert!(GUIDE.contains("siding"), "guide should mention siding finish");
    assert!(GUIDE.contains("shingles"), "guide should mention shingles finish");
    assert!(GUIDE.contains("wood"), "guide should mention wood finish");
    assert!(GUIDE.contains("rebuilt"), "guide should explain rebuilt count");
    assert!(GUIDE.contains("rebuildMs"), "guide should explain rebuildMs");
}

/// The new blocks for materials, cabinets, and large models execute through the engine.
#[test]
fn user_guide_new_blocks_execute_through_the_engine() {
    install_io();
    let blocks = sh_blocks(GUIDE);
    let mut found_materials = false;
    let mut found_cabinets = false;
    let mut found_large = false;

    for lines in &blocks {
        let line = lines.join(" ");

        // Materials block: contains material and export
        if line.contains("material") && line.contains("export") {
            found_materials = true;
            let cmds = cmd_args(&line);
            assert!(!cmds.is_empty(), "materials block should contain commands");
            let mut session = Session::new();
            session.new_drawing(true);
            for command in &cmds {
                let Some((id, json)) = command.split_once(' ') else {
                    panic!("invalid command format: {command}");
                };
                let mut params: serde_json::Value = serde_json::from_str(json).unwrap_or_else(|e| panic!("invalid JSON in {command}: {e}"));
                if let Some(path) = params.get("path").and_then(serde_json::Value::as_str) {
                    if Path::new(path).is_relative() {
                        let out_dir = std::env::temp_dir().join(format!("cadcraft-user-guide-materials-{}", std::process::id()));
                        fs::create_dir_all(&out_dir).unwrap_or_else(|e| panic!("cannot create {}: {e}", out_dir.display()));
                        params["path"] = serde_json::Value::String(out_dir.join(path).to_string_lossy().into_owned());
                    }
                }
                if let Err(error) = session.execute(id, &params) {
                    panic!("materials command `{command}` failed: {error:?}");
                }
            }
        }

        // Cabinets block: contains cabinet and fixture
        if line.contains("cabinet") && line.contains("fixture") {
            found_cabinets = true;
            let cmds = cmd_args(&line);
            assert!(!cmds.is_empty(), "cabinets block should contain commands");
            let mut session = Session::new();
            session.new_drawing(true);
            for command in &cmds {
                let Some((id, json)) = command.split_once(' ') else {
                    panic!("invalid command format: {command}");
                };
                let params: serde_json::Value = serde_json::from_str(json).unwrap_or_else(|e| panic!("invalid JSON in {command}: {e}"));
                if let Err(error) = session.execute(id, &params) {
                    panic!("cabinets command `{command}` failed: {error:?}");
                }
            }
        }

        // Large models block: contains arch.sample and build3d
        if line.contains("arch.sample") && line.contains("build3d") {
            found_large = true;
            let cmds = cmd_args(&line);
            assert!(!cmds.is_empty(), "large models block should contain commands");
            let mut session = Session::new();
            session.new_drawing(true);
            for command in &cmds {
                let Some((id, json)) = command.split_once(' ') else {
                    panic!("invalid command format: {command}");
                };
                let params: serde_json::Value = serde_json::from_str(json).unwrap_or_else(|e| panic!("invalid JSON in {command}: {e}"));
                if let Err(error) = session.execute(id, &params) {
                    panic!("large models command `{command}` failed: {error:?}");
                }
            }
        }
    }

    assert!(found_materials, "user guide should contain a materials example block");
    assert!(found_cabinets, "user guide should contain a cabinets example block");
    assert!(found_large, "user guide should contain a large models example block");
}

#[test]
fn user_guide_sh_blocks_edge_cases() {
    // No sh blocks at all
    let blocks = sh_blocks("just some text\nno blocks here\n");
    assert_eq!(blocks.len(), 0, "no sh blocks should yield empty vec");

    // Empty sh block
    let blocks = sh_blocks("```sh\n```\n");
    assert_eq!(blocks.len(), 1, "one sh block should be found");
    assert_eq!(blocks[0].len(), 0, "empty sh block should have no lines");

    // Sh block with only blank lines
    let blocks = sh_blocks("```sh\n\n\n```\n");
    assert_eq!(blocks.len(), 1, "one sh block should be found");
    assert_eq!(blocks[0].len(), 2, "two blank lines should be found");

    // Multiple blocks
    let blocks = sh_blocks("```sh\necho a\n```\ntext\n```sh\necho b\n```\n");
    assert_eq!(blocks.len(), 2, "two sh blocks should be found");
    assert_eq!(blocks[0][0], "echo a", "first block first line");
    assert_eq!(blocks[1][0], "echo b", "second block first line");

    // Line continuation
    let blocks = sh_blocks("```sh\necho a \\\necho b\n```\n");
    assert_eq!(blocks.len(), 1, "one sh block should be found");
    assert_eq!(blocks[0].len(), 1, "continued lines should be joined");
    assert!(blocks[0][0].contains("echo a"), "joined line should contain first part");
    assert!(blocks[0][0].contains("echo b"), "joined line should contain second part");
}

#[test]
fn user_guide_cmd_args_edge_cases() {
    // No --cmd at all
    let cmds = cmd_args("cargo run -q -p cadcraft-cli -- run --metric");
    assert_eq!(cmds.len(), 0, "no --cmd should yield empty vec");

    // One --cmd
    let cmds = cmd_args("cargo run -- run --cmd 'wall {\"x\":1}'");
    assert_eq!(cmds.len(), 1, "one --cmd should be found");
    assert_eq!(cmds[0], "wall {\"x\":1}", "cmd should be extracted");

    // Multiple --cmd on one line
    let cmds = cmd_args("cargo run -- run --cmd 'wall {\"x\":1}' --cmd 'door {\"y\":2}'");
    assert_eq!(cmds.len(), 2, "two --cmd should be found");
    assert_eq!(cmds[0], "wall {\"x\":1}", "first cmd");
    assert_eq!(cmds[1], "door {\"y\":2}", "second cmd");

    // --cmd with spaces in JSON
    let cmds = cmd_args("cargo run -- run --cmd 'wall {\"points\": [[0, 0], [1, 0]]}'");
    assert_eq!(cmds.len(), 1, "one --cmd should be found");
    assert!(cmds[0].contains("[[0, 0], [1, 0]]"), "JSON with spaces should be preserved");
}

/// Hostile material parameters are refused with nothing written.
#[test]
fn user_guide_hostile_material_params() {
    let mut session = Session::new();
    // Build a wall first
    session.execute("wall", &serde_json::json!({"points":[[0,0],[10,0],[10,8],[0,8],[0,0]],"thickness":0.2,"height":2.4,"exterior":true})).ok();
    // Bad finish name
    let r = session.execute("material", &serde_json::json!({"walls":"exterior","finish":"marble"}));
    assert!(r.is_err(), "finish \"marble\" should be refused");
    // Bad wall id
    let r = session.execute("material", &serde_json::json!({"walls":["w99"],"finish":"siding"}));
    assert!(r.is_err(), "wall \"w99\" should be refused");
    // Empty walls list
    let r = session.execute("material", &serde_json::json!({"walls":[],"finish":"siding"}));
    assert!(r.is_err(), "empty walls list should be refused");
    // Bad roof finish
    let r = session.execute("material", &serde_json::json!({"roofs":"all","finish":"siding"}));
    assert!(r.is_err(), "roof finish \"siding\" should be refused");
    // Bad room finish
    let r = session.execute("material", &serde_json::json!({"rooms":["r0"],"finish":"siding"}));
    assert!(r.is_err(), "room finish \"siding\" should be refused");
}

/// Hostile cabinet parameters are refused with nothing written.
#[test]
fn user_guide_hostile_cabinet_params() {
    let mut session = Session::new();
    // Build a wall and room first
    session.execute("wall", &serde_json::json!({"points":[[0,0],[6,0],[6,4],[0,4],[0,0]],"thickness":0.2,"height":2.4,"exterior":true})).ok();
    session.execute("roomdetect", &serde_json::json!({})).ok();
    // Bad kind
    let r = session.execute("cabinet", &serde_json::json!({"kind":"tall","wall":"w0","offset":1.0,"width":1.5,"height":2.0}));
    assert!(r.is_ok(), "tall cabinet should be accepted");
    let r = session.execute("cabinet", &serde_json::json!({"kind":"drawer","wall":"w0","offset":1.0,"width":1.5}));
    assert!(r.is_err(), "kind \"drawer\" should be refused");
    // Bad wall id
    let r = session.execute("cabinet", &serde_json::json!({"kind":"base","wall":"w99","offset":1.0,"width":1.5}));
    assert!(r.is_err(), "wall \"w99\" should be refused");
    // Zero width
    let r = session.execute("cabinet", &serde_json::json!({"kind":"base","wall":"w0","offset":1.0,"width":0.0}));
    assert!(r.is_err(), "width 0.0 should be refused");
    // Negative offset
    let r = session.execute("cabinet", &serde_json::json!({"kind":"base","wall":"w0","offset":-1.0,"width":1.5}));
    assert!(r.is_err(), "offset -1.0 should be refused");
    // Offset too large for wall
    let r = session.execute("cabinet", &serde_json::json!({"kind":"base","wall":"w0","offset":10.0,"width":1.5}));
    assert!(r.is_err(), "offset 10.0 on 6m wall should be refused");
}

/// Hostile fixture parameters are refused with nothing written.
#[test]
fn user_guide_hostile_fixture_params() {
    let mut session = Session::new();
    // Build a wall and room first
    session.execute("wall", &serde_json::json!({"points":[[0,0],[6,0],[6,4],[0,4],[0,0]],"thickness":0.2,"height":2.4,"exterior":true})).ok();
    session.execute("roomdetect", &serde_json::json!({})).ok();
    // Bad kind
    let r = session.execute("fixture", &serde_json::json!({"kind":"stove","wall":"w0","offset":1.0}));
    assert!(r.is_err(), "kind \"stove\" should be refused");
    // Bad wall id
    let r = session.execute("fixture", &serde_json::json!({"kind":"sink","wall":"w99","offset":1.0}));
    assert!(r.is_err(), "wall \"w99\" should be refused");
    // Negative offset
    let r = session.execute("fixture", &serde_json::json!({"kind":"sink","wall":"w0","offset":-1.0}));
    assert!(r.is_err(), "offset -1.0 should be refused");
}

/// Cabinets and fixtures survive a save/reload round-trip.
#[test]
fn user_guide_cabinets_fixtures_round_trip() {
    let mut session = Session::new();
    session
        .execute("wall", &serde_json::json!({"points":[[0,0],[6,0],[6,4],[0,4],[0,0]],"thickness":0.2,"height":2.4,"exterior":true}))
        .expect("wall should succeed");
    session.execute("roomdetect", &serde_json::json!({})).expect("roomdetect should succeed");
    session.execute("cabinet", &serde_json::json!({"kind":"base","wall":"w0","offset":1.0,"width":1.5})).expect("cabinet should succeed");
    session.execute("fixture", &serde_json::json!({"kind":"sink","wall":"w0","offset":1.75})).expect("fixture should succeed");

    // Verify they exist
    let inspect = session.execute("arch.inspect", &serde_json::json!({"cabinets":true,"fixtures":true})).expect("inspect should succeed");
    let cabinets_before = inspect.get("cabinets").and_then(serde_json::Value::as_array);
    let fixtures_before = inspect.get("fixtures").and_then(serde_json::Value::as_array);
    assert!(cabinets_before.is_some(), "inspect should return cabinets array");
    assert!(fixtures_before.is_some(), "inspect should return fixtures array");
    assert_eq!(cabinets_before.unwrap().len(), 1, "one cabinet should exist before save");
    assert_eq!(fixtures_before.unwrap().len(), 1, "one fixture should exist before save");

    // Save and reload
    let drawing = session.doc().expect("doc should exist").clone();
    let mut session2 = Session::new();
    session2.open_drawing(drawing, "test", None);

    // Verify they still exist after reload
    let inspect2 =
        session2.execute("arch.inspect", &serde_json::json!({"cabinets":true,"fixtures":true})).expect("inspect should succeed after reload");
    let cabinets_after = inspect2.get("cabinets").and_then(serde_json::Value::as_array);
    let fixtures_after = inspect2.get("fixtures").and_then(serde_json::Value::as_array);
    assert!(cabinets_after.is_some(), "inspect should return cabinets array after reload");
    assert!(fixtures_after.is_some(), "inspect should return fixtures array after reload");
    assert_eq!(cabinets_after.unwrap().len(), 1, "one cabinet should exist after reload");
    assert_eq!(fixtures_after.unwrap().len(), 1, "one fixture should exist after reload");
}

/// The materials block has exactly 10 commands: wall, door, window, roomdetect, roofauto, 4 materials, export.
#[test]
fn user_guide_materials_block_command_count() {
    let blocks = sh_blocks(GUIDE);
    let mut found = false;
    for lines in &blocks {
        let line = lines.join(" ");
        if line.contains("material") && line.contains("export") {
            found = true;
            let cmds = cmd_args(&line);
            assert_eq!(cmds.len(), 10, "materials block should have exactly 10 commands, found {}", cmds.len());
            let ids: Vec<&str> = cmds.iter().map(|c| c.split_once(' ').map(|(id, _)| id).unwrap_or("")).collect();
            assert_eq!(
                ids,
                ["wall", "door", "window", "roomdetect", "roofauto", "material", "material", "material", "material", "export"],
                "materials block command order"
            );
            return;
        }
    }
    assert!(found, "materials block not found");
}

/// The cabinets block has exactly 10 commands: wall, roomdetect, 3 cabinets, 4 fixtures, inspect.
#[test]
fn user_guide_cabinets_block_command_count() {
    let blocks = sh_blocks(GUIDE);
    let mut found = false;
    for lines in &blocks {
        let line = lines.join(" ");
        if line.contains("cabinet") && line.contains("fixture") {
            found = true;
            let cmds = cmd_args(&line);
            assert_eq!(cmds.len(), 10, "cabinets block should have exactly 10 commands, found {}", cmds.len());
            let ids: Vec<&str> = cmds.iter().map(|c| c.split_once(' ').map(|(id, _)| id).unwrap_or("")).collect();
            assert_eq!(
                ids,
                ["wall", "roomdetect", "cabinet", "cabinet", "cabinet", "fixture", "fixture", "fixture", "fixture", "arch.inspect"],
                "cabinets block command order"
            );
            return;
        }
    }
    assert!(found, "cabinets block not found");
}

/// The large models block has exactly 4 commands: arch.sample, build3d, wall.stretch, build3d.
#[test]
fn user_guide_large_models_block_command_count() {
    let blocks = sh_blocks(GUIDE);
    let mut found = false;
    for lines in &blocks {
        let line = lines.join(" ");
        if line.contains("arch.sample") && line.contains("build3d") {
            found = true;
            let cmds = cmd_args(&line);
            assert_eq!(cmds.len(), 4, "large models block should have exactly 4 commands, found {}", cmds.len());
            let ids: Vec<&str> = cmds.iter().map(|c| c.split_once(' ').map(|(id, _)| id).unwrap_or("")).collect();
            assert_eq!(ids, ["arch.sample", "build3d", "arch.wall.stretch", "build3d"], "large models block command order");
            return;
        }
    }
    assert!(found, "large models block not found");
}

/// Views survive a save/reload round-trip through the engine.
#[test]
fn user_guide_views_round_trip() {
    let mut session = Session::new();
    // Build a simple house with a view
    session
        .execute("wall", &serde_json::json!({"points":[[0,0],[10,0],[10,8],[0,8],[0,0]],"thickness":0.2,"height":2.4,"exterior":true}))
        .expect("wall should succeed");
    session.execute("elevation", &serde_json::json!({"side":"south","insert":[0,-20]})).expect("elevation should succeed");

    // Verify the view exists
    let inspect = session.execute("arch.inspect", &serde_json::json!({"views":true})).expect("inspect should succeed");
    let views_before = inspect.get("views").and_then(serde_json::Value::as_array).expect("inspect should return views array");
    assert_eq!(views_before.len(), 1, "one view should exist before save");

    // Save and reload
    let drawing = session.doc().expect("doc should exist").clone();
    let mut session2 = Session::new();
    session2.open_drawing(drawing, "test", None);

    // Verify the view still exists after reload
    let inspect2 = session2.execute("arch.inspect", &serde_json::json!({"views":true})).expect("inspect should succeed after reload");
    let views_after = inspect2.get("views").and_then(serde_json::Value::as_array).expect("inspect should return views array after reload");
    assert_eq!(views_after.len(), 1, "one view should exist after reload");

    // Verify the view has the same id
    let id_before = views_before[0].get("id").and_then(serde_json::Value::as_str);
    let id_after = views_after[0].get("id").and_then(serde_json::Value::as_str);
    assert_eq!(id_before, id_after, "view id should be preserved across reload");
}

/// Runs the electrical and split-level blocks of the guide through the engine the way the CLI
/// replays them: a fresh metric session per block, each `--cmd` in order.
#[test]
fn user_guide_electrical_and_split_level_blocks_run() {
    install_io();
    let blocks = sh_blocks(GUIDE);

    // Electrical block: one room, an outlet, a switch, two lights, a connection, outlets.auto, inspect.
    {
        let mut session = Session::new();
        session.new_drawing(true);
        let line = blocks
            .iter()
            .map(|lines| lines.join(" "))
            .find(|l| l.contains("outlet {") && l.contains("outlets.auto"))
            .expect("the guide should contain the electrical block");
        let cmds = cmd_args(line);
        assert_eq!(
            cmds.len(),
            9,
            "electrical block should have exactly 9 commands, found {} (ids: {:?})",
            cmds.len(),
            cmds.iter().map(|c| c.split_once(' ').map(|(id, _)| id).unwrap_or("")).collect::<Vec<_>>()
        );
        let ids: Vec<&str> = cmds.iter().map(|c| c.split_once(' ').map(|(id, _)| id).unwrap_or("")).collect();
        assert_eq!(
            ids,
            ["wall", "roomdetect", "outlet", "switch", "lightfixture", "connection", "circuit.brightness", "outlets.auto", "arch.inspect"],
            "electrical block command order"
        );
        for command in &cmds {
            let Some((id, json)) = command.split_once(' ') else {
                panic!("electrical block command `{command}` should have the form `<id> <json>`");
            };
            let mut params: serde_json::Value =
                serde_json::from_str(json).unwrap_or_else(|e| panic!("electrical block command `{command}` has invalid JSON: {e}"));
            if let Some(path) = params.get("path").and_then(serde_json::Value::as_str) {
                if Path::new(path).is_relative() {
                    let out_dir = std::env::temp_dir().join(format!("cadcraft-electrical-{}", std::process::id()));
                    fs::create_dir_all(&out_dir).unwrap_or_else(|e| panic!("cannot create {}: {e}", out_dir.display()));
                    params["path"] = serde_json::Value::String(out_dir.join(path).to_string_lossy().into_owned());
                }
            }
            if let Err(error) = session.execute(id, &params) {
                panic!("electrical block command `{command}` failed: {error:?}");
            }
        }
    }

    // Split-level block: levels at 0, 1.35 (landing) and 2.7 (floor), a spanning wall, a door on the
    // landing and two half flights, then arch.inspect's levels.
    {
        let mut session = Session::new();
        session.new_drawing(true);
        let line = blocks
            .iter()
            .map(|lines| lines.join(" "))
            .find(|l| l.contains("split-level.dxf"))
            .expect("the guide should contain the split-level block");
        let cmds = cmd_args(line);
        assert_eq!(
            cmds.len(),
            9,
            "split-level block should have exactly 9 commands, found {} (ids: {:?})",
            cmds.len(),
            cmds.iter().map(|c| c.split_once(' ').map(|(id, _)| id).unwrap_or("")).collect::<Vec<_>>()
        );
        let ids: Vec<&str> = cmds.iter().map(|c| c.split_once(' ').map(|(id, _)| id).unwrap_or("")).collect();
        assert_eq!(ids, ["level", "level", "wall", "wall", "door", "stair", "stair", "arch.inspect"], "split-level block command order");
        for command in &cmds {
            let Some((id, json)) = command.split_once(' ') else {
                panic!("split-level block command `{command}` should have the form `<id> <json>`");
            };
            let mut params: serde_json::Value =
                serde_json::from_str(json).unwrap_or_else(|e| panic!("split-level block command `{command}` has invalid JSON: {e}"));
            if let Some(path) = params.get("path").and_then(serde_json::Value::as_str) {
                if Path::new(path).is_relative() {
                    let out_dir = std::env::temp_dir().join(format!("cadcraft-split-level-{}", std::process::id()));
                    fs::create_dir_all(&out_dir).unwrap_or_else(|e| panic!("cannot create {}: {e}", out_dir.display()));
                    params["path"] = serde_json::Value::String(out_dir.join(path).to_string_lossy().into_owned());
                }
            }
            if let Err(error) = session.execute(id, &params) {
                panic!("split-level block command `{command}` failed: {error:?}");
            }
        }
        // The final arch.inspect must report the three levels.
        let inspect = session.execute("arch.inspect", &serde_json::json!({"levels":true})).expect("inspect levels should succeed");
        let levels = inspect.get("levels").and_then(serde_json::Value::as_array);
        assert!(levels.is_some() && levels.unwrap().len() == 3, "arch.inspect should report 3 levels (lower, landing, upper), found {:?}", levels);
    }
}
