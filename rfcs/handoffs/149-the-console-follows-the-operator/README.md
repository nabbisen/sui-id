# RFC 149 — handoff

**RFC.** [`../../accepted/149-the-console-follows-the-operator.md`](../../accepted/149-the-console-follows-the-operator.md)
**Status.** **Accepted 2026-11-15** ([design review record](design-review-2026-11-15.md)). Not yet dispatched: its build is cycle P's, authorized 2026-11-15.

## 2026-11-15: the mockups' questions answered

By the architect, at the owner's request:
[`design-review-2026-11-15.md`](design-review-2026-11-15.md). The answers are
the grouped sidebar, palette B (light warning raised to `#9D6600`), and
custom scopes kept. **The RFC was accepted 2026-11-15.**

## Carried into this RFC's build, 2026-11-16

- **`client.create` (C01) should record the consent policy chosen at
  creation.** RFC 147 (`0.92.0`'s N1) let the create form set a policy, but
  C01 records no attributes (`crates/sui-id-store/src/commands.rs:6314-6318`),
  while changing the policy later is recorded (`:6135`). This RFC rebuilds the
  app form, so C01 gains the policy as an attribute here. It is an audit-event
  schema change, under RFC 094's contract: its registry, the coverage matrix,
  and the event reference.

- **From `0.92.0`'s N2b review (2026-11-17):**
  - the top bar's links now **wrap** at large font sizes and on phones, an
    interim until this RFC's sidebar replaces the top bar;
  - `.kv-list` has no CSS: the user detail and recovery-link pages render
    `<dl>` in the browser's default style. Style it with the detail pages'
    layout, or move them to the key-value table the settings pages use;
  - `me_security/security.rs`'s `page_get` is dead (no route reaches it):
    delete it when the `/me` pages are rebuilt.

- **From the M4-B Google run (2026-11-22)**
  ([record](../096-upstream-oidc-federation/096-b1-m4b-google-run-record-2026-11-22.md)):
  - **the footer is noise on every page.** `@nabbisen`: *"there is
    improvement around common footer which always shows static sentences
    and color theme selector. They can be noise."* Asked when, he chose:
    *"Cycle P, with the chrome redesign (Recommended)"*.
    `sui-id-web/src/layout.rs:270-304` carries:
    - a tagline;
    - three non-interactive accessibility self-claims;
    - the theme toggle;
    - the version.

    Design it with the sidebar and the account menu, once. The likely
    shape:
    - keep the version;
    - move the theme selector into the account menu beside Language;
    - drop the tagline and the self-claims, which belong in the docs if
      anywhere;
  - **`fed_cause_dns_denied` embeds the deny table's English reason**,
    sometimes long (*"IPv4-compatible Address -- RFC 4291 S2.5.5.1 …"*),
    into Japanese and Chinese pages. Give each denied class a short
    localized label.

## The mockups

Open each file in a browser, straight from disk. They are static: no
script, and no network.

| File | Shows | The RFC's question |
|---|---|---|
| [`mockups/navigation.html`](mockups/navigation.html) | §1's six groups as a **sidebar** (recommended) and as a **top bar**; the account menu set apart; §2's first-run path on the dashboard | §8 question 1 |
| [`mockups/palettes.html`](mockups/palettes.html) | §6's two palette directions, **A** (refined current) and **B** (calm slate), each in light and dark, on the same components | §8 question 2 |
| [`mockups/app-form.html`](mockups/app-form.html) | §3's help layers, and §4's scope checkboxes, additional scopes, and the redirect-URI list with a per-URI error | §8 question 3 |

## The two palettes, and how they were derived

**Both satisfy RFC 144's five rules**: every text pair is at least 7:1
against both page surfaces of its mode, every boundary at least 3:1. **A**
is RFC 144's refreshed values, from
[`../144-legible-to-older-eyes/aaa-measurement-2026-11-15.md`](../144-legible-to-older-eyes/aaa-measurement-2026-11-15.md).
**B** is derived below, in the same way, from chosen seed hues.

**B's info colour was chosen for distinctness, not taste.** With an indigo
accent, B's first info (teal) sat ΔE 0.078 from success in light mode, too
close. Of six candidate hues, the one kept leaves B's closest pair of
semantic colours at **0.120** (light) and **0.109** (dark). A's are 0.119
and 0.082.

**Tinted surfaces** (badges and banners, at most 16% of the colour over the
card) keep body text at **9.26:1 or more**, in both directions and both
modes.

The text inputs in the mockups are the computed values. Nothing was picked
by eye.

### B's derivation: output

```text
== B light: body #1A1D23 15.75:1
   accent   #3949B2  7.07:1, label on fill 7.58
   danger   #A51A22  7.07:1, label on fill 7.58
   warning  #B07800  3.54:1
   success  #2E7D52  4.70:1
   info     #1E7F86  4.42:1
   muted    #4D5563  7.01:1
   closest: success/info 0.078, warning/success 0.173, accent/info 0.182
   control border #8A8F96 3.04:1
== B dark: body #EEF1F5 13.93:1
   accent   #98A8FF  7.05:1, label on fill 8.02
   danger   #FF8C8B  7.05:1, label on fill 8.02
   warning  #E8B54A  8.37:1
   success  #5CC48C  7.31:1
   info     #4FC3CA  7.51:1
   muted    #AAB2BF  7.39:1
   closest: success/info 0.089, accent/info 0.140, danger/warning 0.145
   control border #666D79 3.03:1

== B: choosing info's hue to maximise the closest semantic pair
   light: info #2F77C0 -> closest pair accent/info 0.120; non-text 4.33:1
   light: info #136F9B -> closest pair accent/info 0.116; non-text 5.19:1
   light: info #1F6FA0 -> closest pair accent/info 0.111; non-text 5.10:1
   dark: info #67C8E8 -> closest pair accent/info 0.109; non-text 8.27:1
   dark: info #38BDF8 -> closest pair accent/info 0.097; non-text 7.37:1
   dark: info #4DA8E0 -> closest pair accent/info 0.094; non-text 6.01:1
```

### B's derivation: script

It needs `aaa-measurement-2026-11-15.md`'s script beside it, as `aaa-palette.py`, for the shared functions.

```python
import importlib.util, sys
spec=importlib.util.spec_from_file_location("p", "aaa-palette.py")
src=open("aaa-palette.py").read().split('modes={')[0]
exec(src)
B={'light':dict(bgs=['#F6F7F9','#FFFFFF'],fg='#1A1D23',darker=True,
     seed={'accent':'#3D4FB8','danger':'#C23A3A','warning':'#B07800','success':'#2E7D52','info':'#1E7F86','muted':'#5A6270'}),
   'dark':dict(bgs=['#14171C','#1E232B'],fg='#EEF1F5',darker=False,
     seed={'accent':'#8C9BFF','danger':'#FF7A7A','warning':'#E8B54A','success':'#5CC48C','info':'#4FC3CA','muted':'#AAB2BF'})}
for mode,m in B.items():
    out={}
    for k,v in m['seed'].items():
        need = 7.0 if k in ('accent','danger','muted') else 3.0
        c=reach(v,m['bgs'],need,m['darker']) if min(cr(v,b) for b in m['bgs'])<need else v
        out[k]=c
    onfg='#FFFFFF' if mode=='light' else m['bgs'][0]
    print(f"== B {mode}: body {m['fg']} {min(cr(m['fg'],b) for b in m['bgs']):.2f}:1")
    for k,c in out.items():
        extra = f", label on fill {cr(onfg,c):.2f}" if k in ('accent','danger') else ""
        print(f"   {k:8s} {c}  {min(cr(c,b) for b in m['bgs']):.2f}:1{extra}")
    ks=[k for k in out if k!='muted']; pr=sorted((dE(out[a],out[b]),f"{a}/{b}") for i,a in enumerate(ks) for b in ks[i+1:])
    print("   closest:", ", ".join(f"{n} {d:.3f}" for d,n in pr[:3]))
    bd=reach('#D3D8E0' if mode=='light' else '#353B46', m['bgs'],3.0,m['darker']); print(f"   control border {bd} {min(cr(bd,b) for b in m['bgs']):.2f}:1")

print("\n== B: choosing info's hue to maximise the closest semantic pair")
for mode,m in B.items():
    base={k:v for k,v in m['seed'].items() if k!='muted'}
    base['accent']=reach(base['accent'],m['bgs'],7.0,m['darker']); base['danger']=reach(base['danger'],m['bgs'],7.0,m['darker'])
    cands=(['#1E7F86','#1F6FA0','#2B6CB0','#2F77C0','#136F9B','#0E7490'] if mode=='light' else ['#4FC3CA','#5AB0F0','#62B4FF','#4DA8E0','#38BDF8','#67C8E8'])
    res=[]
    for ic in cands:
        cols=dict(base,info=ic); ks=list(cols)
        mn=min((dE(cols[a],cols[b]),f"{a}/{b}") for i,a in enumerate(ks) for b in ks[i+1:])
        res.append((mn[0],ic,mn[1],min(cr(ic,b) for b in m['bgs'])))
    for d,ic,pair,c in sorted(res,reverse=True)[:3]: print(f"   {mode}: info {ic} -> closest pair {pair} {d:.3f}; non-text {c:.2f}:1")
```
