#!/usr/bin/env bash
# capture-video.sh — Export a 10-second 60fps video of a creature
#
# Usage:
#   ./tools/capture-video.sh <evolution_id> <creature_id> [output.mp4]
#
# Requires: frontend running on localhost:5173, ffmpeg installed
#
# How it works:
#   1. Opens the creature page with ?export=video in a headless browser
#   2. The frontend renders all 600 frames and records via MediaRecorder
#   3. The WebM file is downloaded to a temp directory
#   4. ffmpeg converts WebM → MP4 at 60fps
#
# The script uses Playwright via npx to drive the browser.

set -euo pipefail

EVO_ID="${1:?Usage: capture-video.sh <evo_id> <creature_id> [output.mp4]}"
CREATURE_ID="${2:?Usage: capture-video.sh <evo_id> <creature_id> [output.mp4]}"
OUTPUT="${3:-album/videos/${CREATURE_ID}.mp4}"
FRONTEND_URL="${FRONTEND_URL:-http://localhost:5173}"
TIMEOUT="${TIMEOUT:-120}"  # seconds to wait for export

TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT

URL="${FRONTEND_URL}/evolutions/${EVO_ID}/creatures/${CREATURE_ID}?export=video"
WEBM_PATH="${TMPDIR}/creature-${CREATURE_ID}.webm"

echo "Capturing creature #${CREATURE_ID} from evolution #${EVO_ID}..."
echo "URL: ${URL}"
echo "Output: ${OUTPUT}"

# Use a small Node.js script with Playwright to navigate and wait for the download
cat > "${TMPDIR}/capture.mjs" << 'SCRIPT'
import { chromium } from "playwright";
import { writeFileSync } from "fs";
import { join } from "path";

const url = process.argv[2];
const outDir = process.argv[3];
const creatureId = process.argv[4];
const timeout = parseInt(process.argv[5] || "120") * 1000;

(async () => {
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({
    viewport: { width: 1280, height: 720 },
    acceptDownloads: true,
  });
  const page = await context.newPage();

  // Intercept the download
  page.on("download", async (download) => {
    const path = join(outDir, `creature-${creatureId}.webm`);
    await download.saveAs(path);
    console.log(`Downloaded: ${path}`);
  });

  console.log(`Navigating to ${url}`);
  await page.goto(url, { waitUntil: "networkidle" });

  // Wait for the export to complete (signaled by data-export-done attribute)
  console.log("Waiting for video export to complete...");
  await page.waitForSelector("body[data-export-done]", { timeout });

  // Give the download a moment to save
  await page.waitForTimeout(2000);

  await browser.close();
  console.log("Done.");
})();
SCRIPT

# Check if Playwright is available
if ! npx playwright --version > /dev/null 2>&1; then
  echo "Error: Playwright not found. Install with: npm i -D playwright"
  echo ""
  echo "Alternative: open this URL in your browser to export manually:"
  echo "  ${URL}"
  echo ""
  echo "The browser will auto-download a .webm file, then convert with:"
  echo "  ffmpeg -i creature-${CREATURE_ID}.webm -c:v libx264 -pix_fmt yuv420p -r 60 ${OUTPUT}"
  exit 1
fi

# Run the capture script
node "${TMPDIR}/capture.mjs" "${URL}" "${TMPDIR}" "${CREATURE_ID}" "${TIMEOUT}"

# Convert WebM → MP4
if [ -f "${WEBM_PATH}" ]; then
  echo "Converting to MP4..."
  mkdir -p "$(dirname "${OUTPUT}")"
  ffmpeg -y -i "${WEBM_PATH}" \
    -c:v libx264 -pix_fmt yuv420p -r 60 \
    -preset fast -crf 23 \
    "${OUTPUT}" 2>/dev/null
  echo "Saved: ${OUTPUT} ($(du -h "${OUTPUT}" | cut -f1))"
else
  echo "Warning: WebM not found at ${WEBM_PATH}"
  echo ""
  echo "You can export manually by opening this URL in your browser:"
  echo "  ${URL}"
  echo ""
  echo "Then convert with:"
  echo "  ffmpeg -i creature-${CREATURE_ID}.webm -c:v libx264 -pix_fmt yuv420p -r 60 ${OUTPUT}"
fi
