"""Draws the Orberus app icon: a filled seal in front of a hollow
one (an account signed in, and a profile waiting for one), on a dark tile.
Drawn large with Pillow, then scaled down for smooth edges."""
import sys
from PIL import Image, ImageDraw, ImageFilter, ImageChops

N = 1024          # final size
K = 4             # drawn K times larger
S = N * K

def vgrad(top, bottom, y0, y1):
    """A full-canvas vertical gradient between two rows."""
    col = Image.new("RGB", (1, S))
    px = col.load()
    for y in range(S):
        t = min(1.0, max(0.0, (y - y0) / (y1 - y0)))
        px[0, y] = tuple(round(a + (b - a) * t) for a, b in zip(top, bottom))
    return col.resize((S, S))

def disc_mask(cx, cy, r):
    m = Image.new("L", (S, S), 0)
    ImageDraw.Draw(m).ellipse([cx - r, cy - r, cx + r, cy + r], fill=255)
    return m

def squircle_mask(inset, power=5.0):
    """A superellipse: the continuous corner of macOS icons."""
    m = Image.new("L", (S // 2, S // 2), 0)
    px = m.load()
    half = S / 4
    a = half - inset * K / 2
    for y in range(S // 2):
        for x in range(S // 2):
            dx, dy = abs(x + 0.5 - half) / a, abs(y + 0.5 - half) / a
            if dx ** power + dy ** power <= 1.0:
                px[x, y] = 255
    return m.resize((S, S), Image.LANCZOS)

def scale(v): return round(v * K)

tile = squircle_mask(100)
canvas = Image.new("RGBA", (S, S), (0, 0, 0, 0))

# Soft shadow under the tile, as on every macOS icon.
shadow = Image.new("RGBA", (S, S), (0, 0, 0, 0))
shadow.putalpha(ImageChops.offset(tile, 0, scale(12)).point(lambda v: v * 0.35))
canvas = Image.alpha_composite(canvas, shadow.filter(ImageFilter.GaussianBlur(scale(14))))

# The tile: deep ink, a little lighter at the top.
art = vgrad((44, 51, 72), (20, 23, 33), scale(100), scale(924)).convert("RGBA")

cy = scale(512)
front = (scale(418), cy, scale(196))
back = (scale(622), cy, scale(196))
stroke = scale(40)

# A glow of each color behind its seal gives the tile some depth.
for (cx, _, r), color, strength in ((back, (91, 192, 190), 0.20), (front, (240, 170, 100), 0.16)):
    glow = Image.new("RGBA", (S, S), color + (0,))
    glow.putalpha(disc_mask(cx, cy, r * 1.25).filter(ImageFilter.GaussianBlur(scale(90))).point(lambda v: v * strength))
    art = Image.alpha_composite(art, glow)

# Back seal: hollow. A ring, with a faint tint inside. It stops short of
# the front seal, so the overlap stays readable at small sizes.
gap = disc_mask(front[0], cy, front[2] + scale(34))
keep = ImageChops.invert(gap)
ring = ImageChops.subtract(disc_mask(*back), disc_mask(back[0], cy, back[2] - stroke))
teal = vgrad((122, 219, 213), (62, 168, 172), cy - back[2], cy + back[2]).convert("RGBA")
teal.putalpha(ImageChops.multiply(ring, keep))
tint = Image.new("RGBA", (S, S), (91, 192, 190, 0))
tint.putalpha(ImageChops.multiply(disc_mask(back[0], cy, back[2] - stroke), keep).point(lambda v: v * 0.13))
art = Image.alpha_composite(Image.alpha_composite(art, tint), teal)

# Front seal: filled, with its own soft shadow and a light from above.
fmask = disc_mask(*front)
fshadow = Image.new("RGBA", (S, S), (0, 0, 0, 0))
fshadow.putalpha(ImageChops.offset(fmask, 0, scale(14)).point(lambda v: v * 0.45))
art = Image.alpha_composite(art, fshadow.filter(ImageFilter.GaussianBlur(scale(18))))
amber = vgrad((250, 196, 128), (222, 136, 66), cy - front[2], cy + front[2]).convert("RGBA")
amber.putalpha(fmask)
art = Image.alpha_composite(art, amber)
# Thin bright rim along the top edge of the disc.
rim = ImageChops.subtract(fmask, ImageChops.offset(fmask, 0, scale(5)))
light = Image.new("RGBA", (S, S), (255, 240, 215, 0))
light.putalpha(rim.filter(ImageFilter.GaussianBlur(scale(1.5))).point(lambda v: v * 0.55))
art = Image.alpha_composite(art, light)

# Thin light edge at the top of the tile.
edge = ImageChops.subtract(tile, ImageChops.offset(tile, 0, scale(3)))
hl = Image.new("RGBA", (S, S), (255, 255, 255, 0))
hl.putalpha(edge.point(lambda v: v * 0.22))
art = Image.alpha_composite(art, hl)

art.putalpha(tile)
canvas = Image.alpha_composite(canvas, art)
canvas.resize((N, N), Image.LANCZOS).save(sys.argv[1])
