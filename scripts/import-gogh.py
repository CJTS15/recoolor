#!/usr/bin/env python3
"""Converts Gogh's terminal themes into Recoolor palettes, keeping only the ones
that make a readable desktop.

Usage: python3 -I scripts/import-gogh.py THEMES_JSON OUT_DIR

THEMES_JSON is Gogh's data/themes.json
(https://github.com/Gogh-Co/Gogh, MIT or Apache-2.0). OUT_DIR is replaced with
one TOML per kept theme. Prints why each theme was dropped with --verbose.

A Gogh theme is a background, a foreground and 16 ANSI colors. Recoolor needs
window, sidebar, card and border shades and eight hues, so those are derived:
the shades mix the background toward the foreground (and away from it for the
darker ones), the hues take the normal or bright ANSI color that reads better,
orange and pink are mixed from their neighbours.
"""

import json
import math
import re
import sys
from pathlib import Path

# Families Recoolor ships hand-tuned; Gogh's versions are skipped.
BUNDLED = (
    "catppuccin", "dracula", "everforest", "gruvbox", "kanagawa", "nord",
    "one dark", "rose pine", "rosé pine", "solarized", "tokyo night",
)

MIN_TEXT = 7.0       # text on the window
MIN_SUBTEXT = 4.5    # secondary text
MIN_HUE = 3.0        # each hue on the window (UI icons and large text)
MIN_HUE_CARD = 2.5   # each hue on cards
MIN_ACCENT_FG = 4.5  # text on accent-coloured buttons
MIN_ROLE_TEXT = 4.5  # accent, red, green and yellow as libadwaita shows them as text
MAX_BG_CHROMA = 0.06 # windows stay near neutral
DARK_BG = (0.16, 0.36) # OKLab lightness of dark windows: no pure black
MIN_LIGHT_BG = 0.88    # light windows really light
MIN_HUE_CHROMA = 0.06
MIN_HUE_GAP = 18.0   # degrees between the six hues
SAME_THEME = 0.03    # OKLab distance under which two themes count as one
MAX_NUDGE = 0.15     # OKLab lightness a colour may move to become readable


def rgb(hex_):
    hex_ = hex_.strip().lstrip("#")
    return tuple(int(hex_[i:i + 2], 16) for i in (0, 2, 4))


def hex_(c):
    return "#{:02x}{:02x}{:02x}".format(*(max(0, min(255, round(v))) for v in c))


def mix(a, b, t):
    # Whole numbers, so every check sees the colour as written.
    return tuple(round(x + (y - x) * t) for x, y in zip(a, b))


def lin(v):
    v /= 255
    return v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4


def luminance(c):
    r, g, b = (lin(v) for v in c)
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def contrast(a, b):
    hi, lo = sorted((luminance(a), luminance(b)), reverse=True)
    return (hi + 0.05) / (lo + 0.05)


def oklab(c):
    r, g, b = (lin(v) for v in c)
    l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b
    m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b
    s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b
    l, m, s = (math.copysign(abs(x) ** (1 / 3), x) for x in (l, m, s))
    return (
        0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    )


def from_oklab(lab):
    L, a, b = lab
    l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3
    m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3
    s = (L - 0.0894841775 * a - 1.2914855480 * b) ** 3
    lin_rgb = (
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    )
    def encode(v):
        v = min(max(v, 0.0), 1.0)
        return 255 * (12.92 * v if v <= 0.0031308 else 1.055 * v ** (1 / 2.4) - 0.055)
    return tuple(round(encode(v)) for v in lin_rgb)


def nudge(color, backgrounds, ratio, lighter, limit=MAX_NUDGE):
    """`color` with its lightness moved away from the backgrounds until it has
    `ratio` against each, keeping its hue; None if that takes more than `limit`."""
    L, a, b = oklab(color)
    for step in range(int(limit * 100) + 1):
        candidate = from_oklab((L + (step if lighter else -step) / 100, a, b))
        if all(contrast(bg, candidate) >= ratio for bg in backgrounds):
            return candidate
    return None


def standalone(color, dark):
    """libadwaita's text form of a fill: OKLab lightness at most 0.5 in light
    style, at least 0.85 in dark (Color::standalone in Recoolor)."""
    L, a, b = oklab(color)
    return from_oklab((max(L, 0.85) if dark else min(L, 0.5), a, b))


def readable_as_text(color, base, dark, limit):
    """`color`, moved in lightness up to `limit` until its text form reads on the
    window; None if it cannot."""
    L, a, b = oklab(color)
    for step in range(int(limit * 100) + 1):
        candidate = from_oklab((L + (step if dark else -step) / 100, a, b))
        if contrast(base, standalone(candidate, dark)) >= MIN_ROLE_TEXT:
            return candidate
    return None


def chroma(c):
    _, a, b = oklab(c)
    return math.hypot(a, b)


def hue(c):
    _, a, b = oklab(c)
    return math.degrees(math.atan2(b, a)) % 360


def best_foreground(bg, options):
    return max(options, key=lambda fg: contrast(bg, fg))


def slug(name):
    s = name.lower().replace("+", " plus ").replace("&", " and ")
    s = re.sub(r"[^a-z0-9]+", "-", s).strip("-")
    return s


def convert(theme, limit):
    """A Recoolor palette dict for a Gogh theme, or the reason it does not qualify.
    Colours may move up to `limit` in lightness to become readable."""
    base, text = rgb(theme["background"]), rgb(theme["foreground"])
    ansi = [rgb(theme["color_%02d" % i]) for i in range(1, 17)]
    dark = luminance(base) < luminance(text)
    away = (0, 0, 0) if dark else text  # darker shades for sidebars and borders

    text = nudge(text, [base], MIN_TEXT, lighter=dark, limit=limit)
    if text is None:
        return None, "text contrast"
    if chroma(base) > MAX_BG_CHROMA:
        return None, "tinted background (chroma %.3f)" % chroma(base)
    lightness = oklab(base)[0]
    if dark and not DARK_BG[0] <= lightness <= DARK_BG[1]:
        return None, "window too dark or too light (%.2f)" % lightness
    if not dark and lightness < MIN_LIGHT_BG:
        return None, "light window too dim (%.2f)" % lightness

    surface = mix(base, text, 0.13)
    colors = {
        "base": base,
        "mantle": mix(base, away, 0.22 if dark else 0.05),
        "crust": mix(base, away, 0.4 if dark else 0.1),
        "surface": surface,
        "overlay": mix(base, text, 0.42),
        "text": text,
        "subtext": mix(base, text, 0.78),
    }
    colors["subtext"] = nudge(colors["subtext"], [base], MIN_SUBTEXT, lighter=dark, limit=limit)
    if colors["subtext"] is None:
        return None, "subtext contrast"

    # ANSI 1..6 are red, green, yellow, blue, magenta, cyan; +8 the bright ones.
    names = ["red", "green", "yellow", "blue", "purple", "teal"]
    for i, name in enumerate(names):
        normal, bright = ansi[1 + i], ansi[9 + i]
        pick = max((normal, bright), key=lambda c: contrast(base, c))
        pick = nudge(pick, [base], MIN_HUE, lighter=dark, limit=limit)
        pick = pick and nudge(pick, [surface], MIN_HUE_CARD, lighter=dark, limit=limit)
        if pick is None:
            return None, "%s contrast" % name
        if chroma(pick) < MIN_HUE_CHROMA:
            return None, "%s is grey" % name
        colors[name] = pick
    hues = sorted(hue(colors[n]) for n in names)
    gaps = [(b - a) for a, b in zip(hues, hues[1:] + [hues[0] + 360])]
    if min(gaps) < MIN_HUE_GAP:
        return None, "hues too alike (%.0f degrees)" % min(gaps)

    # Links, warnings and errors: libadwaita draws these hues as text.
    for name in ("red", "green", "yellow"):
        colors[name] = readable_as_text(colors[name], base, dark, limit)
        if colors[name] is None:
            return None, "%s as text" % name

    colors["orange"] = mix(colors["red"], colors["yellow"], 0.5)
    colors["pink"] = mix(colors["red"], colors["purple"], 0.5)
    colors["accent"] = colors["blue"]
    accent_fg = best_foreground(colors["accent"], [colors["crust"], base, text])
    if contrast(colors["accent"], accent_fg) < MIN_ACCENT_FG:
        colors["accent"] = colors["purple"]
        accent_fg = best_foreground(colors["accent"], [colors["crust"], base, text])
        if contrast(colors["accent"], accent_fg) < MIN_ACCENT_FG:
            return None, "no readable accent"
    if contrast(base, standalone(colors["accent"], dark)) < MIN_ROLE_TEXT:
        return None, "accent as text"
    return {"variant": "dark" if dark else "light", "colors": colors}, None


def signature(p):
    c = p["colors"]
    return [oklab(c[k]) for k in ("base", "text", "red", "green", "blue", "purple")]


def same(a, b, tolerance):
    return all(math.dist(x, y) < tolerance for x, y in zip(signature(a), signature(b)))


def family_of(name):
    """The theme's name without its light/dark word, to pair halves."""
    words = re.sub(r"[()]", " ", name).split()
    kept = [w for w in words if w.lower() not in ("dark", "light", "day", "night")]
    return " ".join(kept) or name


def main():
    verbose = "--verbose" in sys.argv
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    themes = json.loads(Path(args[0]).read_text(encoding="utf-8"))
    out = Path(args[1])

    # Light themes are scarce and pairs wanted, so only they may be nudged; a
    # lone dark theme must be readable as its author made it.
    variants = {}
    for theme in themes:
        variants.setdefault(family_of(theme["name"].strip()), set()).add(theme.get("variant"))

    kept, dropped = [], {}
    for theme in themes:
        name = theme["name"].strip()
        if any(b in name.lower() for b in BUNDLED):
            dropped[name] = "bundled already"
            continue
        # Gogh's leftover alternate versions.
        if re.search(r"\blost\b", name, re.I):
            dropped[name] = "alternate version"
            continue
        pairable = variants[family_of(name)] >= {"light", "dark"}
        limit = MAX_NUDGE if pairable or theme.get("variant") == "light" else 0.0
        palette, why = convert(theme, limit)
        if why:
            dropped[name] = why
            continue
        palette.update(name=name, author=theme.get("author_name") or "", family=family_of(name))
        kept.append(palette)

    # A family keeps its name only if it has both styles: a ready-made pair.
    by_family = {}
    for p in kept:
        by_family.setdefault(p["family"], []).append(p)
    for members in by_family.values():
        if {p["variant"] for p in members} != {"light", "dark"}:
            for p in members:
                p["family"] = p["name"]
    paired = lambda p: p["family"] != p["name"]

    # Near-identical themes: keep one, a pair's half over a lone theme. Lone
    # themes count as the same more readily, since Gogh has many near-twins.
    unique = []
    for p in sorted(kept, key=lambda p: not paired(p)):
        tolerance = SAME_THEME * (2 if paired(p) else 3)
        if any(same(p, u, tolerance) for u in unique):
            dropped[p["name"]] = "same as another"
        else:
            unique.append(p)
    kept = sorted(unique, key=lambda p: p["name"].lower())
    styles = {}
    for p in kept:
        styles.setdefault(p["family"], set()).add(p["variant"])
    pairs = sum(s == {"light", "dark"} for s in styles.values())

    out.mkdir(parents=True, exist_ok=True)
    for old in out.glob("*.toml"):
        old.unlink()
    for p in kept:
        pid = "gogh-" + slug(p["name"])
        lines = [
            "# Generated by scripts/import-gogh.py from Gogh (MIT), theme %s%s."
            % (json.dumps(p["name"]), " by " + p["author"] if p["author"] else ""),
            'id = "%s"' % pid,
            "name = %s" % json.dumps(p["name"]),
            "family = %s" % json.dumps(p["family"]),
            'variant = "%s"' % p["variant"],
            "",
            "[colors]",
        ]
        for key in ("base", "mantle", "crust", "surface", "overlay", "text", "subtext",
                    "accent", "red", "orange", "yellow", "green", "blue", "teal", "purple", "pink"):
            lines.append('%s = "%s"' % (key, hex_(p["colors"][key])))
        (out / (pid + ".toml")).write_text("\n".join(lines) + "\n", encoding="utf-8")

    light = sum(p["variant"] == "light" for p in kept)
    print("kept %d of %d (%d dark, %d light, %d light/dark pairs)"
          % (len(kept), len(themes), len(kept) - light, light, pairs))
    reasons = {}
    for why in dropped.values():
        key = why.split(" (")[0].split(" contrast")[0] + (" contrast" if "contrast" in why else "")
        reasons[key] = reasons.get(key, 0) + 1
    for why, n in sorted(reasons.items(), key=lambda x: -x[1]):
        print("  dropped %4d: %s" % (n, why))
    if verbose:
        for name, why in sorted(dropped.items()):
            print("%-40s %s" % (name, why))


if __name__ == "__main__":
    main()
