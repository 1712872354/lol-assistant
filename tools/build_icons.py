# -*- coding: utf-8 -*-
"""按品牌规范打包 Windows 图标资产。

来源：lol-assistant-brand（各尺寸 PNG 已按变体规范预渲染：
<=32 ultra 剪影 / 40-120 compact / 128+ standard），本脚本仅做 ICO 打包与资产拷贝，
不做任何重采样，避免违反品牌规范第 1/4 条。
"""
import io
import os
import shutil
import struct

from PIL import Image

SRC = r"C:\Users\17128\WorkBuddy\2026-09-20-08-57-12\outputs\lol-assistant-brand"
PNG_DIR = os.path.join(SRC, "png")
SVG_DIR = os.path.join(SRC, "svg")
WEB_DIR = os.path.join(SRC, "web")

PROJ = r"E:\idea\LOL\lol-assistant"
WIN_DIR = os.path.join(PROJ, "build", "windows")
PUB_DIR = os.path.join(PROJ, "frontend", "public")


def build_ico(out_path, sizes):
    """用各尺寸专属 PNG（含对应变体）打包多分辨率 ICO（PNG 压缩条目，Win Vista+）。"""
    entries = []
    for s in sizes:
        p = os.path.join(PNG_DIR, "lol-assistant-%d.png" % s)
        if not os.path.exists(p):
            raise FileNotFoundError(p)
        im = Image.open(p).convert("RGBA")
        if im.size != (s, s):
            raise ValueError("size mismatch %s: %r" % (p, im.size))
        buf = io.BytesIO()
        im.save(buf, "PNG", optimize=True)
        entries.append((s, buf.getvalue()))

    count = len(entries)
    header = struct.pack("<HHH", 0, 1, count)
    offset = 6 + 16 * count
    dir_entries = b""
    blobs = b""
    for size, blob in entries:
        dim = 0 if size >= 256 else size
        dir_entries += struct.pack(
            "<BBBBHHII", dim, dim, 0, 0, 1, 32, len(blob), offset
        )
        blobs += blob
        offset += len(blob)

    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    with open(out_path, "wb") as f:
        f.write(header + dir_entries + blobs)
    print("ICO  %-28s sizes=%s  %.1f KB" % (
        os.path.basename(out_path), [s for s, _ in entries],
        (len(header) + len(dir_entries) + len(blobs)) / 1024))


def main():
    # 窗口/EXE/任务栏图标：覆盖 Windows 外壳全部常用档位
    build_ico(os.path.join(WIN_DIR, "icon.ico"),
              [16, 24, 32, 40, 48, 64, 72, 96, 120, 128, 256])
    # 系统托盘：仅小尺寸（托盘区渲染 16-32，菜单 48 兜底）
    build_ico(os.path.join(WIN_DIR, "tray.ico"), [16, 24, 32, 48])
    # NSIS 安装包图标：与主图标同源
    shutil.copyfile(os.path.join(WIN_DIR, "icon.ico"),
                    os.path.join(WIN_DIR, "installer", "icon.ico"))
    print("COPY installer/icon.ico (from icon.ico)")

    # 前端运行时资产
    os.makedirs(PUB_DIR, exist_ok=True)
    copies = [
        (os.path.join(WEB_DIR, "favicon.ico"), "favicon.ico"),
        (os.path.join(SVG_DIR, "lol-assistant-ultra.svg"), "brand-ultra.svg"),
        (os.path.join(SVG_DIR, "lol-assistant-standard.svg"), "brand-standard.svg"),
        (os.path.join(SVG_DIR, "lol-assistant-full.svg"), "brand-full.svg"),
        (os.path.join(PNG_DIR, "lol-assistant-48.png"), "logo-48.png"),
        (os.path.join(PNG_DIR, "lol-assistant-256.png"), "logo-256.png"),
    ]
    for src, name in copies:
        dst = os.path.join(PUB_DIR, name)
        shutil.copyfile(src, dst)
        print("COPY public/%-20s <- %s" % (name, os.path.basename(src)))

    # 校验：Pillow 能重新打开生成的 ICO 且含全部档位
    chk = Image.open(os.path.join(WIN_DIR, "icon.ico"))
    print("VERIFY icon.ico readable, available sizes:", sorted(chk.info.get("sizes", [])))


if __name__ == "__main__":
    main()
