#!/usr/bin/env python3
"""Extract keyframes from video via ffmpeg and emit PNG + JSON index."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Sequence


@dataclass(frozen=True)
class Keyframe:
    file: str
    timestamp_seconds: float
    timestamp_label: str


@dataclass(frozen=True)
class ExtractResult:
    video_path: str
    output_dir: str
    mode: str
    interval: int
    max_frames: int
    keyframes: list[Keyframe]


def env_or_default(name: str, default: str | None = None) -> str | None:
    value = os.getenv(name)
    if value is None:
        return default
    cleaned = value.strip()
    return cleaned if cleaned else default


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Extract keyframes from a video file using ffmpeg.",
    )
    parser.add_argument(
        "--video",
        required=True,
        help="Path to the input video file.",
    )
    parser.add_argument(
        "--output-dir",
        required=True,
        help="Directory where keyframe images and index JSON are written.",
    )
    parser.add_argument(
        "--interval",
        type=int,
        default=int(env_or_default("KF_INTERVAL", "30") or "30"),
        help="Seconds between keyframes (interval mode). Default: KF_INTERVAL env or 30.",
    )
    parser.add_argument(
        "--max-frames",
        type=int,
        default=int(env_or_default("KF_MAX_FRAMES", "50") or "50"),
        help="Maximum number of keyframes to extract. Default: KF_MAX_FRAMES env or 50.",
    )
    parser.add_argument(
        "--mode",
        choices=("interval", "scene", "both"),
        default=env_or_default("KF_MODE", "interval") or "interval",
        help=(
            "Extraction mode: 'interval' (every N seconds), "
            "'scene' (scene change detection), or 'both'. "
            "Default: KF_MODE env or interval."
        ),
    )
    return parser.parse_args(argv)


def find_ffmpeg() -> str:
    """Locate ffmpeg executable."""
    import shutil
    path = shutil.which("ffmpeg")
    if path:
        return path
    # Check common Windows install locations
    winget_base = Path(
        os.environ.get("LOCALAPPDATA", "")
    ) / "Microsoft" / "WinGet" / "Packages"
    if winget_base.exists():
        for pkg in winget_base.iterdir():
            if "ffmpeg" in pkg.name.lower():
                bin_dir = pkg / "bin"
                ffmpeg = shutil.which("ffmpeg", path=str(bin_dir))
                if ffmpeg:
                    return ffmpeg
                # Check nested directory structure
                for child in pkg.rglob("ffmpeg.exe"):
                    return str(child)
    raise RuntimeError(
        "ffmpeg not found. Install with: winget install Gyan.FFmpeg  "
        "(Windows) / brew install ffmpeg (macOS) / sudo apt install ffmpeg (Linux)"
    )


def get_video_duration(video_path: str, ffmpeg_bin: str) -> float | None:
    """Get video duration in seconds via ffprobe."""
    import shutil
    ffprobe = shutil.which("ffprobe")
    if not ffprobe:
        # Try same directory as ffmpeg
        ffprobe = str(Path(ffmpeg_bin).parent / "ffprobe")
        if not Path(ffprobe).exists():
            return None

    try:
        result = subprocess.run(
            [
                ffprobe,
                "-v", "quiet",
                "-print_format", "json",
                "-show_format",
                str(video_path),
            ],
            capture_output=True,
            text=True,
            check=True,
        )
        info = json.loads(result.stdout)
        return float(info.get("format", {}).get("duration", 0))
    except Exception:
        return None


def extract_interval_frames(
    video_path: str,
    output_dir: Path,
    interval: int,
    max_frames: int,
    ffmpeg_bin: str,
) -> list[Keyframe]:
    """Extract frames at regular time intervals."""
    output_dir.mkdir(parents=True, exist_ok=True)

    fps = 1.0 / interval
    filter_v = f"fps={fps:.6f}"

    # Build output pattern
    pattern = str(output_dir / "frame_%04d.png")

    cmd = [
        ffmpeg_bin,
        "-i", str(video_path),
        "-vf", filter_v,
        "-frames:v", str(max_frames),
        "-q:v", "2",
        "-y",
        pattern,
    ]

    subprocess.run(cmd, check=True, capture_output=True)

    # Collect generated frames
    keyframes: list[Keyframe] = []
    for png in sorted(output_dir.glob("frame_*.png")):
        # Parse frame number to get timestamp
        stem = png.stem  # e.g. "frame_0001"
        num = int(stem.split("_")[1])
        ts = (num - 1) * interval
        minutes, seconds = divmod(int(ts), 60)
        hours, minutes = divmod(minutes, 60)
        label = (
            f"{hours:02d}h{minutes:02d}m{seconds:02d}s"
            if hours > 0
            else f"{minutes:02d}m{seconds:02d}s"
        )
        keyframes.append(
            Keyframe(
                file=png.name,
                timestamp_seconds=float(ts),
                timestamp_label=label,
            )
        )

    return keyframes


def extract_scene_frames(
    video_path: str,
    output_dir: Path,
    max_frames: int,
    ffmpeg_bin: str,
    threshold: float = 0.3,
) -> list[Keyframe]:
    """Extract frames at scene changes."""
    output_dir.mkdir(parents=True, exist_ok=True)

    filter_v = f"select=gt(scene\\,{threshold})"
    pattern = str(output_dir / "scene_%04d.png")

    cmd = [
        ffmpeg_bin,
        "-i", str(video_path),
        "-vf", filter_v,
        "-vsync", "vfr",
        "-frames:v", str(max_frames),
        "-q:v", "2",
        "-y",
        pattern,
    ]

    subprocess.run(cmd, check=True, capture_output=True)

    # We need to get timestamps from ffprobe for scene frames
    keyframes: list[Keyframe] = []
    for i, png in enumerate(sorted(output_dir.glob("scene_*.png")), start=1):
        # Estimate timestamp from frame number (rough)
        # For accurate timestamps we'd need to parse ffmpeg output
        minutes, seconds = divmod(i * 10, 60)  # rough estimate
        hours, minutes = divmod(minutes, 60)
        label = (
            f"{hours:02d}h{minutes:02d}m{seconds:02d}s"
            if hours > 0
            else f"{minutes:02d}m{seconds:02d}s"
        )
        keyframes.append(
            Keyframe(
                file=png.name,
                timestamp_seconds=float(i * 10),  # rough estimate
                timestamp_label=label,
            )
        )

    return keyframes


def extract_keyframes(
    video_path: str,
    output_dir: Path,
    mode: str,
    interval: int,
    max_frames: int,
) -> ExtractResult:
    """Main extraction logic."""
    ffmpeg_bin = find_ffmpeg()

    if not Path(video_path).exists():
        raise FileNotFoundError(f"Video file not found: {video_path}")

    frames_dir = output_dir / "frames"
    all_keyframes: list[Keyframe] = []

    if mode in ("interval", "both"):
        interval_frames = extract_interval_frames(
            video_path, frames_dir, interval, max_frames, ffmpeg_bin
        )
        all_keyframes.extend(interval_frames)

    if mode in ("scene", "both"):
        scene_max = max_frames if mode == "scene" else max(max_frames // 3, 5)
        scene_frames = extract_scene_frames(
            video_path, frames_dir, scene_max, ffmpeg_bin
        )
        all_keyframes.extend(scene_frames)

    # Deduplicate and sort by timestamp
    seen_files: set[str] = set()
    unique_keyframes: list[Keyframe] = []
    for kf in sorted(all_keyframes, key=lambda k: k.timestamp_seconds):
        if kf.file not in seen_files:
            seen_files.add(kf.file)
            unique_keyframes.append(kf)

    # Limit to max_frames
    unique_keyframes = unique_keyframes[:max_frames]

    # Write index JSON
    frames_dir.mkdir(parents=True, exist_ok=True)
    index_path = frames_dir / "keyframes.json"
    index_data = [asdict(kf) for kf in unique_keyframes]
    index_path.write_text(
        json.dumps(index_data, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )

    return ExtractResult(
        video_path=str(video_path),
        output_dir=str(output_dir),
        mode=mode,
        interval=interval,
        max_frames=max_frames,
        keyframes=unique_keyframes,
    )


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    video_path = Path(args.video).expanduser().resolve()
    output_dir = Path(args.output_dir).expanduser().resolve()

    if not video_path.exists():
        print(
            f"ERROR: Video file not found: {video_path}",
            file=sys.stderr,
        )
        return 1

    try:
        result = extract_keyframes(
            video_path=str(video_path),
            output_dir=output_dir,
            mode=args.mode,
            interval=args.interval,
            max_frames=args.max_frames,
        )
    except Exception as exc:  # pragma: no cover - CLI error handling
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1

    payload = {
        "result": asdict(result),
    }
    print(json.dumps(payload, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
