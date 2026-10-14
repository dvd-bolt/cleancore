#!/usr/bin/env python3
"""
Генератор минимальных иконок CleanCore для Tauri Windows сборки.
"""

import os
import struct
import zlib
from pathlib import Path

ICONS_DIR = Path(__file__).resolve().parent.parent / "src-tauri" / "icons"
ICONS_DIR.mkdir(parents=True, exist_ok=True)

def create_png_chunk(chunk_type: bytes, data: bytes) -> bytes:
    length = len(data)
    crc = zlib.crc32(chunk_type + data) & 0xffffffff
    return struct.pack(">I", length) + chunk_type + data + struct.pack(">I", crc)

def generate_rgba_png(width: int, height: int, r=224, g=227, b=233, a=255) -> bytes:
    # Заголовок PNG
    png_sig = b"\x89PNG\r\n\x1a\n"
    # IHDR: width, height, bit_depth=8, color_type=6 (RGBA), comp=0, filter=0, interlace=0
    ihdr_data = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    ihdr = create_png_chunk(b"IHDR", ihdr_data)
    
    # Raw scanlines: 1 filter byte (0) + width * 4 bytes
    raw_lines = bytearray()
    for y in range(height):
        raw_lines.append(0) # Filter None
        for x in range(width):
            # Простой круглый жетон
            dx = x - width / 2
            dy = y - height / 2
            radius = width / 2 - 2
            if dx * dx + dy * dy <= radius * radius:
                # Внутренний цвет
                raw_lines.extend([r, g, b, a])
            else:
                raw_lines.extend([0, 0, 0, 0])
                
    idat_data = zlib.compress(bytes(raw_lines), 9)
    idat = create_png_chunk(b"IDAT", idat_data)
    iend = create_png_chunk(b"IEND", b"")
    
    return png_sig + ihdr + idat + iend

def generate_ico_from_png(png_bytes: bytes, width=32, height=32) -> bytes:
    # ICO Header
    ico_header = struct.pack("<HHH", 0, 1, 1) # Reserved=0, Type=1 (ICO), Count=1
    # Directory Entry (16 bytes)
    w_byte = width if width < 256 else 0
    h_byte = height if height < 256 else 0
    size = len(png_bytes)
    offset = 6 + 16 # 22
    ico_entry = struct.pack("<BBBBHHII", w_byte, h_byte, 0, 0, 1, 32, size, offset)
    return ico_header + ico_entry + png_bytes

def main():
    png_32 = generate_rgba_png(32, 32)
    png_128 = generate_rgba_png(128, 128)
    ico = generate_ico_from_png(png_32, 32, 32)
    
    (ICONS_DIR / "32x32.png").write_bytes(png_32)
    (ICONS_DIR / "128x128.png").write_bytes(png_128)
    (ICONS_DIR / "128x128@2x.png").write_bytes(png_128)
    (ICONS_DIR / "icon.png").write_bytes(png_128)
    (ICONS_DIR / "icon.ico").write_bytes(ico)
    (ICONS_DIR / "icon.icns").write_bytes(b"") # dummy
    print("Иконки CleanCore успешно созданы в", ICONS_DIR)

if __name__ == "__main__":
    main()
