//! `sunstudy`: a series of stills from the current camera with the site's sun (phase 10),
//! rendered through the app's render hook. Filled in by `p10-cmd-sunstudy`.

use crate::cmd::CommandSpec;

/// Command specs (filled in by `p10-cmd-sunstudy`).
pub fn specs() -> Vec<CommandSpec> {
    Vec::new()
}
//! `sunstudy`: a series of stills from the current camera with the site's sun (phase 10),
//! rendered through the app's render hook. The engine has no GPU; the app (CLI / desktop)
//! installs a renderer via [`render::set_render_hooks`], which is what the hook calls.
//!
//! One still per time `from … to` inclusive by `step`, at or above the horizon only (below
//! horizon is listed but not rendered). Times are solar times as `sun` takes them.

use std::path::PathBuf;

use cadcraft_arch::site::true_to_drawing;
use cadcraft_arch::sun;
use serde_json::{json, Value};

use crate::{
    cmd::{CommandSpec, bad},
    EngineError, Session,
};

use super::pick::parse_camera;
use super::render::{render_hooks, RenderRequest, SunChoice};

/// Largest accepted still size per edge, in pixels (matches the `render`/`pick` caps).
const MAX_SIZE: u64 = 8192;
/// At most this many stills in one study; more is refused.
const MAX_STILLS: usize = 96;

/// `sunstudy` command spec.
pub fn specs() -> Vec<CommandSpec> {
    Vec::new()
}
