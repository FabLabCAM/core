#!/usr/bin/env python3
"""Dessine l'icône de FabLabCAM Studio (fond bleu nuit, trajectoire de poche en spirale rectangulaire)
et l'écrit en PNG 1024×1024, sans dépendance. usage : make-icon.py sortie.png"""
import struct, sys, zlib

N = 1024
bg, fg, acc = (24, 38, 64), (236, 240, 246), (255, 140, 40)
px = bytearray(N * N * 4)

def inside_round(x, y, r):
    cx = min(max(x, r), N - 1 - r); cy = min(max(y, r), N - 1 - r)
    return (x - cx) ** 2 + (y - cy) ** 2 <= r * r

def seg_dist(x, y, a, b):
    ax, ay, bx, by = a[0], a[1], b[0], b[1]
    dx, dy = bx - ax, by - ay
    t = max(0, min(1, ((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy)))
    return ((x - ax - t * dx) ** 2 + (y - ay - t * dy) ** 2) ** 0.5

# spirale rectangulaire de l'extérieur vers l'intérieur
pts, lo, hi, step = [], 250, 774, 86
x0, y0, x1, y1 = lo, lo, hi, hi
pts.append((x0, y0))
while x1 - x0 > step and y1 - y0 > step:
    pts += [(x1, y0), (x1, y1), (x0, y1), (x0, y0 + step)]
    x0 += step; y0 += step; x1 -= step; y1 -= step
    pts.append((x0, y0))
segs = list(zip(pts, pts[1:]))
W = 17  # demi-épaisseur du trait

for y in range(N):
    for x in range(N):
        i = (y * N + x) * 4
        if not inside_round(x, y, 180):
            continue
        # léger dégradé vertical
        k = y / N
        c = tuple(int(v * (1.15 - 0.3 * k)) for v in bg)
        d = min(seg_dist(x, y, a, b) for a, b in segs) if lo - 40 < x < hi + 40 and lo - 40 < y < hi + 40 else 99
        if d < W:
            c = fg
        elif d < W + 1.5:
            f = (W + 1.5 - d) / 1.5
            c = tuple(int(c[j] * (1 - f) + fg[j] * f) for j in range(3))
        # point d'entrée de la fraise
        e = ((x - pts[0][0]) ** 2 + (y - pts[0][1]) ** 2) ** 0.5
        if e < 40:
            c = acc
        px[i:i + 4] = bytes((min(c[0], 255), min(c[1], 255), min(c[2], 255), 255))

def chunk(t, d):
    c = struct.pack(">I", len(d)) + t + d
    return c + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

raw = b"".join(b"\x00" + bytes(px[y * N * 4:(y + 1) * N * 4]) for y in range(N))
png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", N, N, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
open(sys.argv[1], "wb").write(png)

# usage : make-icon.py sortie.png [sortie.rgba] — la seconde sortie est une image 128×128 RGBA brute
# (moyenne par blocs de 8×8), embarquée dans l'exécutable pour l'icône de fenêtre (X11, Windows)
if len(sys.argv) > 2:
    S, B = 128, N // 128
    out = bytearray()
    for y in range(S):
        for x in range(S):
            acc_px = [0, 0, 0, 0]
            for dy in range(B):
                for dx in range(B):
                    i = ((y * B + dy) * N + x * B + dx) * 4
                    a = px[i + 3]
                    acc_px[0] += px[i] * a; acc_px[1] += px[i + 1] * a; acc_px[2] += px[i + 2] * a; acc_px[3] += a
            a = acc_px[3]
            out += bytes((acc_px[0] // a, acc_px[1] // a, acc_px[2] // a, a // (B * B)) if a else (0, 0, 0, 0))
    open(sys.argv[2], "wb").write(out)
