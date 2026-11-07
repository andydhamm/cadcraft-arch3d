# Sample HDRI (groundwork for brief phase 19)

`kloofendal_48d_partly_cloudy_puresky_1k.hdr`: a 1024 × 512 Radiance RGBE (run-length encoded) sky panorama, midday,
partly cloudy, sky only. It is not in `../catalog.json` yet: phase 19's HDRI work registers it as a Backdrops item once
the backdrop loader reads `.hdr`, so the phase-14 checks (which load every catalog item) keep passing until then.

- Source: Poly Haven, https://polyhaven.com/a/kloofendal_48d_partly_cloudy_puresky (fetched 2026-11-07 through
  https://api.polyhaven.com/files/kloofendal_48d_partly_cloudy_puresky, 1k `hdr`, md5 c69498687e876bf68d2a8b8a62234eb9).
- Authors: Greg Zaal (original), Jarod Guest (sky edits). Licence: CC0-1.0.
