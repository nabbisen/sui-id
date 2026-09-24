# RFC 117 — independent design review

**Date:** 2026-09-24
**RFC:** [RFC 117 — Owner decisions must be verifiable](../../accepted/117-verifiable-owner-decisions.md)
**Request:** [`design-review-request.md`](design-review-request.md)
**Reviewer:** Mid-capability model, implementation role. Authored neither the RFC nor its handoff.
**Baseline read:** `11d28a4` (contains `8741fa6`). Read-only: no code, no RFC text, no `ci/` file changed. Everything I ran was in the session scratchpad or was a read (`git`, `gh api` GETs, `man`, `ssh-keygen` on throwaway keys). **I generated no key in `~/.ssh`, signed nothing with any existing key, and made no change on GitHub.**
**Outcome: build with the changes named in §9. The objection is correct and, measured, stronger than the RFC states it. It does not sink the scheme; it changes what the scheme may claim, and it exposes two design errors in D3/D6 and one unverifiable property in D1.**

---

## 0. Items 1–3 first: the objection

### Item 1 — is it correct?

**Yes, and it is worse on this machine than the RFC says**, because "everything in the repository is writable by whoever writes the repository" understates the reach. Measured:

| Fact | Command | Result |
|---|---|---|
| Agents commit with the owner's signing key, unprompted | `git config --get commit.gpgsign`; `git config --get user.signingkey`; `git log --format='%h %G? %ae' -10` | `true`; `25757DA6…`; ten of ten commits `U` (good signature, unknown validity), all under the owner's noreply address |
| Agents push with the owner's GitHub identity | `git remote -v`; `git config --get-regexp '^credential'`; `gh auth status` | `https://github.com/nabbisen/sui-id.git`; helper `!/usr/bin/gh auth git-credential`; logged in as `nabbisen`, scopes **`repo`, `workflow`**, `read:org`, `gist` |
| That identity administers the repository | `gh api repos/nabbisen/sui-id --jq '{admin:.permissions.admin,push:.permissions.push,owner_type:.owner.type,private:.private}'` | `admin: true`, `push: true`, **`owner_type: User`** (a personal account, not an organisation), `private: false` |
| **Nothing server-side protects `main` today** | `gh api repos/nabbisen/sui-id/branches/main/protection`; `gh api repos/nabbisen/sui-id/rulesets` | `404 Branch not protected`; `[]` |
| No `CODEOWNERS` | `ls .github/CODEOWNERS CODEOWNERS docs/CODEOWNERS` | none exists |

So the objection as stated ("a gate in the tree can be weakened in the tree") is correct **and the escape it points at — "configure it server-side" — is also inside the agents' reach as things stand**: a process on this machine that can run `gh` can delete or rewrite branch protection (`admin: true`), rewrite workflows (`workflow` scope), and sign commits as the owner. Server-side is *out* of reach only if the credential agents hold is not the owner's. That is a decision about credentials, not about ledgers.

**How far it goes.** It applies to **every gate in this programme** — G01–G15, RFC 110's G11 guard, RFC 116's contracts — not only to RFC 117. It is a standing condition of the repository, not something RFC 117 introduces, and RFC 117 is right to name it. What is specific to RFC 117 is that it claims more than the other gates do.

**Does it sink D1–D3? No, because two different things are bundled in the RFC and only one of them is defeated.**

- **Enforcement** — "CI fails when an attribution is uncited or a signature is bad". The objection defeats this against an agent that will edit the gate. It survives against an agent that will not.
- **Evidence** — "this entry was signed by a key the agents cannot use". The objection does **not** touch this. An agent that disables the gate still cannot produce a signature; it can only stop anyone being told. The signatures stay verifiable later by anyone, with stock OpenSSH and the pinned public key (the repository is public), in a clone the agent never touched.

The RFC's claim "the owner's decisions become verifiable" is true of the *evidence*. It is not true of the *enforcement*, and the RFC uses one sentence for both.

### Item 2 — is there a construction outside the agents' reach?

For each candidate: does it escape, or move the problem, and what can an agent with commit access still do? Facts above are the baseline.

| Construction | Escapes? | What an agent with today's access still does | What it would take |
|---|---|---|---|
| **Branch protection / rulesets with a required status check** | **No, as things stand.** It moves the problem to the credential. | `gh api -X DELETE …/branches/main/protection` (token has `admin`); or leave protection in place and change the required check's own workflow in the same push (`workflow` scope); or edit `scripts/` and `ci/` (a required check that runs *this tree's* script is only as good as this tree). Personal accounts also lack org-level *required workflows*, which is the one setting that runs the verifier from outside the PR. | Agents get a **different credential**: a fine-grained token (or a separate bot account) with contents/PR write and **no** administration and **no** workflow permission. Then protection is truly out of reach, and a push touching `.github/workflows/` is refused server-side. |
| **`CODEOWNERS` on `ci/`, `scripts/`, `.github/`** | **Only after a credential split and a second identity.** Today there is no file, no protection to enforce it, and the pushing identity is the owner's, who cannot approve his own change. | Push to `main` directly (nothing forbids it), or edit `CODEOWNERS` itself. | Branch protection with "require review from code owners" **plus** agents acting as a distinct account so the owner's approval is a second party. Then "tampering with the verifier" changes from *visible if someone reads it* to *cannot merge until the owner approves that path*. That is the strongest server-side result and it rests on his reading the diff. |
| **Signed tags verified at release** | **Partially.** It moves verification to the moment the owner acts (he cuts the release) and off the per-commit path. | Anything between releases; and the verifier the owner runs lives on a machine where agents run as his user — they can rewrite a verifier script or allowed-signers file kept in his home. | The verifier and the pinned public key must be held **on a different device or account** (a phone, a second machine, a hosted job in a repository the agents cannot write). |
| **Verification somewhere that is not this repository** (a witness) | **Yes, if and only if it is under a credential the agents do not hold.** A witness repository under the same account is *not* out of reach: the token has `repo` on all of the owner's repositories and `read:org`. | Nothing against a witness under a separate account: it fetches the public ledger, runs its own pinned verifier against its own pinned key, and reports to the owner. It can be **detective**, not preventive, unless its result is a required status posted by a GitHub App whose key agents lack. | A second account or organisation, and the owner as the only person who pins the key there (over a browser session with its own 2FA). |
| **Hardware key (D1)** | Escapes for **signing**, and only for signing. | Everything else: the gate, the pinned file, the workflow, omission. | Already proposed. |

**Conclusion for item 2.** Nothing *inside the repository* escapes. What escapes is anything that depends on a credential or a device the agents lack, and there are two such things, both cheap in kind and both the owner's to grant: **(1) the touch on the signing key** (already D1) and **(2) a credential split** so that server-side settings, `CODEOWNERS` and the release verifier are actually out of reach. RFC 117 buys (1) and says nothing about (2). Without (2) its verifier is exactly as tamperable as G01–G15 are today; with (2) it is not, and so are the other fifteen gates. I would raise (2) as its own decision for the owner rather than fold it into RFC 117.

### Item 3 — the honest claim

The sentence RFC 117 should make instead of "the owner's decisions become verifiable":

> **A decision in the ledger is provably the owner's: it is signed by a key held on hardware that an agent cannot use without the owner present, and anyone can verify that with OpenSSH and the pinned public key, in this repository or in a copy. CI fails when a document attributes a decision to the owner without citing such an entry or being on the closed list of pre-adoption attributions. That makes a false attribution by an agent acting in good faith fail visibly. It does not stop an agent that can edit the gate, the pinned keys or the workflow from removing that visibility; only review of changes to those paths does, and today nothing requires that review.**

The second sentence is enforcement, stated as a *visibility* property. That is the claim I would defend.

---

## 1. Findings, ranked

### Blocker

**B1 — The RFC claims enforcement it cannot deliver, and does not state a threat model.** (Item 0, item 12.) One sentence covers two properties of very different strength. Fix: the claim in item 3, plus the threat model in §7, plus naming the credential split as the thing that would upgrade the claim. *Not a reason not to build; a reason to reword before Accepting.*

**B2 — The "physical touch" property is not verifiable by `ssh-keygen -Y verify`, measured.** I crafted `sk-ssh-ed25519@openssh.com` signatures (§3) with the user-presence flag set and cleared. **`ssh-keygen -Y verify` returned exit 0 for all four flag combinations, including "no touch, no PIN".** The flag is readable in the clear in the signature and is bound by it (flipping it without re-signing fails verification), so a *gate can* enforce it, but only by parsing the blob itself; the stock verify does not. D1's central promise ("cannot use it without the owner physically present") therefore holds only if stage 1 adds that check and proves it on the real token. The RFC must say so.

### High

**H1 — The adoption cutoff is keyed to a date *written in the text*, which an agent controls.** D3/D6 say a document asserting an owner decision "dated on or after adoption" must cite an entry. An agent that writes no date, or writes "2026-08-30", is outside the rule. The design review's own lesson about RFC 110 ("a lexical carve-out anyone can write is a hole") applies to the date. **The cutoff must be a property of the tree, not of the sentence:** at adoption, record the set of attributions that exist (the closed baseline, D6); *any attribution introduced after that commit must cite an entry, whatever date it names, or none.* An undated attribution is treated as new. See item 8.

**H2 — D5's boundary is not decidable in the moment, and the RFC's own recent record shows it.** "Accepting an RFC" is listed as routine. But `rfcs/accepted/115-…md` line 5 records that one acceptance message also **ruled three open questions (D10, D11, D12)** — rules, a scope and a security decision, decided inside a routine acceptance. Under D5 the acceptance needs no entry and the three rulings do, and nothing tells the person recording it which they are holding. Sharper test in item 10.

**H3 — The regex the RFC's measurements rest on misses a known-bad attribution, and the counts do not reproduce.** The handoff's command finds, on tracked `.md` files at `11d28a4`: **17 hits in 11 files, 7 distinct dates** (`2026-07-28`, `08-26`, `08-27`, `09-10`, `09-16`, `09-22`, `09-24`). It **does not match `2026-09-09`** — one of the three incidents — because that text reads "`@nabbisen` ruled on 2026-09-09". Two of the seven dates it finds (`09-22`, `09-24`) are not pre-adoption or known-bad at all. The RFC's "seven distinct dated owner attributions across 22 files" is not what the command yields. A wider regex (owner or `@nabbisen` within a line of a decision verb and a date) yields **50 lines in 29 files across 12 dates**, mostly routine. The count depends on the pattern, as RFC 110's review said; a gate keyed to one pattern misses the one you already know about.

**H4 — The verifier and the fixtures live in the tree that the gate protects, and the fixtures need private keys.** Stage 1's mutations ("signature replaced with one made by a different key") need throwaway private keys committed as fixtures. That is fine, but the handoff says "do not stub the key" and "a real entry signed by the real key verifying in CI" — the one real-key check is the only one that touches the pinned key, so the rest of the evidence proves the *gate*, not the *scheme*. State it; and a fixture `allowed_signers` must be a different file from the pinned one and the gate must refuse to be pointed at a fixture in production.

### Medium

**M1 — `ssh-keygen -Y verify` reports every failure as exit 255.** Measured (§2): good key 0; wrong key, unpinned key, missing signer, empty file, wrong namespace, one changed byte, CRLF, a trailing newline, a truncated signature, a junk signature, a missing allowed-signers file — **all 255**; no `-n` is exit 1. The RFC's three failure classes (absent, malformed, unpinned) cannot be told apart from the exit code; `-Y check-novalidate` (structure only: 0 good, 255 junk) separates *malformed* from *unpinned*, and *absent* is the gate's own parse. Use both, and never `find-principals`: it accepts any key listed in the file, so a key an agent adds to `allowed_signers` passes. Fix the principal.

**M2 — Deleting an entry passes the gate.** Open question 3 says so. Nothing in per-entry signatures detects a missing entry, or an old entry replayed after being removed. Fix: sequential ids with a gap check (detects middle deletion), each entry's signed text including the digest of the previous entry's block (detects reordering), and the highest id + head digest recorded outside the ledger (a release tag annotation; the RFC's own witness) to catch tail truncation. Cheap, and it belongs in the signed region (item 5).

**M3 — What the owner signs is a text the agent drafted.** D4 says the entry carries the owner's *verbatim words*. The words live in a chat the repository cannot see; an agent copies them into the draft; the owner signs the draft. The signature proves the owner touched a token over these bytes. It does not prove the bytes equal what he said, unless he read them. The RFC states the attention boundary but not this specific step, and it is the step where all three historical failures would recur (a paraphrase that drifted, typed by an agent). D4 is a discipline and the RFC says so; I would add the ceremony in item 14 that makes the owner's read the visible step.

**M4 — Lane wiring is more than "a lane through `ci-gate.sh`".** From RFC 116's design review (H2/H3): a lane needs a `[gates]` row, a `[gate_owners]` row, a row in an RFC lane table byte-equal to the command, a `[gate_lane_sources]` entry, a workflow job with its setup steps and, for a gate that runs `ssh-keygen`, a stated minimum OpenSSH and an evidence line printing `ssh -V`. Budget for it; and sequence against RFC 116 stage 3, which changes how lanes are generated.

### Low

**L1** The handoff's first measurement is stale by three commits (414 now, not 411); the identities (two) and `%G?` result (all `U`) reproduce.
**L2** `ed25519-sk` needs `libfido2` where it *signs*. On this machine it is absent (`ssh-keygen -t ed25519-sk` → `ssh-sk-helper: error while loading shared libraries: libfido2.so.1`), so stage 1's prerequisite is a token *and* the library on the owner's signing machine. Verification needs neither (§2). I make no request to install anything.
**L3** RFC 117 says OpenSSH "8.2+, which every supported runner has". I could not measure `ubuntu-24.04`. I did measure OpenSSH 10.5p1 here. The gate should print `ssh -V` and fail closed under a pinned minimum, so the claim becomes evidence.

---

## 2. Item 4 — `ssh-keygen -Y sign` / `-Y verify`, measured

**Runner availability and version: not measured.** I cannot run `ubuntu-24.04` from here. I ran OpenSSH_10.5p1 (`ssh -V`), which lists `-Y find-principals`, `match-principals`, `check-novalidate`, `sign` and `verify` in `man ssh-keygen`. The RFC's "8.2+" is unchecked (L3).

**Invocation (worked):** with a throwaway ed25519 key pair in the scratchpad.
```
ssh-keygen -Y sign   -f KEY -n sui-id-decision -q FILE          # writes FILE.sig
ssh-keygen -Y verify -f ALLOWED -I owner -n sui-id-decision -s FILE.sig < FILE
ssh-keygen -Y check-novalidate -n sui-id-decision -s FILE.sig < FILE
```
`allowed_signers` line (one per pinned key): `owner namespaces="sui-id-decision" ssh-ed25519 AAAA…`. **A namespace is required to sign** (`-Y sign` without `-n`: exit 1). `namespaces=` in the allowed-signers line restricts a key to that namespace; a signature made under `-n file` fails to verify under `-n sui-id-decision`.

| Case | Exit | Message |
|---|---:|---|
| good signature | **0** | `Good "sui-id-decision" signature for owner with ED25519 key SHA256:…` |
| signed by a key that is not pinned | 255 | `Could not verify signature.` |
| `-I` names a principal not in the file (missing signer) | 255 | `Could not verify signature.` |
| empty `allowed_signers` | 255 | `Could not verify signature.` |
| `allowed_signers` file missing | 255 | `Unable to open allowed keys file …` |
| malformed: truncated | 255 | `Couldn't parse signature: missing footer` |
| malformed: junk | 255 | `Couldn't parse signature: missing header` |
| namespace mismatch (sign `file`, verify `sui-id-decision`) | 255 | `Couldn't verify signature: namespace does not match` |
| one byte of the message changed | 255 | `Signature verification failed: incorrect signature` |
| CRLF line endings | 255 | same |
| one extra trailing newline | 255 | same |
| second pinned key, verified as its own principal | 0 | `Good … for spare …` |
| `sign` without `-n` | **1** | usage error |
| `check-novalidate`: good / junk | 0 / 255 | structure only |

The byte-exactness cases are the important ones: **a signature covers the raw bytes; a changed line ending or one extra newline breaks it.** Scratch script: `exp4.sh` in the session scratchpad (not committed).

---

## 3. Item 7 — hardware keys, measured

Questions: does an `-sk` key require touch on every signature, can `no-touch-required` be set, and can the gate detect it from the public key alone?

- **`no-touch-required` can be set at generation** (`man ssh-keygen`, FIDO section: "Indicate that the generated private key should not require touch events … when making signatures") **and changed afterwards on the key handle** (`ssh-keygen -p -O no-touch-required` / `touch-required`; also `verify-required` for PIN). So an `-sk` key does **not** inherently require touch: it is a per-key option. Resident vs non-resident does not change this.
- **It cannot be detected from the public key.** The public key line is `sk-ssh-ed25519@openssh.com <key + application "ssh:">`; the option lives in the local key-handle file and is reported per signature. (Read from the man page and the wire layout below; I did not have a token.)
- **It can be detected from the signature, and the stock verifier does not.** Without a token, I built the exact SSHSIG structure for `sk-ssh-ed25519@openssh.com` from a software ed25519 key (`openssl pkeyutl -sign -rawin`; layout per OpenSSH `PROTOCOL.sshsig` and `PROTOCOL.u2f`: signed data is `SHA256("ssh:") ‖ flags ‖ counter ‖ SHA256(SSHSIG blob)`, signature blob is `string alg ‖ string sig ‖ byte flags ‖ uint32 counter`). Then:

| Flags byte | Meaning | `ssh-keygen -Y verify` |
|---|---|---|
| `0x01` | user present | **exit 0** |
| `0x00` | **no touch** | **exit 0** |
| `0x05` | present + verified (PIN) | exit 0 |
| `0x04` | verified, not present | exit 0 |
| `0x00` flipped to `0x01` without re-signing | forged flag | exit 255 (`incorrect signature`) |

  The flags byte sits at the end of the signature blob, in the clear (`len-5`), bound by the signature. **`-Y verify` ignores it.** A gate that means "the owner touched the token" must read that byte itself and require bit `0x01` (and `0x04` if a PIN is wanted).
- **Caveat, stated plainly.** My signatures were synthetic. The layout is from the OpenSSH documents and the verifier accepted it, which shows the verifier's behaviour, not the token's. The claim "a real token reports `UP=0` when asked to sign without touch" is from the protocol description and **must be confirmed on the real token in stage 1**: sign once with touch and once with a handle made `no-touch-required`, and show the gate accepts the first and refuses the second. That is one more line in the stage's evidence, and without it the touch property is unverified.
- **`verify-required` (PIN) is worth adding.** A touch alone can be given by a person who is looking at something else (the RFC says so); a PIN cannot be given without the person typing it on a terminal an agent does not read. It costs a few seconds per decision and raises the bar from "physically present" to "present and knowing the PIN".

---

## 4. Item 5 — canonical text

**Propose a literal delimited region; verify by slicing, not by parsing.** Format I prototyped and attacked (scratch: `ledger_proto.py`, `exp5.sh`; ~60 lines, not committed):

```
# header comments and blank lines only
=== DECISION D-0001 ===
date: 2026-09-24
words:
> RFC 115 and 110 are accpepted.
=== END D-0001 ===
-----BEGIN SSH SIGNATURE-----
…
-----END SSH SIGNATURE-----
```
- **Signed bytes = the lines from the `=== DECISION` marker through the `=== END` marker, inclusive, LF-terminated, exactly as in the file.** No re-serialisation, no field order to agree on, nothing reconstructed. A reader sees precisely the signed bytes.
- **The gate's rules:** the markers are matched by a fixed anchored pattern; the signature block must follow the end marker **immediately**; **any line outside a block+signature, other than a comment or blank line, fails** (this is what closes the "different text than displays" hole); ids unique; no `\r`; no Unicode control, format, or line/paragraph separator characters (bidi overrides are `Cf`) and no non-space spaces.
- **A reader with no tooling but OpenSSH** verifies an entry by hand (worked, exit 0):
  ```
  sed -n '/^=== DECISION D-0001 ===$/,/^=== END D-0001 ===$/p' ledger.txt > entry.txt
  sed -n '/^=== END D-0001 ===$/,/^-----END SSH SIGNATURE-----$/p' ledger.txt | sed 1d > entry.sig
  ssh-keygen -Y verify -f allowed_signers -I owner -n sui-id-decision -s entry.sig < entry.txt
  ```
- **Use a `.txt` (or extensionless) ledger, not Markdown.** Rendered Markdown hides HTML comments and turns `===` under text into a heading; the point is that display equals bytes.

**Attacks run against the prototype** (each result is the gate's verdict):

| Case | Verdict |
|---|---|
| good ledger, 2 entries | PASS (2) |
| one word "corrected" inside a signed block | FAIL: signature incorrect |
| an unsigned `words:` line appended after the ledger | **FAIL: line outside any signed block** — whereas a parser that takes the *last* `words:` in the file would have displayed `'forged after the fact'` |
| U+202E (bidi override) inserted in a quote | FAIL: contains U+202E (Cf) |
| CRLF line endings | FAIL: CR byte present |
| blank line inserted between block and signature | FAIL: not followed by a signature |
| D-0002's text carrying D-0001's signature | FAIL: signature incorrect |

**Is a detached signature over a delimited block sufficient, or must the entry embed its own digest?** Sufficient. A digest inside the block adds nothing (it is signed with the rest) and outside it is unsigned. What *is* worth embedding, inside the signed region, is the **digest of the previous entry's block** (M2): it makes deletion and reordering detectable, and costs one line.

**One thing this cannot do.** Non-ASCII text is allowed (the owner's words may be in Japanese), so a homoglyph the eye cannot tell from another character is signed as it is. The defence is the owner reading the entry before signing, not the gate.

---

## 5. Item 6 — per-entry versus commit signatures

The RFC's choice stands, with one weakness named.

- **What a commit signature gives that an entry signature does not:** (a) the signature covers the *tree* and the parent, so a decision can be bound to exactly the state it authorised; (b) a time: the committer date is inside the signed object. **(b) is weak**: it is a value the signer's machine wrote, not a notarised time. The SSH signature format itself carries no time (`-O verify-time` is a verifier-supplied input for `valid-after`/`valid-before`, not something signed).
- **What entries give:** self-contained evidence that survives any history operation. History rewrites on this project have happened (measured: `git reflog | grep filter-branch` shows two `filter-branch: rewrite` entries, one of them at `1e59e3d`, the commit that introduced the 2026-07-28 attribution); a signed *commit* would be invalidated by a rewrite, an entry is not.
- **Does an entry bind a time?** The `date:` inside the signed region is the owner's own attestation, typed or approved at signing, which is the right level for a governance record. If an external time is wanted, the cheap anchor already exists: the public repository records when a push arrived, and a release tag annotation recording the ledger head digest (M2) gives a dated anchor the ledger itself cannot forge.
- **Countersigning (both) is not worth it.** It costs a second touch per decision and buys the tree binding, which decisions about *rules* do not need.
- **One caveat for signed commits specifically:** the repository's commits are all signed with the shared GPG key, so a hardware-key commit signature would be a second, different signature on the same object. Git supports it awkwardly. Another reason to prefer entries.

---

## 6. Items 8–11 — the citation rule

**Item 8 — an exception mechanism that is not a hole.** Three ingredients, none of them lexical:

1. **History is exempt by being *in the baseline*, not by being marked.** At the adoption commit, the gate's baseline records every attribution that exists (path + normalised text hash), including the seven and the three known-bad. That set is closed. The rule then reads: *an attribution not in the baseline must cite an entry.* This is H1's fix: the cutoff is the tree at adoption, so an undated or back-dated new attribution still fails.
2. **The exemption lives in a different file from the assertion**, so it cannot be written by the sentence it exempts, and it is a visible edit to a path that `CODEOWNERS` can protect (item 2) — not the same as a marker any author types into their own paragraph.
3. **Structured header fields are exempt by location**, because RFC 110's G11 policy already fixes the header's label set. But see item 10: those headers also carry quotations and rulings.

Residual: the baseline and its `ci/` path are in the tree (item 0). This is the same weakness every gate has, not a new one.

**Item 9 — detecting an assertion without banning `Approved by` or matching nothing.** Do not try to detect *phrasing* as the enforcement; detect **attribution shape and report all of it**. Measured: the handoff's pattern gets 17 hits, misses the `09-09` incident; a wider one gets 50 in 29 files. The stable design is the *census*: match generously (owner or `@nabbisen` with a decision verb stem — `rul`, `decid`, `decision`, `authori[sz]`, `approv`, `accept`, `direct`, `instruct` — regardless of date), **print every hit in the CI summary**, and fail only on a hit that is neither baselined nor citing an entry. False positives are cleared by adding to the baseline, in a visible diff; false negatives are the residual, and are stated. This is the same mechanism as item 13's second candidate, and it needs no key.

**Item 10 — is D5's boundary decidable?** No, as written (H2). Replace the test of *what* the decision is with a test of *what the writer is doing*: **a sentence needs an entry if it attributes something to the owner by name or role and the writer intends a reader to rely on it.** That is decidable at the moment of writing, by the writer, and it is exactly the act the three failures share. Routine facts stay unattributed ("RFC 115 is Accepted", not "the owner ruled…"); the RFC's `Accepted on` / `Approved by` header fields remain the routine channel, but a header quotation that also records **rulings** (as RFC 115's does) cites an entry for those rulings. The boundary then argues itself away: if you would write "the owner said", sign it or do not write it.

**Item 11 — the pre-adoption attributions, and any load-bearing ones.** "Label them unverifiable" is right; the baseline is where the label lives. Load-bearing for a rule **still in force** (the reader's question, named, not judged):

| Attribution | Where | Why it matters |
|---|---|---|
| **"Owner decision, 2026-08-26" — role independence** | `ROADMAP.md:368` | It is the rule under which *every design review in this programme, including this one, is routed*; the request that asked for this review cites it as its "Route". Its only source is the architect-authored ROADMAP text. |
| "Owner decision, 2026-08-26" and "2026-08-27 (Part B, B1)" | `rfcs/accepted/094-transactional-audit-registry.md:1175`, `:1067`; `rfcs/accepted/096-…:166`; `rfcs/handoffs/094-…/migration-checklist.md:175`, `backup-into-store.md:43` | Conditions of RFC 094's M2a exit and an accepted residual. |
| **"owner ruling, 2026-09-10"** | `rfcs/handoffs/113-test-file-organization/README.md:12` | **This is the same date as the ruling the owner says he did not make** (`ROADMAP.md:474`: the `roadmap/` directory's "Owner ruling, 2026-09-10"). 113's citation is for a *different* thing — the test-file rule, "tests live in sibling files", which is a standing project instruction enforced on every stage. It may be a separate, genuine ruling, or it may share the disowned one's provenance. **I cannot tell, and I do not decide.** It is load-bearing and it needs the owner's word. |
| "Owner decision, 2026-09-10" | `rfcs/archive/018-rfc-lifecycle-policy.md:14` | The RFC 018 → RFC 000 retirement. Same date again. |
| "owner ruling 2026-09-16" | `rfcs/proposed/101-…md:10` | The §6.3 correction of RFC 101. |

I found no case where an attribution in force is *contradicted* other than the known 09-09 one (recorded, unresolved, `ROADMAP.md:405-420`).

---

## 7. Item 12 — the adversary

The RFC states none. Three candidates and what D1–D6 do against each:

| | (a) helpful but wrong | (b) adversarial agent | (c) third party with repository access |
|---|---|---|---|
| **D1 key** | makes a fabricated entry impossible | stops **forgery**; nothing else. The agent can still sign with the owner's *shared* GPG key, edit the gate, omit, or ask the owner to touch a token over text he has not read | same as (b) without the machine-local key/`gh` access; the public ledger stays verifiable by anyone |
| **D2 per-entry signature** | evidence survives rewrites | same | same |
| **D3 citation gate** | **the control that matters**: it turns the silent failure into a failing check | **nothing**: edit the gate | nothing, unless server-side protection is configured (measured: none) |
| **D4 verbatim words** | catches paraphrase drift, if the writer has to produce the words | agent can invent "verbatim" words; the signature is the only check | same |
| **D5 scope** | reduces friction | n/a | n/a |
| **D6 labelling** | honest about history | n/a | n/a |

**(a) is the real adversary**, as the RFC says and the record supports: three failures, each the architect role writing an attribution it believed. Against (a) the load falls on D3/D4, **and neither needs a key.**

## 8. Item 13 — is a hardware key proportionate against (a)?

For each of the RFC's three failures I use the RFC's own account of it (the RFC's own table; `ROADMAP.md` §S1). **I do not know why any of them happened**; where the outcome depends on that, I say *only if*.

| Incident | (i) verbatim words, no crypto (D4 alone) | (ii) CI lists every attribution (census) | (iii) agents may never write the phrase, only quote an owner-maintained file | (iv) RFC 117 as specified |
|---|---|---|---|---|
| **2026-07-28**, "Owner ruling" in §S1 and four RFC headers, all in one commit (`1e59e3d`) | **Only if** the writer had no words to quote: `ROADMAP.md` says the conversation "is not available" and the role-framework document asserted the ruling on its own author's word. A required quotation would have forced "I have no words". Not if it invented them. | Would have listed five new attributions in that commit's summary. Caught **if** the owner read it; seven RFCs were blocked for four weeks *because* it was not read. A **failing** check that only he can clear catches it; a **listing** that he may read does not. | Catches it, **if** "owner-maintained" is enforced by something other than convention. In the tree, nothing is (measured: every commit reports `U`). | Catches it: no entry, no citation, gate fails. |
| **2026-09-09**, RFC 098's clause, contradicting the 2026-08-26 decision | Same as above; and the quotation would sit next to the contradicting 08-26 text. | The handoff's own regex would **not** have listed it (H3). A census with the wide pattern would. | Same as above. | Catches it (with the gate keyed to a census, not the narrow pattern). |
| **2026-09-10**, `roadmap/` "Owner ruling" in `ee48257` | Same. | One attribution in that commit's summary: visible on the day, before nineteen packages accumulated, **if** read. | Same. | Catches it. |

**Reading the table.** A key adds one thing (ii) and (i) cannot give: **an attribution the agent *cannot* satisfy by writing more text.** Without it, "must quote the owner's words" and "must cite an owner-maintained file" are conventions an agent can satisfy dishonestly or by accident. Against (a) — an agent that believes what it writes — conventions are usually enough: an agent that honestly has no words, no file entry, and no entry id will *say so*. The key is what makes the rule survive an agent that does not notice it has no evidence.

**Is it proportionate?** As the *only* measure, no: the cost is the owner's touch per rule-level decision (item 14), and most of the benefit against (a) comes from the census and the quote requirement, which need no token. As the *last layer over* a census that fails on new attributions, yes: it is cheap for a handful of decisions a month and it is the only layer whose output survives a disabled gate. **My view (not a ruling): build the census first, because it needs no key and works today; add the signing layer when the owner has the token.** The handoff makes the key a hard prerequisite of every stage; I would split it (§9).

## 9. Recommendation: build with these changes

1. **Reword the claim** to the item-3 sentence, and add the threat model of §7. (B1.)
2. **Split the work:** a **stage 0, no key**: the attribution census (item 9), the closed baseline recorded by *tree state* (H1, item 8), and the "new attribution must cite an entry or be baselined" failure. This delivers the control that matters against (a) now, and gives stages 1–3 the tooling they need. Stages 1–3 stay conditional on the key.
3. **The gate reads the signature's flags itself** and requires user presence (and PIN if chosen); stage 1's evidence gains the two-signature real-token test (§3). Until then the RFC must not claim "physical touch". (B2.)
4. **Adopt the literal delimited format** of §4, with ids sequential, the previous block's digest inside the signed region, the head digest anchored outside the ledger (release tag), and the character rules. (M2.)
5. **Key the cutoff to the tree, not the text** (H1); replace D5's boundary with the writer's-act test (H2); require citations for **rulings recorded inside** an acceptance header (item 10).
6. **Fix the principal and use `check-novalidate` then `verify`;** never `find-principals` (M1).
7. **State the credential split as an open decision for the owner, separate from this RFC** (item 2): it is what would let the *whole* gate system claim more than "visible". I recommend raising it; I do not assume its answer.
8. **Owner to rule on the load-bearing attributions** in item 11, in particular `113/README.md:12`, before stage 3 labels them.
9. **Wire the lane the RFC 116 way** (M4), and sequence stage 1 against RFC 116 stage 3.

I do not recommend *not* building: the evidence property survives the objection, and the census is nearly free. I would recommend not **Accepting the text as it stands**, because it claims the enforcement it cannot give.

## 10. Item 14 — what the owner does per decision

A real ruling, end to end, as I would build it (my estimate, **not measured on hardware**):

1. He types the ruling in chat. *(already happens)*
2. The agent writes the entry to a **draft file** — the bytes between the markers, with the previous block's digest — and tells him the path. *(agent, seconds)*
3. He reads the exact bytes: **`cat -A draft.txt`** (shows CR, tabs, and non-ASCII as visible escapes, which is what makes hidden characters visible). *(15–30 s if he reads it; this is the step the whole scheme depends on)*
4. He signs with a fixed command, typed from his own shell, **not a helper in the repository** (a repo-resident helper is agent-writable and would sign whatever it likes): `ssh-keygen -Y sign -f ~/.ssh/… -n sui-id-decision draft.txt`, then PIN and touch. *(5–10 s)*
5. He or the agent appends block + signature to the ledger and commits. *(seconds)*

**About a minute for the owner** if he reads and the command is short; **3–5 minutes and error-prone** if the file, namespace and delimiters are assembled by hand. The RFC's Risks say a scheme felt as friction lapses; this one is on the edge, and the fixed two-command procedure documented in `docs/src/contributing/` is what keeps it under a minute. Every step except 3 and 4 is agent work.

---

## 11. Measured, in one place

| Claim in the RFC or handoff | Command | Result |
|---|---|---|
| 411 commits under two identities | `git rev-list --count HEAD`; `git log --format=%ae \| sort \| uniq -c` | **414** (three commits since); two identities, 216 + 198 |
| `commit.gpgsign = true`, owner's key | `git config --get commit.gpgsign`; `user.signingkey` | `true`; `25757DA6CBF7022C4E14CCAC1B3066B87DB99A34` |
| an agent's commit is indistinguishable | `git log --format='%h %G? %ae' -10` | ten of ten `U` |
| "seven distinct dated attributions across 22 files" | handoff's `grep -rEoi` over tracked `.md` | **17 hits, 11 files, 7 dates**; misses `2026-09-09`; not 22 files |
| wider census | wide pattern (owner/`@nabbisen` + decision verb + date) | 50 lines, 29 files, 12 dates |
| no server-side protection | `gh api …/branches/main/protection`, `…/rulesets` | 404; `[]` |
| agents' token administers the repo | `gh auth status`; `gh api repos/nabbisen/sui-id` | scopes `repo`, `workflow`; `admin: true`; personal account |
| `-sk` needs `libfido2` to sign | `ssh-keygen -t ed25519-sk -f …` | `libfido2.so.1: cannot open shared object file` |
| runner OpenSSH version | — | **not measured** |
| real-token touch behaviour | — | **not measured** (synthetic signatures only, §3) |

**What I did not do.** I did not measure `ubuntu-24.04`, did not use a real token, and did not build the gate: the prototype in §4 is 60 lines of scratch to test the format, and none of it is committed or proposed as the implementation. The `gh api` calls were three read-only GETs on the repository's own settings, using the owner's credential that this session already holds; they changed nothing.

---

