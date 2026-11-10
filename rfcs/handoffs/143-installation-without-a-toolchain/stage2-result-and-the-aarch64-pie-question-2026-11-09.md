# RFC 143 stage 2 result — one fact about the `aarch64` artifact comes back to you

> ## Decided 2026-11-10. `@nabbisen`: *"Agreed."*
> Ship `aarch64` as it is, and say so. The Quick Start's download section and
> `0.89.0`'s `Added` entry state, in one sentence, that the ARM build does not
> use address randomisation for its own code and the x86_64 build does.

**Prepared by.** The architect, 2026-11-09, after rehearsal
[`42536756643`](https://github.com/nabbisen/sui-id/actions/runs/42536756643)
on `b6fa8fb`.

## 1 — where stage 2 stands

**The release workflow now builds, checks and attests both artifacts, and
publishes nothing.** All seven criteria, set before the first rehearsal, were met:
- both static builds, built natively on their own architecture;
- each starts in a bare `debian:stable-slim` and reaches the setup screen;
- a directory sign-in over LDAPS, by short hostname, through the musl binary;
- an email delivered over STARTTLS;
- `SHA256SUMS`, and a provenance attestation for every file;
- nothing published, tagged or released.

The architect also verified the downloaded artifacts as an operator would:
`sha256sum -c` passes, `gh attestation verify` passes, and **a single flipped
byte fails both**.

It took three rehearsals. Each failure was in the check itself, not in sui-id:
- the LDAP container would not start on a read-only certificate mount;
- the directory demanded client certificates, which sui-id does not send;
- the assertions looked for log lines that the check's log level never emits.

## 2 — one defect found by review, being fixed now

The attestation records the commit **the workflow run was started on**, not the
commit it built. Today's release command starts `publish.yml` on `main` and
builds the tag, so if `main` had moved past the tag, the attestation would
name the wrong commit, and still verify. **Fix round 2** makes the release run
on the tag itself and refuse otherwise, so the attested commit is the built
commit by construction. Operators can then verify against the release tag
itself. This is a correctness fix inside the authorized scope. **No decision is
needed.**

## 3 — the question: the `aarch64` binary is not position-independent

**What PIE is.** A position-independent executable can be loaded at a random
address each time it starts (ASLR). That makes a memory-corruption bug harder
to exploit, because an attacker cannot know in advance where the code sits.
sui-id is Rust, which rules out most such bugs. But the static build carries
OpenSSL, which is C, and it runs on every passkey sign-in.

**What ships.**

| Artifact | Position-independent |
|---|---|
| `x86_64` | **Yes** (`static-pie linked`). The workflow now fails if this ever regresses |
| `aarch64` | **No** (`statically linked`) |

**Why, measured, not assumed.** Rust's own definition of the `aarch64` musl
target does not enable static-PIE; the `x86_64` one does. And the compiler
enforces this itself (`compiler/rustc_codegen_ssa/src/back/link.rs:2272` at
rustc 1.99.1): a request for PIE on a target without static-PIE support is
silently linked as non-PIE. **No flag on the stable compiler changes it.** The
only route is a custom target definition, which needs the nightly compiler,
and this project releases with stable.

**The options:**

| | Ship `aarch64` as it is, and say so | Hold `aarch64` back from `0.89.0` |
|---|---|---|
| ARM operators | Get a working static binary, without one hardening layer | Need `cargo install`, which is what RFC 143 exists to remove |
| Honesty | The Quick Start and release notes state the difference plainly | Nothing to explain |
| Your settled decision | Kept: both architectures | Reverses it |
| When it improves | Automatically, if Rust enables static-PIE for this target. The workflow reports `pie: yes`, and the check is made strict | Same |

**The architect's recommendation: ship it, and say so.** In the Quick Start's
download section, one plain sentence: *the ARM build does not use address
randomisation for its own code; the x86_64 build does*. An ARM operator is
better served by a static binary with that difference stated than by no
binary. And a difference that is written down cannot be misunderstood. Stage 4
carries the sentence.

**Stage 3 is not held by this question**, because nothing here changes what
the workflow builds. Only stage 4's wording depends on your answer.
