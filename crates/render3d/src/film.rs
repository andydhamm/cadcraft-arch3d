use cadcraft_doc::arch::Film;

/// Apply the still's film to linear RGB pixels before the tone curve.
///
/// The phase-22 film lands in a later segment; until then the film is neutral and this stub leaves
/// the pixels unchanged.
pub(crate) fn apply(film: &Film, pixels: &mut [[f32; 3]], size: (u32, u32)) {
    let _ = (film, pixels, size);
}
