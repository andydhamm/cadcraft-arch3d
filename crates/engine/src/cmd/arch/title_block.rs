//! The master title block of a sheet (phase 21). Stub: `p21-title-block` fills it.

use crate::Session;
use crate::error::Result;

/// Bring every sheet's title block up to date with the sheets and the sheet set.
///
/// Stub: does nothing until the title-block package lands. Keep the signature.
pub(crate) fn refresh_title_blocks(_s: &mut Session) -> Result<()> {
    Ok(())
}
