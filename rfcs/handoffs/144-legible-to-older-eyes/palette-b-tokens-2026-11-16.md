# RFC 144 — palette B's complete token set, as built in `0.92.0`

**Written 2026-11-16 by the architect**, for `0.92.0`'s N2. RFC 144 is built
with palette **B** (RFC 149 §6; decided 2026-11-16). RFC 149's derivation gave
B's main colours, checked against the two page surfaces. **This file gives
every colour token in `tokens.rs`, in both modes**, and checks it against
**every surface and every tint a component composes text on**, the standard
RFC 144's closure sets (*"every … pair a component actually composes"*).

**The rules applied** (RFC 144, *Refresh*, R):
- body and secondary text: at least **7:1** on all four surfaces and all five
  tints;
- **accent** as text: 7:1 on all four surfaces **and on its own tint**,
  because components compose it there: the current navigation item
  (`components/chrome.rs:150-152`), the pressed theme toggle (`:379-381`), the
  active language (`components/setup.rs:107-109`) and `.badge--accent`
  (`components/badges.rs:40`);
- **danger** as text: 7:1 on all four surfaces, and **never on its own
  tint**. RFC 144 §1a moves every tinted danger label to `--fg-default`, the
  banner convention;
- button labels: 7:1 on their fills, hover included; the inverse pair: 7:1;
- control borders, the focus ring, the status colours and `fg-subtle`, all
  non-text: at least **3:1** on every surface.

## How these values differ from RFC 149's, and why

| Mode | Token | RFC 149 recorded | Built | OKLab ΔE | Forced by |
|---|---|---|---|---|---|
| light | accent | `#3949B2` | `#3342AA` | 0.023 | accent text on its tint (the current nav item) |
| light | danger | `#A51A22` | `#9D0D1B` | 0.023 | danger text on `--surface-sunken` |
| dark | accent | `#98A8FF` | `#AABBFF` | 0.057 | accent text on its tint |
| dark | danger | `#FF8C8B` | `#FF8C8B` | 0 | — |

**The tints are 10%** of their colour over the card surface, within RFC
149's *"at most 16%"*. At 12%, secondary text (`fg-muted`) fell below 7:1 on
the tints (lowest 5.74:1, dark). `fg-muted` is computed to clear 7:1 on every
surface **and every tint**, which raised dark `fg-muted` to `#B8C1CE`.

The hierarchy holds: body text against the page is 15.75:1 (light) and
15.86:1 (dark); secondary text is 7.82:1 and 9.89:1.

**For `prefers-contrast: more`**, the control border becomes `#666A72` (light,
4.54:1) and `#838A94` (dark, 4.53:1).

## Three tokens added

- **`--danger-emphasis`**, the danger button's hover fill: light `#870009`,
  dark `#FFA7A5`, each clearing 7:1 under its label. Today the hover is
  `filter: brightness(0.92)` (`components/buttons.rs:62`), a fill no gate can
  resolve.
- **`--devmode-bg`** `#5A3D00`, with **`--devmode-fg`** `#FFF8E1` (9.40:1)
  and **`--devmode-warn`** `#FFCDD2` (7.09:1), the same in both modes. Today
  the dev-mode banner sets them as literals (`components/banners.rs:97-107`):
  `#7A5C00` under them gives 5.89:1 and 4.44:1. The new background stays a
  deep amber, ΔE 0.111 from the old one, and distinct from every semantic
  colour.

## A live defect this measurement found

**The danger button's label is hard-coded white** (`components/buttons.rs:60`,
`color: #FFFFFF`). Dark mode's danger fill is light, so every destructive
button in dark mode is **2.78:1** today (`#FFFFFF` on `#FF6B6B`). The label
must be `var(--fg-on-danger)`, which is dark in dark mode.

**Not colour, unchanged:** `--state-hover`, `--state-active` and
`--state-disabled` (translucent), the shadows, and every non-colour token.

## The values, and the check: output

```text
== B light: 32 tokens
   --accent-default: #3342AA;
   --accent-emphasis: #262F96;
   --accent-subtle: #EBEDF7;
   --bg-disabled: #EEF0F3;
   --border-accent: #3342AA;
   --border-default: #83878F;
   --border-muted: #E2E5E9;
   --danger-default: #9D0D1B;
   --danger-emphasis: #870009;
   --danger-subtle: #F6E8E9;
   --fg-default: #1A1D23;
   --fg-disabled: #464E5C;
   --fg-inverse: #F6F7F9;
   --fg-muted: #464E5C;
   --fg-on-accent: #FFFFFF;
   --fg-on-danger: #FFFFFF;
   --fg-on-info: #FFFFFF;
   --fg-on-success: #FFFFFF;
   --fg-on-warning: #FFFFFF;
   --fg-subtle: #818791;
   --info-default: #2F77C0;
   --info-subtle: #EAF1F9;
   --state-focus: #3342AA;
   --success-default: #2E7D52;
   --success-subtle: #EAF2EE;
   --surface-default: #F6F7F9;
   --surface-elevated: #FFFFFF;
   --surface-inverse: #23272E;
   --surface-subtle: #EEF0F3;
   --surface-sunken: #E8EBEF;
   --warning-default: #9D6600;
   --warning-subtle: #F5F0E6;
   58 pairs checked; below threshold: 0; tightest: danger-default on #E8EBEF 7.00:1 (needs 7.0)
== B dark: 32 tokens
   --accent-default: #AABBFF;
   --accent-emphasis: #BDCEFF;
   --accent-subtle: #2A3040;
   --bg-disabled: #191D23;
   --border-accent: #AABBFF;
   --border-default: #666D77;
   --border-muted: #262B33;
   --danger-default: #FF8C8B;
   --danger-emphasis: #FFA7A5;
   --danger-subtle: #342E35;
   --fg-default: #EEF1F5;
   --fg-disabled: #B8C1CE;
   --fg-inverse: #1A1D23;
   --fg-muted: #B8C1CE;
   --fg-on-accent: #14171C;
   --fg-on-danger: #14171C;
   --fg-on-info: #14171C;
   --fg-on-success: #14171C;
   --fg-on-warning: #14171C;
   --fg-subtle: #666D78;
   --info-default: #67C8E8;
   --info-subtle: #25343E;
   --state-focus: #AABBFF;
   --success-default: #5CC48C;
   --success-subtle: #243335;
   --surface-default: #14171C;
   --surface-elevated: #1E232B;
   --surface-inverse: #EEF1F5;
   --surface-subtle: #191D23;
   --surface-sunken: #101317;
   --warning-default: #E8B54A;
   --warning-subtle: #32322E;
   58 pairs checked; below threshold: 0; tightest: danger-default on #1E232B 7.05:1 (needs 7.0)
ALL PASS
light accent: RFC 149 recorded #3949B2, built #3342AA, OKLab dE 0.023
light danger: RFC 149 recorded #A51A22, built #9D0D1B, OKLab dE 0.023
dark accent: RFC 149 recorded #98A8FF, built #AABBFF, OKLab dE 0.057
dark danger: RFC 149 recorded #FF8C8B, built #FF8C8B, OKLab dE 0.000
light: hierarchy on the page — body 15.75:1, muted 7.82:1, body vs muted dE 0.192
dark: hierarchy on the page — body 15.86:1, muted 9.89:1, body vs muted dE 0.150
```

## The script

It needs RFC 144's `aaa-measurement-2026-11-15.md` script beside it, as `aaa-palette.py`.

```python
# Palette B, every token of tokens.rs, derived from B's seeds, then every
# composed pair checked against RFC 144's rules (text >= 7:1, non-text >= 3:1).
exec(open("aaa-palette.py").read().split('modes={')[0])
def mixhex(c, base, p):
    a, b = hx(c), hx(base); return tohex(tuple(p*x + (1-p)*y for x, y in zip(a, b)))
def shiftL(c, d):
    return with_L(c, lch(c)[0] + d)
def full(mode):
    light = mode == 'light'
    S = (dict(default='#F6F7F9', elevated='#FFFFFF', subtle='#EEF0F3', sunken='#E8EBEF', inverse='#23272E') if light
         else dict(default='#14171C', elevated='#1E232B', subtle='#191D23', sunken='#101317', inverse='#EEF1F5'))
    fg = '#1A1D23' if light else '#EEF1F5'
    seeds = (dict(accent='#3949B2', danger='#A51A22', warning='#9D6600', success='#2E7D52', info='#2F77C0') if light
             else dict(accent='#98A8FF', danger='#FF8C8B', warning='#E8B54A', success='#5CC48C', info='#67C8E8'))
    surf = [S['default'], S['elevated'], S['subtle'], S['sunken']]
    T = {f'surface-{k}': v for k, v in S.items()}
    T['fg-default'] = fg
    T['fg-subtle'] = reach('#8E949E' if light else '#5B626D', surf, 3.0, light)
    T['fg-inverse'] = S['default'] if light else '#1A1D23'
    on = '#FFFFFF' if light else S['default']
    for k, v in seeds.items():
        tint = mixhex(v, S['elevated'], 0.10)
        T[f'{k}-subtle'] = tint
        if k == 'accent':                       # text on every surface and on its own tint (nav current item, toggles, .badge--accent)
            T[f'{k}-default'] = reach(v, surf + [tint], 7.0, light)
        elif k == 'danger':                     # text on every surface; never on its own tint (RFC 144 sec. 1a: tinted -> fg-default)
            T[f'{k}-default'] = reach(v, surf, 7.0, light)
        else:                                   # non-text only (RFC 144 rule 3): 3:1
            T[f'{k}-default'] = reach(v, surf, 3.0, light)
        T[f'fg-on-{k}'] = on
    tints = [T[f'{k}-subtle'] for k in seeds]
    T['fg-muted'] = reach('#4D5563' if light else '#AAB2BF', surf + tints, 7.0, light)
    T['accent-emphasis'] = shiftL(T['accent-default'], -0.06 if light else +0.06)
    T['danger-emphasis'] = shiftL(T['danger-default'], -0.06 if light else +0.06)
    T['fg-disabled'] = T['fg-muted']; T['bg-disabled'] = S['subtle']
    T['state-focus'] = T['accent-default']; T['border-accent'] = T['accent-default']
    T['border-default'] = reach('#A9AEB6' if light else '#4A505A', surf, 3.0, light)
    T['border-muted'] = '#E2E5E9' if light else '#262B33'
    return T, surf
fails = 0
for mode in ('light', 'dark'):
    T, surf = full(mode)
    print(f"== B {mode}: {len(T)} tokens")
    for k in sorted(T): print(f"   --{k}: {T[k]};")
    bgs = surf + [T[f'{k}-subtle'] for k in ('accent','danger','warning','success','info')] + [T['bg-disabled']]
    checks = []
    for t in ('fg-default', 'fg-muted'):
        for b in bgs: checks.append((f"{t} on {b}", T[t], b, 7.0))
    for b in surf + [T['accent-subtle']]: checks.append((f"accent-default on {b}", T['accent-default'], b, 7.0))
    for b in surf: checks.append((f"danger-default on {b}", T['danger-default'], b, 7.0))
    for f in ('accent-default', 'accent-emphasis', 'danger-default', 'danger-emphasis'):
        checks.append((f"label on {f}", T['fg-on-accent'], T[f], 7.0))
    checks.append(("fg-inverse on surface-inverse", T['fg-inverse'], T['surface-inverse'], 7.0))
    for t in ('border-default', 'state-focus', 'warning-default', 'success-default', 'info-default', 'fg-subtle'):
        for b in surf: checks.append((f"{t} (non-text) on {b}", T[t], b, 3.0))
    bad = [(n, cr(a, b), need) for n, a, b, need in checks if cr(a, b) < need]
    worst = min(checks, key=lambda c: cr(c[1], c[2]) / c[3])
    print(f"   {len(checks)} pairs checked; below threshold: {len(bad)}; tightest: {worst[0]} {cr(worst[1], worst[2]):.2f}:1 (needs {worst[3]})")
    for n, r, need in bad: print(f"   FAIL {n}: {r:.2f} < {need}")
    fails += len(bad)
print("ALL PASS" if fails == 0 else f"{fails} FAILURES")
B149={'light':dict(accent='#3949B2',danger='#A51A22'),'dark':dict(accent='#98A8FF',danger='#FF8C8B')}
for mode in ('light','dark'):
    T,_=full(mode)
    for k in ('accent','danger'):
        print(f"{mode} {k}: RFC 149 recorded {B149[mode][k]}, built {T[k+'-default']}, OKLab dE {dE(B149[mode][k],T[k+'-default']):.3f}")
for mode in ('light','dark'):
    T,_=full(mode)
    print(f"{mode}: hierarchy on the page — body {cr(T['fg-default'],T['surface-default']):.2f}:1, muted {cr(T['fg-muted'],T['surface-default']):.2f}:1, body vs muted dE {dE(T['fg-default'],T['fg-muted']):.3f}")
```
