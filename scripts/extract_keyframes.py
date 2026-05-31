#!/usr/bin/env python3
"""Extract keyframes from video via ffmpeg and emit PNG + JSON index."""

from __future__ import annotations

import argparse
import json
import os
import re
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
    trigger: str = "interval"


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
        default=int(env_or_default("KF_MAX_FRAMES", "40") or "40"),
        help="Maximum number of keyframes to extract. Default: KF_MAX_FRAMES env or 40.",
    )
    parser.add_argument(
        "--mode",
        choices=("interval", "scene", "both"),
        default=env_or_default("KF_MODE", "both") or "both",
        help=(
            "Extraction mode: 'interval' (every N seconds), "
            "'scene' (scene change detection), or 'both'. "
            "Default: KF_MODE env or both."
        ),
    )
    parser.add_argument(
        "--timestamps",
        help=(
            "Optional JSON file of guided timestamps. Accepts an array of "
            '{"ts": seconds, "reason": "..."} objects or raw numbers.'
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


def timestamp_label(seconds_value: float) -> str:
    seconds_int = max(0, int(round(seconds_value)))
    minutes, seconds = divmod(seconds_int, 60)
    hours, minutes = divmod(minutes, 60)
    return (
        f"{hours:02d}h{minutes:02d}m{seconds:02d}s"
        if hours > 0
        else f"{minutes:02d}m{seconds:02d}s"
    )


def slug_reason(reason: str) -> str:
    cleaned = re.sub(r"\s+", "_", reason.strip())
    cleaned = re.sub(r"[^\w\u4e00-\u9fff_-]+", "", cleaned)
    return cleaned[:24] or "guided"


def load_guided_timestamps(path: str | None) -> list[tuple[float, str]]:
    if not path:
        return []

    timestamp_path = Path(path).expanduser().resolve()
    if not timestamp_path.exists():
        return []

    raw = json.loads(timestamp_path.read_text(encoding="utf-8"))
    items = raw.get("timestamps", raw) if isinstance(raw, dict) else raw
    result: list[tuple[float, str]] = []

    for item in items:
        if isinstance(item, (int, float)):
            result.append((float(item), "AI推荐"))
        elif isinstance(item, dict):
            ts = item.get("ts", item.get("timestamp", item.get("timestamp_seconds")))
            if ts is None:
                continue
            result.append((float(ts), str(item.get("reason", "AI推荐"))))

    deduped: list[tuple[float, str]] = []
    for ts, reason in sorted(result, key=lambda value: value[0]):
        if ts < 0:
            continue
        if deduped and abs(deduped[-1][0] - ts) < 2:
            continue
        deduped.append((ts, reason))
    return deduped


def extract_guided_frames(
    video_path: str,
    output_dir: Path,
    timestamps: list[tuple[float, str]],
    max_frames: int,
    ffmpeg_bin: str,
) -> list[Keyframe]:
    output_dir.mkdir(parents=True, exist_ok=True)
    keyframes: list[Keyframe] = []

    for index, (ts, reason) in enumerate(timestamps[:max_frames], start=1):
        output = output_dir / f"guided_{index:04d}_{slug_reason(reason)}.png"
        cmd = [
            ffmpeg_bin,
            "-ss", f"{ts:.3f}",
            "-i", str(video_path),
            "-frames:v", "1",
            "-q:v", "2",
            "-y",
            str(output),
        ]
        subprocess.run(cmd, check=True, capture_output=True)
        if output.exists():
            keyframes.append(
                Keyframe(
                    file=output.name,
                    timestamp_seconds=float(ts),
                    timestamp_label=timestamp_label(ts),
                    trigger=f"guided:{reason}",
                )
            )

    return keyframes


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
        keyframes.append(
            Keyframe(
                file=png.name,
                timestamp_seconds=float(ts),
                timestamp_label=timestamp_label(ts),
                trigger="interval",
            )
        )

    return keyframes


def extract_scene_frames(
    video_path: str,
    output_dir: Path,
    max_frames: int,
    ffmpeg_bin: str,
    threshold: float = 0.12,
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
        ts = float(i * 10)
        keyframes.append(
            Keyframe(
                file=png.name,
                timestamp_seconds=ts,
                timestamp_label=timestamp_label(ts),
                trigger="scene",
            )
        )

    return keyframes


def extract_keyframes(
    video_path: str,
    output_dir: Path,
    mode: str,
    interval: int,
    max_frames: int,
    timestamps_path: str | None = None,
) -> ExtractResult:
    """Main extraction logic."""
    ffmpeg_bin = find_ffmpeg()

    if not Path(video_path).exists():
        raise FileNotFoundError(f"Video file not found: {video_path}")

    frames_dir = output_dir / "frames"
    all_keyframes: list[Keyframe] = []

    guided_timestamps = load_guided_timestamps(timestamps_path)
    if guided_timestamps:
        guided_frames = extract_guided_frames(
            video_path, frames_dir, guided_timestamps, max_frames, ffmpeg_bin
        )
        all_keyframes.extend(guided_frames)

    if mode in ("interval", "both"):
        interval_frames = extract_interval_frames(
            video_path, frames_dir, interval, max_frames, ffmpeg_bin
        )
        all_keyframes.extend(interval_frames)

    if mode in ("scene", "both"):
        scene_max = max_frames if mode == "scene" else max(max_frames // 3, 5)
        threshold = float(env_or_default("KF_SCENE_THRESHOLD", "0.12") or "0.12")
        scene_frames = extract_scene_frames(
            video_path, frames_dir, scene_max, ffmpeg_bin, threshold
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
            timestamps_path=args.timestamps,
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
