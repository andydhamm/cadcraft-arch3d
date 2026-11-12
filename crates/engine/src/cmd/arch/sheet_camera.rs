//! A saved camera's render placed on a sheet (phase 21). Stub: `p21-sheet-camera` fills it.

use crate::Session;
use crate::error::{EngineError, Result};
use cadcraft_doc::arch::sheet::SheetView;

/// Place the render of the camera view `view` on the sheet's layout; the image's name when one is made.
///
/// Stub: refuses until the sheet-camera package lands. Keep the signature.
pub(crate) fn place_camera_image(_s: &mut Session, _sheet: &str, _view: &SheetView) -> Result<Option<String>> {
    Err(EngineError::Other("3D images on sheets are not available yet".into()))
}
