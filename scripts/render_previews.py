"""Export selected synthetic Rust renderer snapshots as PNG, without image dependencies."""
from pathlib import Path
import struct
import zlib

ROOT = Path(__file__).resolve().parents[1]
SNAPSHOTS = [
    (".scratch/qa-native-render/expanded-474x549.bgra", "note-light.png", 474, 549),
    (".scratch/qa-status-picker/picker-light-366x363.bgra", "status-picker.png", 366, 363),
    (".scratch/qa-preferences/settings-light-appearance-594x969.bgra", "settings-light.png", 594, 969),
]


def png(pixels, width, height):
    if len(pixels) != width * height * 4:
        raise ValueError("Unexpected snapshot dimensions; check the Rust render test")
    rgba = bytearray(len(pixels))
    for offset in range(0, len(pixels), 4):
        blue, green, red, alpha = pixels[offset:offset + 4]
        channels = [min(255, round(channel * 255 / alpha)) if alpha else 0 for channel in (red, green, blue)]
        rgba[offset:offset + 4] = bytes((*channels, alpha))
    rows = b"".join(b"\0" + rgba[row * width * 4:(row + 1) * width * 4] for row in range(height))

    def chunk(kind, value):
        return struct.pack(">I", len(value)) + kind + value + struct.pack(">I", zlib.crc32(kind + value) & 0xffffffff)

    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(rows, 9)) + chunk(b"IEND", b""))


if __name__ == "__main__":
    output = ROOT / "docs/images"
    output.mkdir(parents=True, exist_ok=True)
    for source, name, width, height in SNAPSHOTS:
        source = ROOT / source
        if not source.exists():
            raise SystemExit("Run cargo test --locked first to create synthetic renderer snapshots")
        (output / name).write_bytes(png(source.read_bytes(), width, height))
        print(name)
