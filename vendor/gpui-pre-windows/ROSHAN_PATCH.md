# Roshan patch to gpui-pre-windows 0.3.7

Upstream crate: https://crates.io/crates/gpui-pre-windows/0.3.7 (Apache-2.0,
see LICENSE-APACHE). Changes:

* `DrawGlyphRun` positions glyphs from DirectWrite's `baselineOriginX` instead
  of accumulating a running width, and mirrors glyph placement for runs with an
  odd bidi level. Without this, Arabic-script text (Persian) renders mirrored.
* `StringIndexConverter::seek_to_utf16_ix` can seek backwards, because runs of
  a right-to-left segment are drawn in visual, not logical, order.

* `src/window.rs` / `src/events.rs`: windows are created without
  `WS_MAXIMIZEBOX`, and a double click on the caption is swallowed, so the
  compact Roshan window can be resized but never maximized.

Search for `ROSHAN PATCH` to find every change. Drop this vendored copy once
upstream GPUI handles right-to-left runs.
