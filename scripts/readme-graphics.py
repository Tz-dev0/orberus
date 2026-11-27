# Draws the two graphics of the README into docs/images: the terminal and
# "where it runs". Run it from the root of the repository:
#
#   python3 scripts/readme-graphics.py
#
# The captures of the app come from app/scripts/snapshot.swift.
SANS = "-apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif"
MONO = "ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Consolas, 'Liberation Mono', monospace"
Q = "&quot;"

def platforms(theme):
    c = dict(text="#1b1b1d", muted="#6b6b70", line="#d9d9de", panel="#f6f6f8") if theme == "light" else \
        dict(text="#f2f2f3", muted="#9a9aa0", line="#3a3a3f", panel="#1f1f22")
    W, H = 840, 232
    out = ['<svg xmlns="http://www.w3.org/2000/svg" width="%d" height="%d" viewBox="0 0 %d %d" role="img" '
           'aria-label="Works today: Claude Code, on macOS. Coming next: Codex, Claude Desktop profiles, Windows.">' % (W, H, W, H),
           '<rect x="0.5" y="0.5" width="%d" height="%d" rx="14" fill="%s" stroke="%s"/>' % (W - 1, H - 1, c["panel"], c["line"]),
           '<g font-family="%s">' % SANS]
    today_x, next_x = 130, 400
    out.append('<text x="%d" y="38" font-size="13" font-weight="600" fill="%s">Works today</text>' % (today_x, c["muted"]))
    out.append('<text x="%d" y="38" font-size="13" font-weight="600" fill="%s">Coming next</text>' % (next_x, c["muted"]))
    out.append('<line x1="%d" y1="24" x2="%d" y2="%d" stroke="%s" stroke-dasharray="2 5" stroke-linecap="round"/>' % (next_x - 28, next_x - 28, H - 24, c["line"]))

    def item(x, y, hue, filled, name, note):
        seal = ('<circle cx="%d" cy="%d" r="13" fill="%s"/>' % (x + 13, y, hue)) if filled else \
               ('<circle cx="%d" cy="%d" r="12" fill="none" stroke="%s" stroke-width="2"/>' % (x + 13, y, hue))
        return (seal +
                '<text x="%d" y="%d" font-size="16" font-weight="600" fill="%s">%s</text>' % (x + 38, y - 2, c["text"], name) +
                '<text x="%d" y="%d" font-size="12.5" fill="%s">%s</text>' % (x + 38, y + 16, c["muted"], note))

    rows = [
        ("Tools", 84, [(today_x, "#d99a3d", True, "Claude Code", "accounts chosen by folder"),
                       (next_x, "#2fa39b", False, "Codex", "same profiles and rules"),
                       (next_x + 210, "#d6648b", False, "Claude Desktop", "an account per profile")]),
        ("Systems", 168, [(today_x, "#6f83e6", True, "macOS", "Apple Silicon and Intel"),
                          (next_x, "#9a78d8", False, "Windows", "the command already builds there")]),
    ]
    for label, y, items in rows:
        out.append('<text x="32" y="%d" font-size="14" font-weight="600" fill="%s">%s</text>' % (y + 5, c["text"], label))
        for it in items:
            out.append(item(it[0], y, *it[1:]))
    out.append('<line x1="32" y1="126" x2="%d" y2="126" stroke="%s"/>' % (W - 32, c["line"]))
    out.append('</g></svg>')
    open("docs/images/platforms-%s.svg" % theme, "w").write("\n".join(out) + "\n")

def terminal():
    W, H = 840, 312
    bg, text, dim, line = "#1b1b1d", "#ececee", "#8e8e93", "#343438"
    work, perso, bad = "#7fc46c", "#8a9cf0", "#ff9d94"
    out = ['<svg xmlns="http://www.w3.org/2000/svg" width="%d" height="%d" viewBox="0 0 %d %d" role="img" '
           'aria-label="Typing claude in a company repository starts the work account; in a personal folder, the personal one; asking for the wrong account in a locked folder is refused.">' % (W, H, W, H),
           '<rect width="%d" height="%d" rx="14" fill="%s"/>' % (W, H, bg),
           '<g font-family="%s" font-size="14" xml:space="preserve">' % MONO]
    state = {"y": 44}
    def row(parts, gap=24):
        spans = "".join('<tspan fill="%s"%s>%s</tspan>' % (color, ' font-weight="600"' if bold else "", t) for t, color, bold in parts)
        out.append('<text x="32" y="%d">%s</text>' % (state["y"], spans))
        state["y"] += gap
    def rule():
        out.append('<line x1="32" y1="%d" x2="%d" y2="%d" stroke="%s"/>' % (state["y"] - 8, W - 32, state["y"] - 8, line))
        state["y"] += 22
    remote = "git_remote = " + Q + "github.com/acme/*" + Q
    row([("~/code/acme-api $ ", dim, False), ("claude", text, True)])
    row([("orberus: ", dim, False), ("work", work, True), (" (locked rule " + remote + " in config.toml)", dim, False)], 36)
    rule()
    row([("~/code/blog $ ", dim, False), ("claude", text, True)])
    row([("orberus: ", dim, False), ("perso", perso, True), (" (default profile)", dim, False)], 36)
    rule()
    row([("~/code/acme-api $ ", dim, False), ("ORBERUS_PROFILE=perso claude", text, True)])
    row([("orberus: ", dim, False), ("refused", bad, True), (": this folder is locked to profile 'work' by rule", text, False)])
    row([(remote + " in config.toml, so profile 'perso'", text, False)])
    row([("cannot be used here", text, False)])
    out.append('</g></svg>')
    open("docs/images/terminal.svg", "w").write("\n".join(out) + "\n")

platforms("light"); platforms("dark"); terminal()
