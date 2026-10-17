# RFC 139 — handoffs

**Governing RFC:** [RFC 139](../../accepted/139-getting-started-leaves-its-warnings-elsewhere.md)
— Getting Started leaves its warnings in other documents. **Status: Accepted**,
2026-10-14.

Documentation only. No code, no schema, no crate's public surface. The one code
defect the review found belongs to [RFC 107](../../accepted/107-cli-help-completeness.md)
and is a non-goal here.

**Updated 2026-10-14.** One dispatch open.

### Dispatched and open

- [`the-newcomers-path-2026-10-14.md`](the-newcomers-path-2026-10-14.md) —
  **DISPATCHED, this is the open work.** The five decisions D1–D5: one maturity
  statement with two pointers, prerequisites on both install commands, no
  instruction into a permission error, a worked first-client example to end the
  page, and the two screenshots. **D4 first** — it is the gap; the other four
  are corrections. The dispatch also carries an independent design review of
  RFC 107, which cannot be accepted until it has one.

### Background

The review that produced this RFC walked the documented path rather than
reading it — config generated, `setup` run, server started, `/admin/login` and
`/.well-known/openid-configuration` both `200`. Three of the findings were
confirmed that way. The full review is in `.git-exclude/reviewed/`.
