#!/usr/bin/env python3
"""Decode creature frame batches from .playwright-mcp/batch_*.json into JPEGs.

Usage:
  # First: use Playwright to extract batches (see capture-video.sh)
  # Then: python3 tools/export-frames.py [frames_dir] [output.mp4]

Reads batch_*.json files from .playwright-mcp/, decodes base64 JPEGs,
writes numbered frames, and assembles with ffmpeg.
"""
import json, base64, os, glob, subprocess, sys

frames_dir = sys.argv[1] if len(sys.argv) > 1 else "/tmp/creature_frames"
output = sys.argv[2] if len(sys.argv) > 2 else None
batch_dir = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), ".playwright-mcp")

os.makedirs(frames_dir, exist_ok=True)

batch_files = sorted(glob.glob(os.path.join(batch_dir, "batch_*.json")))
if not batch_files:
    print(f"No batch files found in {batch_dir}")
    sys.exit(1)

frame_idx = 0
for bf in batch_files:
    with open(bf) as f:
        data = json.load(f)
    for url in data:
        b64 = url.split(",")[1]
        with open(os.path.join(frames_dir, f"frame_{frame_idx:04d}.jpg"), "wb") as out:
            out.write(base64.b64decode(b64))
        frame_idx += 1
    # Clean up batch file after processing
    os.remove(bf)

print(f"Decoded {frame_idx} frames to {frames_dir}")

if output:
    print(f"Assembling {output}...")
    os.makedirs(os.path.dirname(os.path.abspath(output)), exist_ok=True)
    subprocess.run([
        "ffmpeg", "-y", "-framerate", "60",
        "-i", os.path.join(frames_dir, "frame_%04d.jpg"),
        "-c:v", "libx264", "-pix_fmt", "yuv420p",
        "-preset", "fast", "-crf", "20",
        output
    ], check=True, capture_output=True)
    size = os.path.getsize(output)
    print(f"Saved: {output} ({size // 1024}KB)")
