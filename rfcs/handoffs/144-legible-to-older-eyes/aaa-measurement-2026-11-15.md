# RFC 144 — the measurement behind the 2026-11-15 refresh

**Written 2026-11-15 by the architect**, for RFC 144's *Refresh, 2026-11-15:
toward the stricter standard*. Every figure there comes from the output
below.
**Method.** WCAG 2.x relative luminance and contrast ratio. Each colour is
measured against **both** page surfaces of its mode (`--surface-default` and
`--surface-elevated`), and the lower ratio is reported. An AAA candidate
keeps the colour's OKLab hue, and steps its lightness until both ratios
clear 7:1, lowering chroma only where sRGB's gamut forces it.
*Distinguishability* is the Euclidean distance in OKLab (ΔE) between two
colours. Token values are those on `7c35c42`.
**Run it.** `python3.14 -I` on the script below, with no dependencies.

## Output

```text
== light  (surfaces ['#F6F5F9', '#FFFFFF']; body text #1F1B24 = 15.60:1)
  accent   #6956C0  5.24:1  ->  AAA #5741AA  7.06:1  dL=-0.068
  danger   #BE3F3F  4.87:1  ->  AAA #A12127  7.00:1  dL=-0.084
  warning  #8A6500  4.91:1  ->  AAA #714D00  7.00:1  dL=-0.082
  success  #2F7D5E  4.59:1  ->  AAA #046043  7.01:1  dL=-0.099
  info     #3B6EA8  4.86:1  ->  AAA #22558E  7.01:1  dL=-0.085
  muted    #5F5A66  6.16:1  ->  AAA #56515D  7.09:1  dL=-0.032
  closest semantic pair now: dE=0.094 (accent/info);  at AAA: dE=0.095 (accent/info)
  semantic vs body text now: min dE=0.315;  at AAA: 0.229
== dark  (surfaces ['#1A1720', '#2A2433']; body text #F1EEF6 = 13.09:1)
  accent   #A89BFF  6.29:1  ->  AAA #B2A6FF  7.00:1  dL=+0.028
  danger   #FF6B6B  5.41:1  ->  AAA #FF938F  7.02:1  dL=+0.065
  warning  #E6B85C  8.13:1  ->  AAA #E6B85C  8.13:1  dL=+0.000
  success  #5FC49A  7.04:1  ->  AAA #5FC49A  7.04:1  dL=+0.000
  info     #6FA8FF  6.24:1  ->  AAA #7AB4FF  7.01:1  dL=+0.031
  muted    #B8B2C4  7.30:1  ->  AAA #B8B2C4  7.30:1  dL=+0.000
  closest semantic pair now: dE=0.074 (accent/info);  at AAA: dE=0.074 (accent/info)
  semantic vs body text now: min dE=0.197;  at AAA: 0.197

== text ON the colour (buttons), and non-text pairs
  light fg-on-accent #FFFFFF on accent     now  5.69:1   at AAA  7.66:1
  light fg-on-danger #FFFFFF on danger     now  5.29:1   at AAA  7.60:1
  light fg-on-warning #FFFFFF on warning   now  5.33:1   at AAA  7.60:1
  light fg-on-success #FFFFFF on success   now  4.98:1   at AAA  7.61:1
  light fg-on-info #FFFFFF on info         now  5.27:1   at AAA  7.61:1
  dark  fg-on-accent #1A1720 on accent     now  7.41:1   at AAA  8.25:1
  dark  fg-on-danger #1A1720 on danger     now  6.37:1   at AAA  8.27:1
  dark  fg-on-warning #1A1720 on warning   now  9.58:1   at AAA  9.58:1
  dark  fg-on-success #1A1720 on success   now  8.30:1   at AAA  8.30:1
  dark  fg-on-info #1A1720 on info         now  7.34:1   at AAA  8.25:1
  light border-default #D6D1DE: 1.38:1 (non-text needs 3:1)   focus ring #6956C0: 5.24:1
        control border at 3:1 -> #918C98 3.02:1;  at 4.5:1 (prefers-contrast) -> #736F7A
  dark  border-default #3A3445: 1.26:1 (non-text needs 3:1)   focus ring #A89BFF: 6.29:1
        control border at 3:1 -> #736D80 3.02:1;  at 4.5:1 (prefers-contrast) -> #908A9E

== the proposed set: AAA where colour is text or a fill (accent, danger, muted); others unchanged
  light: three closest pairs accent/info 0.119, warning/success 0.130, success/info 0.140
    warning as non-text on surfaces: 4.91:1 (needs 3:1)
    success as non-text on surfaces: 4.59:1 (needs 3:1)
    info as non-text on surfaces: 4.86:1 (needs 3:1)
  dark: three closest pairs accent/info 0.082, danger/warning 0.130, warning/success 0.164
    warning as non-text on surfaces: 8.13:1 (needs 3:1)
    success as non-text on surfaces: 7.04:1 (needs 3:1)
    info as non-text on surfaces: 6.24:1 (needs 3:1)
```

## The script

```python
# RFC 144 review: AA vs AAA for the semantic colours, hue-preserving.
import math, sys
def hx(h): h=h.lstrip('#'); return tuple(int(h[i:i+2],16)/255 for i in (0,2,4))
def lin(c): return c/12.92 if c<=0.04045 else ((c+0.055)/1.055)**2.4
def unlin(c): return 12.92*c if c<=0.0031308 else 1.055*c**(1/2.4)-0.055
def L(rgb): r,g,b=map(lin,rgb); return 0.2126*r+0.7152*g+0.0722*b
def cr(a,b): la,lb=L(hx(a)) if isinstance(a,str) else L(a),L(hx(b)) if isinstance(b,str) else L(b); hi,lo=max(la,lb),min(la,lb); return (hi+0.05)/(lo+0.05)
def to_oklab(rgb):
    r,g,b=map(lin,rgb)
    l=0.4122214708*r+0.5363325363*g+0.0514459929*b; m=0.2119034982*r+0.6806995451*g+0.1073969566*b; s=0.0883024619*r+0.2817188376*g+0.6299787005*b
    l,m,s=[x**(1/3) for x in (l,m,s)]
    return (0.2104542553*l+0.7936177850*m-0.0040720468*s, 1.9779984951*l-2.4285922050*m+0.4505937099*s, 0.0259040371*l+0.7827717662*m-0.8086757660*s)
def from_oklab(lab):
    L_,a,b=lab
    l=(L_+0.3963377774*a+0.2158037573*b)**3; m=(L_-0.1055613458*a-0.0638541728*b)**3; s=(L_-0.0894841775*a-1.2914855480*b)**3
    r=4.0767416621*l-3.3077115913*m+0.2309699292*s; g=-1.2684380046*l+2.6097574011*m-0.3413193965*s; bb=-0.0041960863*l-0.7034186147*m+1.7076147010*s
    return tuple(min(1,max(0,unlin(x))) for x in (r,g,bb))
def tohex(rgb): return '#'+''.join(f'{round(c*255):02X}' for c in rgb)
def lch(h):
    L_,a,b=to_oklab(hx(h)); return L_, math.hypot(a,b), math.atan2(b,a)
def with_L(h,newL):
    _,C,H=lch(h)
    # reduce chroma until in gamut
    for k in range(100):
        c=C*(1-k/100); rgb=from_oklab((newL,c*math.cos(H),c*math.sin(H)))
        back=to_oklab(rgb)
        if abs(back[0]-newL)<0.01: return tohex(rgb)
    return tohex(from_oklab((newL,0,0)))
def reach(h, bgs, target, darker):
    L0,_,_=lch(h); step=-0.002 if darker else 0.002; Lc=L0
    for _ in range(600):
        c=with_L(h,Lc)
        if all(cr(c,bg)>=target for bg in bgs): return c
        Lc+=step
    return None
def dE(a,b):
    x=to_oklab(hx(a)); y=to_oklab(hx(b)); return math.dist(x,y)
modes={
 'light':dict(bgs=['#F6F5F9','#FFFFFF'], darker=True,
   col={'accent':'#6956C0','danger':'#BE3F3F','warning':'#8A6500','success':'#2F7D5E','info':'#3B6EA8','muted':'#5F5A66'}, fg='#1F1B24'),
 'dark':dict(bgs=['#1A1720','#2A2433'], darker=False,
   col={'accent':'#A89BFF','danger':'#FF6B6B','warning':'#E6B85C','success':'#5FC49A','info':'#6FA8FF','muted':'#B8B2C4'}, fg='#F1EEF6'),
}
for mode,m in modes.items():
    print(f"== {mode}  (surfaces {m['bgs']}; body text {m['fg']} = {min(cr(m['fg'],b) for b in m['bgs']):.2f}:1)")
    aaa={}
    for k,v in m['col'].items():
        now=min(cr(v,b) for b in m['bgs']); a=reach(v,m['bgs'],7.0,m['darker']); aaa[k]=a
        print(f"  {k:8s} {v} {now:5.2f}:1  ->  AAA {a} {min(cr(a,b) for b in m['bgs']):5.2f}:1  dL={lch(a)[0]-lch(v)[0]:+.3f}")
    keys=[k for k in m['col'] if k!='muted']
    def mind(cols):
        best=(9,None)
        for i in range(len(keys)):
            for j in range(i+1,len(keys)):
                d=dE(cols[keys[i]],cols[keys[j]]); best=min(best,(d,f"{keys[i]}/{keys[j]}"))
        return best
    print(f"  closest semantic pair now: dE={mind(m['col'])[0]:.3f} ({mind(m['col'])[1]});  at AAA: dE={mind(aaa)[0]:.3f} ({mind(aaa)[1]})")
    print(f"  semantic vs body text now: min dE={min(dE(m['col'][k],m['fg']) for k in keys):.3f};  at AAA: {min(dE(aaa[k],m['fg']) for k in keys):.3f}")

print("\n== text ON the colour (buttons), and non-text pairs")
pairs={
 'light':[('fg-on-accent #FFFFFF on accent','#FFFFFF','#6956C0','#5741AA'),('fg-on-danger #FFFFFF on danger','#FFFFFF','#BE3F3F','#A12127'),
          ('fg-on-warning #FFFFFF on warning','#FFFFFF','#8A6500','#714D00'),('fg-on-success #FFFFFF on success','#FFFFFF','#2F7D5E','#046043'),('fg-on-info #FFFFFF on info','#FFFFFF','#3B6EA8','#22558E')],
 'dark':[('fg-on-accent #1A1720 on accent','#1A1720','#A89BFF','#B2A6FF'),('fg-on-danger #1A1720 on danger','#1A1720','#FF6B6B','#FF938F'),
         ('fg-on-warning #1A1720 on warning','#1A1720','#E6B85C','#E6B85C'),('fg-on-success #1A1720 on success','#1A1720','#5FC49A','#5FC49A'),('fg-on-info #1A1720 on info','#1A1720','#6FA8FF','#7AB4FF')]}
for mode,ps in pairs.items():
    for name,fg,now,aaa in ps: print(f"  {mode:5s} {name:34s} now {cr(fg,now):5.2f}:1   at AAA {cr(fg,aaa):5.2f}:1")
for mode,(bd,bgs,focus) in {'light':('#D6D1DE',['#F6F5F9','#FFFFFF'],'#6956C0'),'dark':('#3A3445',['#1A1720','#2A2433'],'#A89BFF')}.items():
    print(f"  {mode:5s} border-default {bd}: {min(cr(bd,b) for b in bgs):.2f}:1 (non-text needs 3:1)   focus ring {focus}: {min(cr(focus,b) for b in bgs):.2f}:1")
    c=reach(bd,bgs,3.0, mode=='light'); print(f"        control border at 3:1 -> {c} {min(cr(c,b) for b in bgs):.2f}:1;  at 4.5:1 (prefers-contrast) -> {reach(bd,bgs,4.5,mode=='light')}")

print("\n== the proposed set: AAA where colour is text or a fill (accent, danger, muted); others unchanged")
prop={'light':{'accent':'#5741AA','danger':'#A12127','warning':'#8A6500','success':'#2F7D5E','info':'#3B6EA8'},
      'dark':{'accent':'#B2A6FF','danger':'#FF938F','warning':'#E6B85C','success':'#5FC49A','info':'#6FA8FF'}}
for mode,cols in prop.items():
    ks=list(cols); out=[]
    for i in range(len(ks)):
        for j in range(i+1,len(ks)): out.append((dE(cols[ks[i]],cols[ks[j]]),f"{ks[i]}/{ks[j]}"))
    out.sort(); print(f"  {mode}: three closest pairs " + ", ".join(f"{n} {d:.3f}" for d,n in out[:3]))
    for k in ('warning','success','info'):
        bgs=modes[mode]['bgs']; print(f"    {k} as non-text on surfaces: {min(cr(cols[k],b) for b in bgs):.2f}:1 (needs 3:1)")
```
