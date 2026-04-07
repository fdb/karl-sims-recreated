#!/usr/bin/env bash
# capture-video.sh — Export a frame-perfect 10-second 60fps video of a creature
#
# Usage:
#   ./tools/capture-video.sh <evolution_id> <creature_id> [output.mp4]
#
# Requires: frontend running on localhost:5173, Playwright, ffmpeg
#
# How it works:
#   1. Opens the creature page with ?export=video in headless Chromium
#   2. Waits for the WASM simulation to pre-compute all 600 frames
#   3. Calls window.__creatureExport.renderFrameBatch() to capture each frame
#      as a JPEG data URL — deterministic, no timing dependency
#   4. Writes 600 JPEGs to a temp directory
#   5. ffmpeg assembles them into an MP4 at 60fps

set -euo pipefail

EVO_ID="${1:?Usage: capture-video.sh <evo_id> <creature_id> [output.mp4]}"
CREATURE_ID="${2:?Usage: capture-video.sh <evo_id> <creature_id> [output.mp4]}"
OUTPUT="${3:-album/videos/${CREATURE_ID}.mp4}"
FRONTEND_URL="${FRONTEND_URL:-http://localhost:5173}"
TIMEOUT="${TIMEOUT:-120}"  # seconds to wait for simulation
BATCH_SIZE="${BATCH_SIZE:-30}"  # frames per IPC call

TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT

URL="${FRONTEND_URL}/evolutions/${EVO_ID}/creatures/${CREATURE_ID}?export=video"

echo "Capturing creature #${CREATURE_ID} from evolution #${EVO_ID}..."
echo "Output: ${OUTPUT}"

cat > "${TMPDIR}/capture.mjs" << 'SCRIPT'
import { chromium } from "playwright";
import { writeFileSync, mkdirSync } from "fs";
import { join } from "path";

const url = process.argv[2];
const outDir = process.argv[3];
const batchSize = parseInt(process.argv[4] || "30");
const timeout = parseInt(process.argv[5] || "120") * 1000;

(async () => {
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({
    viewport: { width: 1280, height: 720 },
  });
  const page = await context.newPage();

  console.log(`Navigating to ${url}`);
  await page.goto(url, { waitUntil: "networkidle" });

  // Wait for simulation to finish and export API to be ready
  console.log("Waiting for simulation...");
  await page.waitForFunction(() => window.__creatureExport, { timeout });

  const totalFrames = await page.evaluate(() => window.__creatureExport.totalFrames);
  console.log(`Simulation ready: ${totalFrames} frames`);

  // Extract frames in batches
  const framesDir = join(outDir, "frames");
  mkdirSync(framesDir, { recursive: true });

  for (let start = 0; start < totalFrames; start += batchSize) {
    const count = Math.min(batchSize, totalFrames - start);
    const dataUrls = await page.evaluate(
      ([s, c]) => window.__creatureExport.renderFrameBatch(s, c),
      [start, count]
    );

    for (let i = 0; i < dataUrls.length; i++) {
      const frameNum = start + i;
      const base64 = dataUrls[i].split(",")[1];
      const buf = Buffer.from(base64, "base64");
      writeFileSync(
        join(framesDir, `frame_${String(frameNum).padStart(4, "0")}.jpg`),
        buf
      );
    }

    const pct = Math.round(((start + count) / totalFrames) * 100);
    process.stdout.write(`\rExtracting frames: ${pct}%`);
  }
  console.log("\nFrames extracted.");

  await browser.close();
})();
SCRIPT

# Check if Playwright is available
if ! npx playwright --version > /dev/null 2>&1; then
  echo "Error: Playwright not found. Install with: npm i -D playwright"
  exit 1
fi

# Run the capture script
node "${TMPDIR}/capture.mjs" "${URL}" "${TMPDIR}" "${BATCH_SIZE}" "${TIMEOUT}"

# Assemble with ffmpeg
FRAMES_DIR="${TMPDIR}/frames"
if [ -f "${FRAMES_DIR}/frame_0000.jpg" ]; then
  echo "Assembling MP4..."
  mkdir -p "$(dirname "${OUTPUT}")"
  ffmpeg -y -framerate 60 \
    -i "${FRAMES_DIR}/frame_%04d.jpg" \
    -c:v libx264 -pix_fmt yuv420p \
    -preset fast -crf 20 \
    "${OUTPUT}" 2>/dev/null
  echo "Saved: ${OUTPUT} ($(du -h "${OUTPUT}" | cut -f1))"
else
  echo "Error: No frames found in ${FRAMES_DIR}"
  exit 1
fi
