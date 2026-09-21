# Audit notes: escape attribute values

**Authorized by.** [`ROADMAP.md`](../../ROADMAP.md) §Non-RFC work packages. Owner
authorization 2026-09-22.
**Implementer.** Mid-capability model.
**Baseline.** The commit that adds this file, or later.
**Found by.** RFC 103 stage 3's review (finding 3), and its stage 4 tests.

## The defect

An event's `note` is built as `key=value` pairs joined by spaces, and values are
not escaped or quoted. Any event whose attributes carry operator- or
user-supplied text can therefore be made to look as though it carries fields it
does not. A recovery-link reason of

    x step_up=fresh:totp:1 via=cli

renders a note that contains those pairs.

**What saves it today, and why that is not enough.** Every command writes the
supplied text *before* the real fields, so the last occurrence of each key is the
true one, and a reader who takes the last wins is never misled. But:
- the operator guide's `LIKE '%step_up=…%'` queries match anywhere in the note,
  so a forged value produces a **false positive** (never a false negative);
- the rule "the last one wins" is unwritten in the data, and a future command
  that appends a free-text attribute last would invert it;
- the same shape reaches the audit page, the CSV export and the event labels.

## Required

- **Escape values where the note is built**, in the store's audit-note builder,
  so that a value cannot introduce a key/value boundary. Choose one and say why:
  - quote values that contain a space or `=` and escape the quote and backslash,
    or
  - percent-encode space and `=` in values.

  Whichever: the encoding is reversible, documented next to the builder, and
  applied to **every** event, not only the ones with free text.
- **Keys are unaffected**; they are `&'static str` from the descriptors.
- **Do not change the attribute set of any event**, and add no new event.
- **Readers.** Update anything that parses a note: the operator guide's queries
  (and their caveat sentence, which can then go), `docs/src/reference/audit-events.md`
  where it shows note formats, and the audit page if it splits on spaces.
- **Historical rows are not rewritten** (RFC 098 rule 4). Say in the guide that
  rows written before this change carry the old form.

## Evidence

- A round-trip test over a table of hostile values: a space, `=`, a quote, a
  backslash, a newline (already refused for reasons, but other attributes may
  carry one), a value that looks like `k=v`, and a UTF-8 value at the 512-byte
  bound.
- The RFC 103 test that pins the forged-reason case (`u37_a_reason_that_imitates_fields_cannot_displace_the_recorded_ones`)
  is extended: with escaping, the forged pairs must not appear as pairs at all.
- One mutation: the escape removed, shown caught.
- G13 (no event added), G15, G10a, G10b; fmt, both clippy scopes, the test count
  before and after, MSRV 1.95.
