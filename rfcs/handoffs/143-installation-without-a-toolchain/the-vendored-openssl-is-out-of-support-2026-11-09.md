# RFC 143 — the vendored OpenSSL is out of support, and `0.89.0` is held

> ## Decided 2026-11-09. `@nabbisen`: *"A"*
> Static, linked against OpenSSL **3.5 LTS** built from upstream source by the
> release workflow, and verified against a pinned SHA-256. **`0.89.0` stays held**
> until that is built and rehearsed. The architect prototypes it first (§3).

**Prepared by.** The architect, 2026-11-09, at step 0 of `0.89.0`'s
preparation. Every fact below was measured today against a primary source,
named beside it.

## 1 — what was found

Step 0's `Security audit` run (`42554168988`, on `46b7dda`) was **green**,
including the `openssl-src freshness` job. That job's one informational line,
*"a newer OpenSSL series than 3.6 is published"*, was followed up:

| Fact | Source |
|---|---|
| The static build vendors **OpenSSL 3.6.3** (`openssl-src 300.6.1+3.6.3`) | `Cargo.lock` |
| **OpenSSL 3.6 is out of support.** OpenSSL lists only 4.1 (supported to 2027-11-23), 4.0 (to 2027-05-14) and **3.5 LTS (to 2030-04-08)**, and says *"All versions not listed above are out of support and should not be used in production"* | openssl-library.org/source/ |
| Upstream shipped **3.6.4 and 3.6.5** after 3.6.3, and OpenSSL's vulnerability page lists security fixes in both. **3.6.3 has none of them** | `openssl/openssl` tags; openssl-library.org/news/vulnerabilities/ |
| **`openssl-src` never packaged 3.6.4 or 3.6.5.** Its 3.5 line stops at 3.5.5 while upstream is at 3.5.9. It follows only its newest series (`400.0.2+4.0.3`) | crates.io, `openssl-src` versions |
| **No `openssl-sys` can use that newest series.** The current release, 0.9.118, still requires `openssl-src ^300.2.0` | crates.io, `openssl-sys` dependencies |

**So with today's crates, the `vendored` feature cannot produce a supported,
patched OpenSSL at all.** Had `0.89.0` shipped, its static binaries would have
carried an unsupported OpenSSL with known, fixed vulnerabilities, in the code
that verifies every passkey sign-in.

## 2 — why nothing caught it, and the error is the architect's

Your static decision (2026-11-08) rested on the architect's stage 1 result,
which said that *"`openssl-src` follows each [patch release] with a new crate
version"*. **That is false**, and it was not measured. The freshness check was
built on it: it compares `Cargo.lock` with **`openssl-src`'s own** newest version
in the same series. When `openssl-src` stops following a series, the check
compares a stale version with an equally stale one and passes. **It checked
the supplier against itself.**

The glibc builds and `cargo install` are unaffected. They use the system's
OpenSSL, which the operating system patches.

## 3 — the options

| | **A. Static, OpenSSL 3.5 LTS built from upstream source** | **B. Dynamic (glibc)** | **C. Static, `vendored`, wait for the crates** |
|---|---|---|---|
| How | The release workflow downloads `openssl-3.5.x.tar.gz` from openssl-library.org and checks it against a SHA-256 pinned in this repository. It builds a static OpenSSL for each target, and links it through `openssl-sys`'s `OPENSSL_DIR` + `OPENSSL_STATIC`. The `vendored` table goes | Build against the system `libssl` on an older base image, so the binary runs on distributions at least that old | Ship 3.6.3 until `openssl-sys` accepts `openssl-src` 400 |
| Who patches OpenSSL | This project, **the day upstream ships**: bump one pinned version and hash. No third-party crate in the way | The operator's OS | Whenever two crates move. Measured: they have not, for months |
| Support window | 3.5 LTS to **2030-04-08** | The OS's | None. 3.6 is already out |
| Runs on | Any Linux with a CA bundle | Distributions at least as new as the build base | — |
| The freshness check | **Rewritten to compare against upstream OpenSSL's own release list**, and to fail when the pinned series leaves OpenSSL's supported list | Not needed | Cannot be made honest |
| Cost | A new pinned input (version + hash). The OpenSSL build in the workflow, about one more step per target. The check's rewrite | Choosing and pinning a glibc floor. A narrower "runs on" | — |

**C is not acceptable.** It ships known vulnerabilities, and is listed only to
say so.

**The architect's recommendation: A.**
1. It keeps what static was for: the people RFC 143 serves are the least able
   to match a glibc version.
2. It removes the supplier that failed. The project takes OpenSSL from OpenSSL,
   verified by hash, on the LTS line, with years of support.
3. It makes the commitment you accepted with static true: an OpenSSL security
   release means a sui-id release. Under `vendored`, that commitment could not
   have been kept, because no crate carried the fix.

**And A is prototyped before it is built**: the first act of the next stage is
to link one static binary against a source-built 3.5 in a rehearsal. That
binary then has to pass the same checks every rehearsal runs, passkey
verification's OpenSSL path included. If the prototype fails, this comes back
to you with what failed.

## 4 — what is yours

1. **Static via upstream OpenSSL 3.5 LTS (A), or dynamic (B).** Recommended: **A**.

Until you answer, `0.89.0` stays held. `main`'s Quick Start and README already
describe the download. A reader who follows them now gets a 404 from `curl
-f`. That is confusing, but it cannot install anything, so the documents stay
as they are rather than being reverted for a short hold.
