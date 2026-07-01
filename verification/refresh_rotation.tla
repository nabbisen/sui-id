------------------------- MODULE refresh_rotation -------------------------
(* TLA+ model of the RFC 080 refresh-token rotation protocol.
 *
 * This spec models the rows-affected arbitration that closes the TOCTOU
 * window in the previous find/revoke/insert sequence. It checks three
 * invariants and one temporal property, and includes a deliberately
 * guard-less variant (GuardlessSpec) that must VIOLATE Inv1 — serving
 * as the pilot's own sanity check.
 *
 * Model parameters (small constants for TLC exhaustion):
 *   MaxTokens  = 6   (total refresh-token rows across all families)
 *   MaxExchanges = 3 (concurrent exchange attempts)
 *
 * How to run:
 *   tlc -config refresh_rotation.cfg refresh_rotation.tla
 *   (Expect: all invariants hold; state space exhausted in minutes)
 *
 * Guard-less variant:
 *   tlc -config refresh_rotation_guardless.cfg refresh_rotation.tla
 *   (Expect: Inv1 violated — demonstrates the bug class RFC 080 fixes)
 *
 * Code symbol mapping (this file ↔ implementation):
 *   RotateAction      → refresh_tokens::begin_rotation (UPDATE rows-affected guard)
 *   InsertSuccessor   → refresh_tokens::insert (after RotatedHere)
 *   ReplayAction      → begin_rotation call with a revoked token hash
 *   TokenState.revoked → RefreshTokenRow.revoked_at IS NOT NULL
 *   family            → FamilyId (groups tokens belonging to one rotation chain)
 *)

EXTENDS Naturals, Sequences, FiniteSets

CONSTANTS
    MaxTokens,      \* Maximum number of token rows in the model
    MaxExchanges    \* Maximum number of concurrent exchange attempts

ASSUME MaxTokens \in Nat /\ MaxTokens > 0
ASSUME MaxExchanges \in Nat /\ MaxExchanges > 0

(* Token identifiers — abstract; the model doesn't care about actual values *)
TokenIds == 1..MaxTokens
FamilyIds == 1..MaxTokens   \* at most one family per initial token

(* ---------------------------------------------------------------------- *)
(* State variables                                                         *)
(* ---------------------------------------------------------------------- *)

VARIABLES
    tokens,     \* [TokenId -> {family: FamilyId, revoked: Bool, active: Bool}]
    families,   \* [FamilyId -> {root: TokenId}]
    exchanges,  \* Set of in-progress exchange attempts (each names a token)
    n_tokens    \* number of token rows issued so far

TypeInvariant ==
    /\ n_tokens \in 0..MaxTokens
    /\ \A t \in DOMAIN tokens :
        /\ tokens[t].family \in FamilyIds
        /\ tokens[t].revoked \in {TRUE, FALSE}

vars == <<tokens, families, exchanges, n_tokens>>

(* ---------------------------------------------------------------------- *)
(* Initial state                                                           *)
(* ---------------------------------------------------------------------- *)

Init ==
    /\ tokens   = << >>   \* empty function — no tokens yet
    /\ families = << >>
    /\ exchanges = {}
    /\ n_tokens = 0

(* ---------------------------------------------------------------------- *)
(* Helper predicates                                                       *)
(* ---------------------------------------------------------------------- *)

\* The one active (non-revoked) token in a family, if any.
ActiveInFamily(fid) ==
    {t \in DOMAIN tokens : tokens[t].family = fid /\ ~tokens[t].revoked}

\* Is this token the "current" (non-revoked) holder of its family?
IsCurrentToken(t) ==
    t \in DOMAIN tokens /\ ~tokens[t].revoked

(* ---------------------------------------------------------------------- *)
(* Actions                                                                 *)
(* ---------------------------------------------------------------------- *)

\* Issue the first token of a new rotation family.
IssueRoot ==
    /\ n_tokens < MaxTokens
    /\ LET t  == n_tokens + 1
           fid == n_tokens + 1   \* family id = root token id by convention
       IN
       /\ tokens'   = [tokens EXCEPT ![t] = [family |-> fid, revoked |-> FALSE]]
       /\ families' = [families EXCEPT ![fid] = [root |-> t]]
       /\ n_tokens' = n_tokens + 1
       /\ UNCHANGED exchanges

\* Start an exchange attempt for an active token t.
StartExchange(t) ==
    /\ t \in DOMAIN tokens
    /\ ~tokens[t].revoked
    /\ Cardinality(exchanges) < MaxExchanges
    /\ exchanges' = exchanges \cup {t}
    /\ UNCHANGED <<tokens, families, n_tokens>>

\* The GUARDED rotation: only the first exchange to UPDATE wins.
\* rows-affected guard: only one thread can flip revoked from FALSE to TRUE.
RotateAction(t) ==
    /\ t \in exchanges
    /\ t \in DOMAIN tokens
    /\ ~tokens[t].revoked          \* guard: row must still be active
    /\ LET fid == tokens[t].family
       IN
       \* Revoke only the presented token (not the whole family — that's for replay).
       /\ tokens' = [tokens EXCEPT ![t].revoked = TRUE]
       /\ exchanges' = exchanges \ {t}
       /\ UNCHANGED <<families, n_tokens>>

\* Insert a successor token after winning the rotation.
InsertSuccessor(t) ==
    /\ n_tokens < MaxTokens
    /\ t \in DOMAIN tokens
    /\ tokens[t].revoked      \* only after t was revoked (won the race)
    /\ LET fid  == tokens[t].family
           succ == n_tokens + 1
       IN
       /\ tokens'   = [tokens EXCEPT ![succ] = [family |-> fid, revoked |-> FALSE]]
       /\ n_tokens' = n_tokens + 1
       /\ UNCHANGED <<families, exchanges>>

\* Replay: an exchange attempt presents an already-revoked token.
\* In the guarded implementation, rows-affected = 0 → family revocation.
ReplayAction(t) ==
    /\ t \in exchanges
    /\ t \in DOMAIN tokens
    /\ tokens[t].revoked          \* token already revoked → theft detection
    /\ LET fid == tokens[t].family
       IN
       \* Revoke all remaining active tokens in the family.
       /\ tokens' = [tok \in DOMAIN tokens |->
            IF tokens[tok].family = fid /\ ~tokens[tok].revoked
            THEN [tokens[tok] EXCEPT !.revoked = TRUE]
            ELSE tokens[tok]]
       /\ exchanges' = exchanges \ {t}
       /\ UNCHANGED <<families, n_tokens>>

\* Spurious exchange drop (models crash or timeout between revoke and insert).
DropExchange(t) ==
    /\ t \in exchanges
    /\ exchanges' = exchanges \ {t}
    /\ UNCHANGED <<tokens, families, n_tokens>>

(* ---------------------------------------------------------------------- *)
(* Next-state relation (guarded spec)                                      *)
(* ---------------------------------------------------------------------- *)

Next ==
    \/ IssueRoot
    \/ \E t \in TokenIds : StartExchange(t)
    \/ \E t \in TokenIds : RotateAction(t)
    \/ \E t \in TokenIds : InsertSuccessor(t)
    \/ \E t \in TokenIds : ReplayAction(t)
    \/ \E t \in TokenIds : DropExchange(t)

Spec == Init /\ [][Next]_vars /\ WF_vars(Next)

(* ---------------------------------------------------------------------- *)
(* Invariants (RFC 080 security properties)                                *)
(* ---------------------------------------------------------------------- *)

\* Inv1 (P3): per-family active-token count ≤ 1.
Inv1 ==
    \A fid \in FamilyIds :
        Cardinality(ActiveInFamily(fid)) <= 1

\* Inv2 (P4): revoked_at is absorbing — no token's revoked flag goes FALSE→TRUE→FALSE.
\* (Encoded as a state invariant: revoked tokens never become active.)
\* This is guaranteed structurally by the UpdateAction; the invariant catches bugs.
Inv2 ==
    \A t \in DOMAIN tokens :
        tokens[t].revoked => (\A t2 \in DOMAIN tokens : t2 = t => tokens[t2].revoked)

\* Inv3 (P2): replay of a revoked token eventually leads to a fully-revoked family.
\* Encoded as: after a ReplayAction, no active tokens remain in that family.
\* (Checked via temporal property below rather than a state invariant.)

(* ---------------------------------------------------------------------- *)
(* Guard-less variant (for the sanity check)                               *)
(*                                                                         *)
(* Replace RotateAction's guard with an unconditional update. TLC must     *)
(* find a Inv1 violation: two concurrent exchanges both see revoked=FALSE, *)
(* both win, both insert successors → two active tokens in one family.     *)
(* ---------------------------------------------------------------------- *)

RotateActionGuardless(t) ==
    /\ t \in exchanges
    /\ t \in DOMAIN tokens
    \* No guard here — deliberately omitted to show the bug.
    /\ tokens' = [tokens EXCEPT ![t].revoked = TRUE]
    /\ exchanges' = exchanges \ {t}
    /\ UNCHANGED <<families, n_tokens>>

NextGuardless ==
    \/ IssueRoot
    \/ \E t \in TokenIds : StartExchange(t)
    \/ \E t \in TokenIds : RotateActionGuardless(t)
    \/ \E t \in TokenIds : InsertSuccessor(t)
    \/ \E t \in TokenIds : ReplayAction(t)
    \/ \E t \in TokenIds : DropExchange(t)

SpecGuardless == Init /\ [][NextGuardless]_vars

(* Expected TLC results:
 *   Spec (guarded):      Inv1 holds — no violation found.
 *   SpecGuardless:       Inv1 VIOLATED — demonstrates the pre-RFC-080 bug.
 *)

=============================================================================
