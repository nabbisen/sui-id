# Owner attributions (G16)

This repository has, three times, recorded a decision as the owner's that the
owner did not make. Each was a single sentence in a single commit that nobody
read. Lane **G16** exists so that such a sentence cannot be added *unseen*.
It is described by [RFC 117](https://github.com/nabbisen/sui-id/blob/main/rfcs/proposed/117-verifiable-owner-decisions.md)
(stage 0) and needs no key.

## What it does

G16 reads every tracked text file and finds each sentence that pairs an owner
word (`@nabbisen`, `owner`) with a decision verb (`ruled`, `decided`,
`decision`, `authorised`, `approved`, `accepted`, `directed`, `instructed`).
It does this **whatever date the sentence names**, or none. In Markdown every
sentence counts; in code and configuration only comments do.

It prints every one it finds, new ones first, to the job log and to the GitHub
step summary. It **fails** when a sentence is not in the closed baseline,
`ci/owner-attribution-baseline.txt`, which lists the attributions that existed
when the gate was adopted.

## When it fails

A new attribution has appeared. Read it, then do one of three things:

1. **Remove the attribution.** If you wrote "the owner ruled X" because you
   believe it, and you cannot point to the owner's own words, delete it. State
   the fact ("RFC 115 is Accepted") without attributing it.
2. **Rephrase it as your own proposal.** "The architect proposes X" is not an
   attribution.
3. **Add it to the baseline**, deliberately, in a visible edit to
   `ci/owner-attribution-baseline.txt` (run
   `python3.14 scripts/check-owner-attributions.py --root . --policy ci/owner-attributions.toml --update-baseline`
   and read the diff). Do this only for a false positive (a sentence that is
   about something else) or for an attribution you can quote from the owner's
   own words. **Do not do it to make the check pass.**

## What it does not do

- It does not verify that a decision is real. Nothing in stage 0 can. Signed
  ledger entries are the later stages of the RFC.
- It does not honour a citation yet. There is no ledger, and a citation checked
  against an unsigned file would be a hole.
- It does not stop a commit that edits the script, the policy or the baseline
  in the same change. The verifier lives in the tree it verifies, as every gate
  in this repository does. A change to `scripts/check-owner-attributions.py`,
  `ci/owner-attributions.toml` or `ci/owner-attribution-baseline.txt` should be
  read as carefully as a change to a security control.
- It matches text. An attribution with no owner word in it is not seen.
