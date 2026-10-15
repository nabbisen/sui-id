-- Migration 0048: at most one *active* federation_mfa_pending row per
-- user (RFC 096-B1 stage 11b, RFC 096 :756)
--
-- RFC 096 `:756`, in the paragraph describing this table: "A restart
-- preserves count/status; it grants no reset." Before this migration,
-- the table had no constraint tying rows to one another by `user_id` at
-- all -- every `INSERT` (F02, `issue_federated_mfa_pending`) created a
-- brand-new row unconditionally, so a federated sign-in restarted while
-- an existing continuation sat at `failure_count = 4` produced a
-- *second* row at 0, with its own independent five-guess budget. Once
-- stage 12 wires a handler, that is the five-attempt cap on a federated
-- second factor, bypassable by restarting the upstream sign-in
-- indefinitely -- exactly the brute force the cap exists to stop.
--
-- `:756`'s own sentence is about the row's brute-force-relevant state,
-- not every status equally: the two values a restart must not be able
-- to escape are `pending` (a budget still in use) and `exhausted` (a
-- budget already spent) -- not `completed` (already granted its
-- session; nothing left to protect) or `invalidated` (ended for a
-- reason that was never a wrong guess at all -- authority drift, or a
-- plain 300-second timeout, the same thing this migration would turn
-- into a de-facto lockout if it were made sticky too). So the new
-- constraint is a *partial* unique index, scoped to exactly that pair --
-- the same "exclude what the invariant does not apply to" shape
-- migration 0019's `idx_refresh_tokens_token_hash` and migration 0020's
-- `idx_users_user_uuid` already use for an unrelated reason (there,
-- rows not yet backfilled; here, rows the brute-force concern does not
-- reach).
--
-- `repos::federation_mfa_pending::find_active_by_user_id_within_tx`
-- reads with this exact predicate, so what the index enforces and what
-- the code reads agree by construction rather than by two authors
-- remembering to keep two strings in sync.
--
-- § 1: deduplicate existing data before the index can be added ---------
--
-- An existing database may already hold more than one `pending`/
-- `exhausted` row for the same user -- this table has been shipping
-- since migration 0047 (stage 8), and nothing before this migration
-- ever refused a second one. For each `user_id` with more than one
-- such row, keep exactly one and delete the rest:
--
--   1. `exhausted` outranks `pending` -- if any row for this user
--      already recorded the cap being spent, that is the state RFC 096
--      `:756` says a restart must not be able to undo, so it survives
--      over a `pending` row that has not (yet) reached it.
--   2. Within the same status, the higher `failure_count` survives --
--      closer to what a single, correctly-restart-preserving history
--      for this user would have recorded.
--   3. Within a tie on both, the most recently created row survives --
--      an arbitrary but deterministic tiebreak, not a claim that
--      recency carries any security meaning here.
--
-- Deleting a losing row cascades its `federation_webauthn_ceremony`
-- row, if one exists, through that table's own `ON DELETE CASCADE`
-- (migration 0047) -- no separate cleanup needed.
--
-- `completed`/`invalidated` rows are untouched: they were never in
-- scope for the new index (the `WHERE status IN ('pending',
-- 'exhausted')` clause on both the `DELETE` below and the index itself
-- agree), so no number of them for one user is a conflict to resolve.

DELETE FROM federation_mfa_pending
WHERE status IN ('pending', 'exhausted')
  AND id NOT IN (
      SELECT id FROM (
          SELECT id,
                 ROW_NUMBER() OVER (
                     PARTITION BY user_id
                     ORDER BY
                         CASE status WHEN 'exhausted' THEN 0 WHEN 'pending' THEN 1 END ASC,
                         failure_count DESC,
                         created_at DESC
                 ) AS survivor_rank
          FROM federation_mfa_pending
          WHERE status IN ('pending', 'exhausted')
      )
      WHERE survivor_rank = 1
  );

-- § 2: the constraint itself ---------------------------------------------

CREATE UNIQUE INDEX IF NOT EXISTS idx_federation_mfa_pending_one_active_per_user
    ON federation_mfa_pending(user_id)
    WHERE status IN ('pending', 'exhausted');
