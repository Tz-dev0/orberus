# Draws the two graphics of the README into docs/images: the terminal and
# "where it runs". Run it from the root of the repository:
#
#   python3 scripts/readme-graphics.py
#
# The captures of the app come from app/scripts/snapshot.swift.
SANS = "-apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif"
MONO = "ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Consolas, 'Liberation Mono', monospace"
Q = "&quot;"

# Logos: Simple Icons (https://simpleicons.org), CC0, drawn in a 24x24 box.
# They are trademarks of their owners; the README says so.
LOGOS = {
    "claude": "m4.7144 15.9555 4.7174-2.6471.079-.2307-.079-.1275h-.2307l-.7893-.0486-2.6956-.0729-2.3375-.0971-2.2646-.1214-.5707-.1215-.5343-.7042.0546-.3522.4797-.3218.686.0608 1.5179.1032 2.2767.1578 1.6514.0972 2.4468.255h.3886l.0546-.1579-.1336-.0971-.1032-.0972L6.973 9.8356l-2.55-1.6879-1.3356-.9714-.7225-.4918-.3643-.4614-.1578-1.0078.6557-.7225.8803.0607.2246.0607.8925.686 1.9064 1.4754 2.4893 1.8336.3643.3035.1457-.1032.0182-.0728-.164-.2733-1.3539-2.4467-1.445-2.4893-.6435-1.032-.17-.6194c-.0607-.255-.1032-.4674-.1032-.7285L6.287.1335 6.6997 0l.9957.1336.419.3642.6192 1.4147 1.0018 2.2282 1.5543 3.0296.4553.8985.2429.8318.091.255h.1579v-.1457l.1275-1.706.2368-2.0947.2307-2.6957.0789-.7589.3764-.9107.7468-.4918.5828.2793.4797.686-.0668.4433-.2853 1.8517-.5586 2.9021-.3643 1.9429h.2125l.2429-.2429.9835-1.3053 1.6514-2.0643.7286-.8196.85-.9046.5464-.4311h1.0321l.759 1.1293-.34 1.1657-1.0625 1.3478-.8804 1.1414-1.2628 1.7-.7893 1.36.0729.1093.1882-.0183 2.8535-.607 1.5421-.2794 1.8396-.3157.8318.3886.091.3946-.3278.8075-1.967.4857-2.3072.4614-3.4364.8136-.0425.0304.0486.0607 1.5482.1457.6618.0364h1.621l3.0175.2247.7892.522.4736.6376-.079.4857-1.2142.6193-1.6393-.3886-3.825-.9107-1.3113-.3279h-.1822v.1093l1.0929 1.0686 2.0035 1.8092 2.5075 2.3314.1275.5768-.3218.4554-.34-.0486-2.2039-1.6575-.85-.7468-1.9246-1.621h-.1275v.17l.4432.6496 2.3436 3.5214.1214 1.0807-.17.3521-.6071.2125-.6679-.1214-1.3721-1.9246L14.38 17.959l-1.1414-1.9428-.1397.079-.674 7.2552-.3156.3703-.7286.2793-.6071-.4614-.3218-.7468.3218-1.4753.3886-1.9246.3157-1.53.2853-1.9004.17-.6314-.0121-.0425-.1397.0182-1.4328 1.9672-2.1796 2.9446-1.7243 1.8456-.4128.164-.7164-.3704.0667-.6618.4008-.5889 2.386-3.0357 1.4389-1.882.929-1.0868-.0062-.1579h-.0546l-6.3385 4.1164-1.1293.1457-.4857-.4554.0608-.7467.2307-.2429 1.9064-1.3114Z",
    "openai": "M22.2819 9.8211a5.9847 5.9847 0 0 0-.5157-4.9108 6.0462 6.0462 0 0 0-6.5098-2.9A6.0651 6.0651 0 0 0 4.9807 4.1818a5.9847 5.9847 0 0 0-3.9977 2.9 6.0462 6.0462 0 0 0 .7427 7.0966 5.98 5.98 0 0 0 .511 4.9107 6.051 6.051 0 0 0 6.5146 2.9001A5.9847 5.9847 0 0 0 13.2599 24a6.0557 6.0557 0 0 0 5.7718-4.2058 5.9894 5.9894 0 0 0 3.9977-2.9001 6.0557 6.0557 0 0 0-.7475-7.0729zm-9.022 12.6081a4.4755 4.4755 0 0 1-2.8764-1.0408l.1419-.0804 4.7783-2.7582a.7948.7948 0 0 0 .3927-.6813v-6.7369l2.02 1.1686a.071.071 0 0 1 .038.052v5.5826a4.504 4.504 0 0 1-4.4945 4.4944zm-9.6607-4.1254a4.4708 4.4708 0 0 1-.5346-3.0137l.142.0852 4.783 2.7582a.7712.7712 0 0 0 .7806 0l5.8428-3.3685v2.3324a.0804.0804 0 0 1-.0332.0615L9.74 19.9502a4.4992 4.4992 0 0 1-6.1408-1.6464zM2.3408 7.8956a4.485 4.485 0 0 1 2.3655-1.9728V11.6a.7664.7664 0 0 0 .3879.6765l5.8144 3.3543-2.0201 1.1685a.0757.0757 0 0 1-.071 0l-4.8303-2.7865A4.504 4.504 0 0 1 2.3408 7.872zm16.5963 3.8558L13.1038 8.364 15.1192 7.2a.0757.0757 0 0 1 .071 0l4.8303 2.7913a4.4944 4.4944 0 0 1-.6765 8.1042v-5.6772a.79.79 0 0 0-.407-.667zm2.0107-3.0231l-.142-.0852-4.7735-2.7818a.7759.7759 0 0 0-.7854 0L9.409 9.2297V6.8974a.0662.0662 0 0 1 .0284-.0615l4.8303-2.7866a4.4992 4.4992 0 0 1 6.6802 4.66zM8.3065 12.863l-2.02-1.1638a.0804.0804 0 0 1-.038-.0567V6.0742a4.4992 4.4992 0 0 1 7.3757-3.4537l-.142.0805L8.704 5.459a.7948.7948 0 0 0-.3927.6813zm1.0976-2.3654l2.602-1.4998 2.6069 1.4998v2.9994l-2.5974 1.4997-2.6067-1.4997Z",
    "apple": "M12.152 6.896c-.948 0-2.415-1.078-3.96-1.04-2.04.027-3.91 1.183-4.961 3.014-2.117 3.675-.546 9.103 1.519 12.09 1.013 1.454 2.208 3.09 3.792 3.039 1.52-.065 2.09-.987 3.935-.987 1.831 0 2.35.987 3.96.948 1.637-.026 2.676-1.48 3.676-2.948 1.156-1.688 1.636-3.325 1.662-3.415-.039-.013-3.182-1.221-3.22-4.857-.026-3.04 2.48-4.494 2.597-4.559-1.429-2.09-3.623-2.324-4.39-2.376-2-.156-3.675 1.09-4.61 1.09zM15.53 3.83c.843-1.012 1.4-2.427 1.245-3.83-1.207.052-2.662.805-3.532 1.818-.78.896-1.454 2.338-1.273 3.714 1.338.104 2.715-.688 3.559-1.701",
    "windows": "M0,0H11.377V11.372H0ZM12.623,0H24V11.372H12.623ZM0,12.623H11.377V24H0Zm12.623,0H24V24H12.623",
}

def platforms(theme):
    c = dict(text="#1b1b1d", muted="#6b6b70", line="#d9d9de", panel="#f6f6f8", faint="#a5a5ab") if theme == "light" else \
        dict(text="#f2f2f3", muted="#9a9aa0", line="#3a3a3f", panel="#1f1f22", faint="#6e6e74")
    W, H = 840, 244
    out = ['<svg xmlns="http://www.w3.org/2000/svg" width="%d" height="%d" viewBox="0 0 %d %d" role="img" '
           'aria-label="Works today: Claude Code and Claude Desktop (account switch, first version), on macOS. Coming next: Codex, Windows.">' % (W, H, W, H),
           '<rect x="0.5" y="0.5" width="%d" height="%d" rx="14" fill="%s" stroke="%s"/>' % (W - 1, H - 1, c["panel"], c["line"]),
           '<g font-family="%s">' % SANS]
    # Two columns of tiles work today, one is to come.
    today_x, today2_x, next_x = 112, 340, 608
    out.append('<text x="%d" y="38" font-size="13" font-weight="600" fill="%s">Works today</text>' % (today_x, c["muted"]))
    out.append('<text x="%d" y="38" font-size="13" font-weight="600" fill="%s">Coming next</text>' % (next_x, c["muted"]))
    out.append('<line x1="%d" y1="24" x2="%d" y2="%d" stroke="%s" stroke-dasharray="2 5" stroke-linecap="round"/>' % (next_x - 24, next_x - 24, H - 24, c["line"]))

    # Works today: the logo on a solid tile. Coming: a dashed, empty tile
    # with the logo dimmed, the way the app draws a profile nobody is
    # signed in to.
    def item(x, y, logo, today, name, note):
        if today:
            tile = '<rect x="%d" y="%d" width="44" height="44" rx="11" fill="%s"/>' % (x, y - 22, c["text"])
            ink = c["panel"]
        else:
            tile = ('<rect x="%.1f" y="%.1f" width="43" height="43" rx="10.5" fill="none" stroke="%s" stroke-width="1.5" stroke-dasharray="3 4"/>'
                    % (x + 0.5, y - 21.5, c["faint"]))
            ink = c["muted"]
        glyph = '<path transform="translate(%d %d)" fill="%s" d="%s"/>' % (x + 10, y - 12, ink, LOGOS[logo])
        return (tile + glyph +
                '<text x="%d" y="%d" font-size="16" font-weight="600" fill="%s">%s</text>' % (x + 58, y - 2, c["text"] if today else c["muted"], name) +
                '<text x="%d" y="%d" font-size="12.5" fill="%s">%s</text>' % (x + 58, y + 16, c["muted"], note))

    rows = [
        ("Tools", 88, [(today_x, "claude", True, "Claude Code", "accounts chosen by folder"),
                       (today2_x, "claude", True, "Claude Desktop", "account switch, first version"),
                       (next_x, "openai", False, "Codex", "same profiles and rules")]),
        ("Systems", 178, [(today_x, "apple", True, "macOS", "Apple Silicon and Intel"),
                          (next_x, "windows", False, "Windows", "the command builds there")]),
    ]
    for label, y, items in rows:
        out.append('<text x="32" y="%d" font-size="14" font-weight="600" fill="%s">%s</text>' % (y + 5, c["text"], label))
        for it in items:
            out.append(item(it[0], y, *it[1:]))
    out.append('<line x1="32" y1="133" x2="%d" y2="133" stroke="%s"/>' % (W - 32, c["line"]))
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
