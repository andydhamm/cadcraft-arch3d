use std::fs;

fn guide() -> String {
    fs::read_to_string("docs/arch3d/USER-GUIDE.md").expect("USER-GUIDE.md must exist")
}

fn jsonl() -> String {
    fs::read_to_string("docs/arch3d/mcp-examples/11-photoreal.jsonl")
        .expect("11-photoreal.jsonl must exist")
}

fn readme() -> String {
    fs::read_to_string("docs/arch3d/mcp-examples/README.md").expect("README.md must exist")
}

#[test]
fn user_guide_has_photoreal_stills_section() {
    let g = guide();
    assert!(
        g.contains("## Photoreal stills"),
        "USER-GUIDE.md must have a '## Photoreal stills' section"
    );
}

#[test]
fn photoreal_section_before_out_of_scope() {
    let g = guide();
    let photoreal = g
        .find("## Photoreal stills")
        .expect("Photoreal stills section missing");
    let out_of_scope = g
        .find("## Out of scope")
        .expect("Out of scope section missing");
    assert!(
        photoreal < out_of_scope,
        "Photoreal stills must come before Out of scope"
    );
}

#[test]
fn photoreal_section_has_eight_rungs() {
    let g = guide();
    for i in 0..8 {
        assert!(
            g.contains(&format!("| {} |", i)),
            "Rungs table must include rung {}",
            i
        );
    }
}

#[test]
fn photoreal_section_has_lens_keys() {
    let g = guide();
    for key in ["projection", "compression", "shift", "aperture", "focus"] {
        assert!(g.contains(key), "Lens keys must include: {}", key);
    }
}

#[test]
fn photoreal_section_has_film_keys() {
    let g = guide();
    for key in ["bloom", "vignette", "contrast", "saturation", "whiteBalance"] {
        assert!(g.contains(key), "Film keys must include: {}", key);
    }
}

#[test]
fn photoreal_section_has_render_only_keys() {
    let g = guide();
    for key in ["edges", "materials", "variation", "clamp", "adaptive", "denoise", "pfm"] {
        assert!(g.contains(key), "Render-only keys must include: {}", key);
    }
}

#[test]
fn photoreal_section_has_reply_keys() {
    let g = guide();
    for key in ["stop", "noise", "sppMean", "clampLoss", "denoiser", "denoiseFallback"] {
        assert!(g.contains(key), "Reply keys must include: {}", key);
    }
}

#[test]
fn photoreal_section_has_sh_block_with_ladder() {
    let g = guide();
    assert!(g.contains("```sh"), "Must have a sh code block");
    assert!(
        g.contains("render.ladder"),
        "sh block must use render.ladder"
    );
    assert!(
        g.contains("[48,32]"),
        "sh block must use size [48, 32]"
    );
    assert!(g.contains("spp: 2") || g.contains("\"spp\":2") || g.contains("\"spp\": 2"), "sh block must use spp 2");
    assert!(g.contains("timeBudget: 5") || g.contains("\"timeBudget\":5") || g.contains("\"timeBudget\": 5"), "sh block must use timeBudget 5");
    assert!(
        g.contains("target/guide/ladder"),
        "sh block must write under target/guide/ladder"
    );
}

#[test]
fn photoreal_mentions_learned_denoiser_native_only() {
    let g = guide();
    assert!(
        g.contains("native") && g.contains("denoiser"),
        "Must mention the learned denoiser is native only"
    );
    assert!(
        g.contains("fallback") || g.contains("falls back"),
        "Must mention fallback to the filter"
    );
}

#[test]
fn photoreal_mentions_raster_views_take_projection_and_shift() {
    let g = guide();
    let section = g
        .split("## Photoreal stills")
        .nth(1)
        .and_then(|s| s.split("## ").nth(1))
        .unwrap_or("");
    assert!(
        section.contains("projection") && section.contains("shift"),
        "Must mention raster views take projection and shift"
    );
}

#[test]
fn mcp_example_is_valid_jsonl() {
    let content = jsonl();
    for (i, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        assert!(
            line.starts_with('{') && line.ends_with('}'),
            "Line {} must be a JSON object: {}",
            i + 1,
            line
        );
    }
}

#[test]
fn mcp_example_has_required_commands() {
    let content = jsonl();
    assert!(content.contains("\"wall\""), "Must have a wall command");
    assert!(content.contains("\"camera\""), "Must have a camera command");
    assert!(
        content.contains("photoreal"),
        "Camera must use photoreal technique"
    );
    assert!(content.contains("\"look\""), "Must have a look command");
    assert!(content.contains("denoiser.info"), "Must have denoiser.info");
    assert!(content.contains("\"render\""), "Must have a render command");
}

#[test]
fn mcp_example_camera_has_panini_lens() {
    let content = jsonl();
    assert!(
        content.contains("panini"),
        "Camera must use a Panini lens"
    );
}

#[test]
fn mcp_example_render_is_tiny() {
    let content = jsonl();
    assert!(
        content.contains("[48,32]") || content.contains("[48, 32]"),
        "Render must use size [48, 32]"
    );
    assert!(
        content.contains("target/"),
        "Render output must be under target/"
    );
}

#[test]
fn readme_lists_photoreal_example() {
    let r = readme();
    assert!(
        r.contains("11-photoreal.jsonl"),
        "README must list 11-photoreal.jsonl"
    );
}
