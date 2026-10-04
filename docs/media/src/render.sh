#!/bin/sh
# Renders every poster in docs/media/src to docs/media/<name>.png (1280x640)
# with headless Microsoft Edge or Chrome. Run from docs/media.
BROWSER="${BROWSER:-/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe}"
for page in src/[0-9]*.html; do
  name=$(basename "$page" .html)
  "$BROWSER" --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=1 \
    --window-size=1280,640 --screenshot="$(cygpath -w "$PWD/$name.png" 2>/dev/null || echo "$PWD/$name.png")" \
    "file:///$(cygpath -m "$PWD/$page" 2>/dev/null || echo "$PWD/$page")" >/dev/null 2>&1
  echo "rendered $name.png"
done
