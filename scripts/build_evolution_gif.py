#!/usr/bin/env python3
"""Build the gate-evolution GIF/MP4 from per-gate diagnostic artifacts.

Sources are the gate-local artifact trees (not a single latest-run folder).
1B0/1B1 have no PPM → title cards. R1/E0 ends on a corpus montage + lock digest.

Requires: ImageMagick (`magick`), ffmpeg (optional, for mp4).

Usage (repo root):
  python3 scripts/build_evolution_gif.py
"""

from __future__ import annotations

import hashlib
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT_DIR = ROOT / "docs" / "media"
FRAMES_DIR = OUT_DIR / "evolution-frames"
GIF_PATH = OUT_DIR / "blackhole-rust-evolution.gif"
MP4_PATH = OUT_DIR / "blackhole-rust-evolution.mp4"
SIZE = 640
HOLD_CS = 150  # ImageMagick centiseconds ≈ 1.5s

FONT_CANDIDATES = [
    "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
    "/Library/Fonts/Arial Unicode.ttf",
    "/System/Library/Fonts/Helvetica.ttc",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
]


def font() -> str:
    for path in FONT_CANDIDATES:
        if Path(path).is_file():
            return path
    raise SystemExit("no usable font found")


def require(path: Path) -> Path:
    if not path.is_file():
        raise SystemExit(f"missing artifact: {path.relative_to(ROOT)}")
    return path


def run(cmd: list[str]) -> None:
    subprocess.check_call(cmd)


def annotate(src: Path, dst: Path, gate: str, note: str, font_path: str) -> None:
    """Compact top-left caption: gate + one short note."""
    # Narrow bar so diagnostics stay visible.
    run(
        [
            "magick",
            str(src),
            "-filter",
            "point",
            "-resize",
            f"{SIZE}x{SIZE}",
            "-gravity",
            "NorthWest",
            "-fill",
            "rgba(0,0,0,0.72)",
            "-draw",
            "roundrectangle 12,12 392,78 8,8",
            "-fill",
            "white",
            "-font",
            font_path,
            "-pointsize",
            "26",
            "-annotate",
            "+24+24",
            gate,
            "-fill",
            "#C9DBF5",
            "-pointsize",
            "16",
            "-annotate",
            "+24+54",
            note,
            "-gravity",
            "SouthWest",
            "-fill",
            "rgba(0,0,0,0.55)",
            "-draw",
            f"rectangle 0,{SIZE - 28} {SIZE},{SIZE}",
            "-fill",
            "#B8B8B8",
            "-pointsize",
            "12",
            "-annotate",
            "+12+8",
            "diagnostic channels · not a beauty render",
            str(dst),
        ]
    )


def title_card(dst: Path, gate: str, note: str, detail: str, font_path: str) -> None:
    run(
        [
            "magick",
            "-size",
            f"{SIZE}x{SIZE}",
            "xc:#141820",
            "-gravity",
            "NorthWest",
            "-fill",
            "#7EB6FF",
            "-font",
            font_path,
            "-pointsize",
            "18",
            "-annotate",
            "+36+48",
            "blackhole-rust · gate evolution",
            "-fill",
            "white",
            "-pointsize",
            "42",
            "-annotate",
            "+36+96",
            gate,
            "-fill",
            "#D7E8FF",
            "-pointsize",
            "22",
            "-annotate",
            "+36+168",
            note,
            "-fill",
            "#9AA7B8",
            "-pointsize",
            "16",
            "-annotate",
            "+36+220",
            detail,
            "-gravity",
            "SouthWest",
            "-fill",
            "#6A7380",
            "-pointsize",
            "12",
            "-annotate",
            "+36+28",
            "no image artifact at this gate · evidence was numerical",
            str(dst),
        ]
    )


def dual_panel(left: Path, right: Path, dst: Path, left_label: str, right_label: str) -> None:
    half = SIZE // 2
    run(
        [
            "magick",
            "(",
            str(left),
            "-filter",
            "point",
            "-resize",
            f"{half}x{SIZE}!",
            ")",
            "(",
            str(right),
            "-filter",
            "point",
            "-resize",
            f"{half}x{SIZE}!",
            ")",
            "+append",
            "-gravity",
            "NorthWest",
            "-fill",
            "rgba(0,0,0,0.65)",
            "-draw",
            f"rectangle 0,0 {half},22",
            "-draw",
            f"rectangle {half},0 {SIZE},22",
            "-fill",
            "#DDDDDD",
            "-pointsize",
            "13",
            "-annotate",
            "+8+4",
            left_label,
            "-annotate",
            f"+{half + 8}+4",
            right_label,
            str(dst),
        ]
    )


def corpus_montage(paths: list[Path], labels: list[str], dst: Path, digest_line: str, font_path: str) -> None:
    cell = SIZE // 2
    tiles: list[Path] = []
    tmp = dst.parent / "_montage_tiles"
    tmp.mkdir(parents=True, exist_ok=True)
    for i, (src, label) in enumerate(zip(paths, labels, strict=True)):
        tile = tmp / f"tile-{i}.png"
        run(
            [
                "magick",
                str(src),
                "-filter",
                "point",
                "-resize",
                f"{cell}x{cell}",
                "-gravity",
                "SouthWest",
                "-fill",
                "rgba(0,0,0,0.7)",
                "-draw",
                f"rectangle 0,{cell - 22} {cell},{cell}",
                "-fill",
                "#E8E8E8",
                "-pointsize",
                "12",
                "-annotate",
                "+6+4",
                label,
                str(tile),
            ]
        )
        tiles.append(tile)
    montage = tmp / "grid.png"
    run(
        [
            "magick",
            str(tiles[0]),
            str(tiles[1]),
            "+append",
            str(tmp / "row0.png"),
        ]
    )
    run(
        [
            "magick",
            str(tiles[2]),
            str(tiles[3]),
            "+append",
            str(tmp / "row1.png"),
        ]
    )
    run(["magick", str(tmp / "row0.png"), str(tmp / "row1.png"), "-append", str(montage)])
    run(
        [
            "magick",
            str(montage),
            "-gravity",
            "NorthWest",
            "-fill",
            "rgba(0,0,0,0.78)",
            "-draw",
            "roundrectangle 12,12 420,78 8,8",
            "-fill",
            "white",
            "-font",
            font_path,
            "-pointsize",
            "26",
            "-annotate",
            "+24+24",
            "R1 / E0",
            "-fill",
            "#C9DBF5",
            "-pointsize",
            "16",
            "-annotate",
            "+24+54",
            "+ OracleFrame corpus lock",
            "-gravity",
            "SouthWest",
            "-fill",
            "rgba(0,0,0,0.72)",
            "-draw",
            f"rectangle 0,{SIZE - 36} {SIZE},{SIZE}",
            "-fill",
            "#B8D4FF",
            "-pointsize",
            "13",
            "-annotate",
            "+12+12",
            digest_line,
            str(dst),
        ]
    )
    shutil.rmtree(tmp, ignore_errors=True)


def main() -> int:
    if shutil.which("magick") is None:
        print("magick (ImageMagick) required", file=sys.stderr)
        return 1

    fnt = font()
    FRAMES_DIR.mkdir(parents=True, exist_ok=True)
    for stale in FRAMES_DIR.glob("frame-*.png"):
        stale.unlink()

    # Per-gate sources (historical trees).
    src_1b2 = require(ROOT / "artifacts/gate-1b2/outcome-map.ppm")
    src_2a0_cat = require(
        ROOT / "artifacts/gate-2a0-trace-shade/authoritative-128-run-0/gate1b2-categorical.ppm"
    )
    src_2a0_disk = require(
        ROOT / "artifacts/gate-2a0-trace-shade/authoritative-128-run-0/disk-suppressed.ppm"
    )
    src_2a1 = require(
        ROOT / "artifacts/gate-2a1-celestial-directions/gate-run-0/celestial-uv-debug.ppm"
    )
    src_2a2 = require(
        ROOT
        / "artifacts/gate-2a2-lensed-celestial/opaque-gate-run-0/lensed-celestial-opaque-disk-mask.ppm"
    )
    src_2b0 = require(ROOT / "artifacts/gate-2b0-frequency-shift/gate-run-0/g-factor-debug.ppm")
    src_2b1 = require(
        ROOT
        / "artifacts/gate-2b1-bolometric-radiance/gate-run-0/bolometric-disk-celestial-composite.ppm"
    )
    src_2b2 = require(
        ROOT / "artifacts/gate-2b2-spectral-transport/gate-run-0/observed-integral.pgm"
    )
    lock = require(ROOT / "experiments/oracle-benchmark/corpus-lock-v1.json")
    lock_digest = hashlib.sha256(lock.read_bytes()).hexdigest()

    corpus_paths = [
        require(ROOT / "artifacts/r1-e0-oracle-corpus/eval-subprocess-a/cases/schwarzschild-edge-sky/reference.ppm"),
        require(ROOT / "artifacts/r1-e0-oracle-corpus/eval-subprocess-a/cases/kerr0999-edge-opaque/reference.ppm"),
        require(ROOT / "artifacts/r1-e0-oracle-corpus/eval-subprocess-a/cases/kerr0999-midinc-opaque/reference.ppm"),
        require(
            ROOT
            / "artifacts/r1-e0-oracle-corpus/eval-subprocess-a/crops/kerr0999-edge-opaque-boundary-crop/reference.ppm"
        ),
    ]
    corpus_labels = [
        "Schwarzschild sky",
        "Kerr a=0.999 opaque",
        "Kerr mid-inc opaque",
        "boundary crop 64²",
    ]

    frames: list[Path] = []
    n = 0

    def next_frame() -> Path:
        nonlocal n
        path = FRAMES_DIR / f"frame-{n:02d}.png"
        n += 1
        return path

    # 1B0 / 1B1 — numerical gates, title cards
    p = next_frame()
    title_card(
        p,
        "Gate 1B0",
        "+ DOP853 / ivp candidate spike",
        "ODE+IVP matrix · SolOut · vector tolerances",
        fnt,
    )
    frames.append(p)

    p = next_frame()
    title_card(
        p,
        "Gate 1B1",
        "+ typed event localization",
        "EventHit / SurfaceApproach · Kerr convergence corpus",
        fnt,
    )
    frames.append(p)

    p = next_frame()
    annotate(src_1b2, p, "Gate 1B2", "+ typed ray outcome map", fnt)
    frames.append(p)

    # 2A0 — same categorical bits as 1B2 alone; show trace/shade split instead
    dual = FRAMES_DIR / "_dual-2a0.png"
    dual_panel(src_2a0_cat, src_2a0_disk, dual, "categorical", "disk-suppressed")
    p = next_frame()
    annotate(dual, p, "Gate 2A0", "+ trace/shade + release path", fnt)
    dual.unlink(missing_ok=True)
    frames.append(p)

    p = next_frame()
    annotate(src_2a1, p, "Gate 2A1", "+ celestial UV directions", fnt)
    frames.append(p)

    p = next_frame()
    annotate(src_2a2, p, "Gate 2A2", "+ lensed sky + opaque disk", fnt)
    frames.append(p)

    p = next_frame()
    annotate(src_2b0, p, "Gate 2B0", "+ disk frequency-shift g", fnt)
    frames.append(p)

    p = next_frame()
    annotate(src_2b1, p, "Gate 2B1", "+ I_obs = g⁴ I_em composite", fnt)
    frames.append(p)

    p = next_frame()
    annotate(src_2b2, p, "Gate 2B2", "+ ∫ I_ν,obs (g³ continuum)", fnt)
    frames.append(p)

    p = next_frame()
    corpus_montage(
        corpus_paths,
        corpus_labels,
        p,
        f"lock sha256 {lock_digest[:16]}… · e0-oracle-corpus-v1",
        fnt,
    )
    frames.append(p)

    run(
        [
            "magick",
            "-delay",
            str(HOLD_CS),
            "-loop",
            "0",
            *[str(f) for f in frames],
            "-layers",
            "Optimize",
            str(GIF_PATH),
        ]
    )
    print(f"wrote {GIF_PATH.relative_to(ROOT)} ({GIF_PATH.stat().st_size} bytes)")

    if shutil.which("ffmpeg"):
        # ~1.5s/frame → framerate 2/3
        run(
            [
                "ffmpeg",
                "-y",
                "-framerate",
                "2/3",
                "-i",
                str(FRAMES_DIR / "frame-%02d.png"),
                "-vf",
                f"fps=10,scale={SIZE}:-1:flags=neighbor",
                "-pix_fmt",
                "yuv420p",
                str(MP4_PATH),
            ]
        )
        print(f"wrote {MP4_PATH.relative_to(ROOT)} ({MP4_PATH.stat().st_size} bytes)")
    else:
        print("ffmpeg not found; skipped mp4", file=sys.stderr)

    print(f"frames: {len(frames)} in {FRAMES_DIR.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
