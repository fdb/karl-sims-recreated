import { Muxer, ArrayBufferTarget } from "mp4-muxer";

export interface ExportOptions {
  canvas: HTMLCanvasElement;
  totalFrames: number;
  fps: number;
  /** Called before each frame is rendered — the caller should render the frame to the canvas. */
  renderFrame: (frameIndex: number) => void;
  /** Progress callback: 0..1 */
  onProgress?: (fraction: number) => void;
}

/**
 * Encode all pre-computed frames from a Three.js canvas into an MP4 using
 * the WebCodecs VideoEncoder API + mp4-muxer.
 *
 * This runs faster than real-time because we drive the renderer frame-by-frame
 * rather than waiting for requestAnimationFrame.
 */
export async function exportToMp4({
  canvas,
  totalFrames,
  fps,
  renderFrame,
  onProgress,
}: ExportOptions): Promise<Blob> {
  // Use CSS pixel dimensions (not the device-pixel-ratio-scaled canvas.width)
  // to keep the encoded resolution reasonable and AVC-compatible.
  // H.264 requires even dimensions — round down to nearest even number.
  const width = ((canvas.clientWidth || canvas.width) & ~1);
  const height = ((canvas.clientHeight || canvas.height) & ~1);

  // AVC level selection based on coded area (width rounded up to 16 * height rounded up to 16)
  const codedArea = Math.ceil(width / 16) * 16 * Math.ceil(height / 16) * 16;
  let avcLevel: string;
  if (codedArea <= 921_600) avcLevel = "1f";       // 3.1 — up to ~1280x720
  else if (codedArea <= 2_088_960) avcLevel = "28"; // 4.0 — up to ~1920x1080
  else avcLevel = "33";                             // 5.1 — up to 4K

  let encoderError: Error | null = null;

  const muxer = new Muxer({
    target: new ArrayBufferTarget(),
    video: {
      codec: "avc",
      width,
      height,
      frameRate: fps,
    },
    fastStart: "in-memory",
  });

  const encoder = new VideoEncoder({
    output: (chunk, meta) => muxer.addVideoChunk(chunk, meta ?? undefined),
    error: (e) => {
      encoderError = new Error(`VideoEncoder error: ${e.message}`);
    },
  });

  encoder.configure({
    codec: `avc1.4200${avcLevel}`,
    width,
    height,
    bitrate: 5_000_000,
    framerate: fps,
  });

  const frameDuration = 1_000_000 / fps; // microseconds per frame

  // If the canvas is HiDPI-scaled, draw into an offscreen canvas at the target size
  const needsScale = canvas.width !== width || canvas.height !== height;
  const offscreen = needsScale ? document.createElement("canvas") : null;
  if (offscreen) {
    offscreen.width = width;
    offscreen.height = height;
  }

  for (let i = 0; i < totalFrames; i++) {
    if (encoderError) throw encoderError;

    renderFrame(i);

    let frameSource: HTMLCanvasElement;
    if (offscreen) {
      const ctx = offscreen.getContext("2d")!;
      ctx.drawImage(canvas, 0, 0, width, height);
      frameSource = offscreen;
    } else {
      frameSource = canvas;
    }

    const frame = new VideoFrame(frameSource, {
      timestamp: Math.round(i * frameDuration),
      duration: Math.round(frameDuration),
    });

    const keyFrame = i % 60 === 0; // keyframe every second
    encoder.encode(frame, { keyFrame });
    frame.close();

    // Yield to the browser every 30 frames so the UI stays responsive
    if (i % 30 === 0) {
      onProgress?.(i / totalFrames);
      await new Promise<void>((r) => setTimeout(r, 0));
    }
  }

  if (encoderError) throw encoderError;

  await encoder.flush();
  encoder.close();
  muxer.finalize();

  const buffer = (muxer.target as ArrayBufferTarget).buffer;
  return new Blob([buffer], { type: "video/mp4" });
}

/** Trigger a browser download for a Blob. */
export function downloadBlob(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}
