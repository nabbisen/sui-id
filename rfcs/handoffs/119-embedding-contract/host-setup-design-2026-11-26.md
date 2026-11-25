# How a host sets up an embedded sui-id — the design

**Written.** 2026-11-26, by the architect, at `@nabbisen`'s request: *"Review them carefully and design in a considerate
manner with time you need with my fundamental philosophy"*. Measured at `4070e73`.
**What it is.** The companion RFC 119 lacks. RFC 119 says what a host must *promise* (H1–H8). This says how a host *sets the
module up*, and what the module's API looks like to it. **A design for review, not a decision**, and nothing here is built:
- embedding is frozen (ROADMAP, decision 1);
- the split's RFC is not written;
- RFC 119's D7 enumeration has not been done.

**The test applied to every choice**, from the philosophy's two halves:
1. **Is the safe use the only use the API allows?** Not merely the documented use. This is RFC 119's own line: *a leak the
   host cannot commit beats a leak the host is told not to commit.*
2. **Can a host developer, or the operator who runs the host, misread what they configured?** Every choice whose wrong value
   fails silently is either removed, or made a required, typed and named decision.

---

## 0 — the finding that shapes everything: today's crates are not a boundary

Before designing what a host *should* call, consider what it *can* call. `sui-id-store` is published on crates.io:
- `pub mod repos` exposes its writers, including ones documented as *"Test-seeding only — no production caller"*
  (`repos/server_settings.rs`, `update_idle_session_timeout`). They change data **with no audit row**;
- `Database::with_conn` and `with_tx` hand out a raw `&rusqlite::Connection`;
- inside this workspace, write authority is held by lane G23, which reads the workspace sources. **A crate outside the
  workspace is not read by anything.**

So **"take core + store" today gives a host unaudited write access to every table.** That is the opposite of RFC 094's Class-A
guarantee, and it would be silent.

The service itself reaches past core into the store's repositories **161 times** (`sui_id_store::repos` in `crates/sui-id/src`).
At least ten of those are direct writes (`users::create`, `federation_link::upsert`, `credentials::upsert`, and others).
**RFC 119's D5 requires the service to use exactly the published interface.** So every one of those becomes either a command
or a read query on the module's surface, or moves inside it.

**Decision S0 (proposed): the module's public surface is commands and queries, nothing else.**
- Repositories, `with_conn`, `with_tx`, models with writable fields and test-seeding writers become crate-private.
- Test seeding goes behind a `test-support` feature, which the store already has for its guard clock.
- A **compile-fail test** (trybuild, which the project already uses) proves an outside crate cannot name a repository
  writer. **Mutation:** make one `pub` again, and the fixture compiles.

**This is the largest single piece of the split, and it is a precondition for every other row below.**

## 1 — the shape: one handle, opened once, with every decision named

```rust
let suiid = sui_id::embed::Module::open(
    sui_id::embed::Setup {
        contract: ContractVersion::V1,                  // §11: required, so a contract change is a compile error
        storage:  Storage::Sqlite { path: "/var/lib/app/sui-id.sqlite".into() },   // §2
        key:      MasterKeySource::File("/var/lib/app/sui-id.key".into()),         // §4
        migrations: MigrationPolicy::Apply { backup_before: true },                // §5
        client_address: ClientAddressPolicy::FromHost,                             // §8
        mail:     Mail::Host(Arc::new(app_mailer)),                                // §9
        identity: Identity { issuer: "https://app.example/auth".parse()? },        // §6
    },
).await?;
```

Every field is **required, with no `Default`**, and each is an enum whose variants name the choice.

**Why so strict.** P2 made master-key custody a required flag for one reason: an operator who never chose had silently
accepted the most dangerous option. The same reasoning applies to every row below, and to a developer rather than an
operator.

`open`:
- takes the liveness guard (§3);
- runs crash recovery (§4);
- applies or refuses migrations (§5);
- starts the module's own background work (§10).

It returns a `Module` that holds all of them. **Dropping it releases them.** Nothing a host must remember to start or stop
exists outside the handle.

## 2 — the database: named by the host, opened by the module, in a namespace of its own

### The accessor: the host names the database; it never hands the module a connection

```rust
enum Storage {
    Sqlite     { path: PathBuf },                         // a file of sui-id's own
    Postgres   { url: SecretUrl, schema: SchemaName },    // RFC 009; a schema of sui-id's own
    // MariaDB / MySQL: the same shape, once RFC 009 stage 5 lands
}
```

**Why not accept the host's pool or connection**, which looks friendlier:
- **D3.** A connection the host also uses is one the host can open a transaction on. The audit row's atomicity rests on sui-id
  owning the transaction boundary.
- **The settings the guarantees rest on are per connection.** WAL, `foreign_keys = ON`, the busy timeout, PostgreSQL's
  isolation level, and the read-only transactions behind `ReadConn` are all set on sui-id's own connections. A host's
  connection with different settings would break a guarantee and say nothing.
- **The liveness guard** (§3) is held for as long as the module's own connections live, which a borrowed pool does not tell
  it.

**The module creates its own pool. The host's only input is where.** This is the database accessor your model asks about,
deliberately one-way.

### The namespace: sui-id's tables have names of their own, everywhere

**Revised 2026-11-26** on `@nabbisen`'s clarification: *"I meant tables which sui-id uses possibly should have unique names."*

**Measured: they do not.** Of sui-id's 31 tables, only `sui_meta` is namespaced. The rest are generic names a host is likely to
have itself: `users`, `sessions`, `clients`, `credentials`, `consents`, `audit_log`, `server_settings` and more. In any database
the two share, that is a collision waiting to happen. Depending on order, it shows up as a failed migration, as the host's
table read as sui-id's, or as a host migration tool altering a table it believes is its own.

**Decision N1 (proposed): every sui-id table carries one fixed prefix, `sui_id_`, built into its name.** For example
`sui_id_users`, `sui_id_sessions` and `sui_id_meta`.
- **Fixed, not configurable.** The objection the first draft raised against a prefix was to a *configurable* one: a string
  spliced into every statement at run time, which is dynamic SQL and an injection surface. **A fixed prefix is part of the
  table's name**, written once into each static statement, exactly as `users` is today. Nothing is spliced and nothing can
  change at deploy time.
- **The same names everywhere**: the service, every embedding, and every backend. An operator who learns `sui_id_users` once
  finds it in every deployment. No deployment can have a different name, so no documentation or tool can be wrong about it.
- **When.** It touches every statement and every migration name, so it belongs in **RFC 009's stage 1**, which rewrites every
  statement anyway. Done there, it costs one migration that renames the 31 tables. SQLite's `ALTER TABLE … RENAME TO`
  rewrites the references to the renamed table (foreign keys, indexes, triggers), and the review re-measures that on the
  bundled version, it is not assumed. Done separately, it costs a second pass over everything.

**On PostgreSQL, a schema of sui-id's own as well** (`sui_id`), with its own role. The prefix gives unique names; the schema
adds what SQLite cannot: a host query that touches sui-id's tables by mistake is refused by the database. That defends
against the host's bugs, not against a malicious host, which RFC 119 Q2 settled no in-process design can stop.

### One SQLite file, shared with the host: supported, as a named choice

**Revised 2026-11-26.** The first draft refused it, and `@nabbisen` asked why. **The honest answer: it was a default the
architect chose to avoid four facts of today's code, not a principle.** Each fact has a remedy:

| Today's fact (measured) | Why it blocked a shared file | Remedy |
|---|---|---|
| **The table names are generic** | They collide with the host's | N1's fixed prefix |
| **RFC 112's refusal counts every table.** `check_database_file` refuses a file with *any* table besides `sui_meta` but no schema version (`migrations.rs`, `name <> 'sui_meta'`) | A host's file is refused as *"a populated database without a schema version"* | The check counts **sui-id's own (prefixed) tables only**. A host's tables are not sui-id's business. A file holding `sui_id_*` tables without `sui_id_meta` is still refused, as now |
| **`open` sets `journal_mode = WAL` on the file** (`db.rs`) | WAL is a property of the file, so the module would silently change the host's journal mode. A host that backs up by copying the `.db` file alone would then miss the `-wal` file's contents | **The module never changes a file-wide setting on a file it did not create.** On a shared file it requires WAL and refuses otherwise, naming the setting and why. It sets WAL only on a file it creates |
| **One writer per SQLite file** | The host's long write transactions delay sui-id's sign-in commits, and the reverse | Not removable. It is SQLite. **It is not silent**: a commit that waits past the busy timeout fails with an error. The contract states the cost: keep the host's write transactions short. `status()` reports lock waits |

**And one thing gets better.** In a shared file, **one backup of the file is atomic across the host's data and sui-id's**,
which is exactly what peisear's deployment story needs. The backup unit becomes the host's file plus sui-id's key. So the
module offers `Module::backup_key_to(…)`, and the guide states plainly that **the database half of the backup is the host's
whole file**.

**Revised again, 2026-11-26**, on `@nabbisen`'s model: *"which kind of storage which an app uses dose not matter when our
service embedded. All we have to do is to get the information to connect to it and add tables there ?"* **Yes.** So there is
one variant per database engine, and none for "shared" or "own":

```rust
enum Storage {
    Sqlite   { path: PathBuf },       // any file: the host's own, or one only sui-id uses
    Postgres { url: SecretUrl },      // any database: sui-id adds schema `sui_id` there
    // MariaDB / MySQL with RFC 009 stage 5
}
```

The module connects, and adds its `sui_id_*` tables (and on PostgreSQL its schema) beside whatever is there. **Whether the
host shares the database is not a setting**, because no rule depends on it. The rules that used to:
- **File-wide settings.** The module changes a file-wide or database-wide setting **only on a database it created**. On one that
  already existed, it checks what it needs, and refuses with a sentence naming the setting and why, never changing it. On
  SQLite that is `journal_mode = WAL`.
- **Backups.** `Module::backup_to(dir)` writes the key, and on SQLite the whole file, through SQLite's backup API, never a raw
  copy of a live file. **The whole file is the unit, shared or not**, so a shared file's backup is atomic across the host's data
  and sui-id's. On PostgreSQL the database half is refused with the native tool named (RFC 009 row 10), and the key is still
  written.

**Two limits on "the kind does not matter"**, stated so neither is a surprise:
1. **The engine must be one sui-id supports.** That is SQLite today, and PostgreSQL then MariaDB/MySQL through RFC 009. A host on
   anything else gives sui-id a database of its own, of a supported kind. **`open` refuses an unsupported engine by name**; it
   never half-works.
2. **The settings the module checks** (above) must hold. A database that fails one is refused at `open` with the reason, never
   adjusted behind the host's back.

**What a shared file still cannot do**, stated rather than hidden: SQLite has no roles. A host bug that writes `sui_id_*`
tables directly is not refused by the database, as it would be on PostgreSQL. `PRAGMA foreign_keys` is set per connection, so
the host's connections may run without it. Both are host obligations under the contract. Neither is fixable inside SQLite.

## 3 — the liveness guard: taken by the store's own open, keyed on the database

From peisear's finding of 2026-11-26, and RFC 009 row 8:
- **`open` takes the guard shared, for the life of the `Module`.** It is keyed on the database, not on the key file: a lock file
  beside the SQLite file, or an advisory lock or lease row on PostgreSQL.
- **Rotation takes it exclusive and refuses beside any open handle**, the service's or any host's, through the same code.

The service is one more opener of the same store. **No embedder can forget the guard, because the guard is not something it
calls.**

## 4 — the master key

```rust
enum MasterKeySource {
    File(PathBuf),            // the module owns custody: the startup check, recovery, rotation
    Provided(MasterKey),      // from the host's KMS or secret store; rotation through the module is refused
}
```

- **The same two sources the service has today**: the key file, and a key given as a value, as `SUI_ID_MASTER_KEY` is.
  They follow **the same rule**: a provided key cannot be rotated by the module, exactly as an environment key cannot today.
  A host learns no new concept, and an operator moving between the service and an embedding meets the same rule.
- `File` brings RFC 100's whole custody: the startup key check, crash recovery at `open`, and the custody flags on rotation.
  **That custody is what peisear found on the wrong side of the line, and this moves all of it inside.**
- **Rotation is not a method on `Module`.** It is `sui_id::embed::rotate_master_key(storage, key_file, custody)`, a separate
  entry point that needs no open handle and refuses while one exists anywhere (§3). That makes "rotation is offline" a
  property of the API, not a sentence in a guide.

## 5 — migrations: a required, named choice

```rust
enum MigrationPolicy {
    Apply { backup_before: bool },   // what the service does today, with its pre-migration backup
    Refuse,                          // a host with its own deploy step: open fails, naming the pending migrations
}
```

- **Required, because both defaults are wrong for someone.** Applying silently surprises a host whose migrations run as a
  controlled deploy step. Refusing silently surprises a host that expected it to just work.
- **Either way, a database newer than the build is refused** (RFC 112), and nothing is written to it.
- `sui_id::embed::migrate(storage, key, backup_before)` runs the same migrations as an explicit step, for the `Refuse` host.

## 6 — identity: what the host stores, and the `sub` question

**What the host keys on: `sui_id::UserId`.** It is a random UUID, opaque, fixed at creation, and never reused: a deleted user
is kept as a deleted row. The host's own tables reference it, for example `app_users.suiid_user`.
- **Never the username.** It can be anything an administrator typed, and a federated user's is derived from the upstream.
- **Never the email.** It is unverified until RFC 101 ships.

**`sub`**: in an embedding that serves OpenID Connect (contract V2, below), `sub` is that same `UserId`, unprefixed, so the
surface stays indistinguishable from the standalone one. OpenID Connect already namespaces `sub` by issuer. *(The first draft
read "`sub` prefix" as a prefix on the claim; `@nabbisen` clarified that he meant unique table names, which §2's N1 now
answers.)*

## 7 — the per-request context: H1, H2, H3 and H6 carried by types, not promises

Every module call that serves a person takes one argument the host cannot build wrongly:

```rust
struct RequestContext {
    client: ClientAddress,     // H3
    attempt: AttemptId,        // H6: stable across the host's own retries (RFC 119 Q1)
    session: Option<SessionToken>,
}
```

and every state-changing call **also takes a `CsrfProof`**, which only the module can construct, from a request it verified.

- **H2 (CSRF).** The host cannot call `change_password` without a `CsrfProof`, and cannot make one except through
  `Module::verify_csrf(&request_parts)`. **A host that forgets CSRF gets a compile error, not a vulnerability.** This is
  D4 applied to the one obligation peisear could not meet: CSRF moves into the module, and peisear's gap closes on our side.
- **H1 and H4 (session fixation, cookies).** A successful sign-in returns a **new** `SessionToken` and a `SetCookie` value
  whose attributes the module fixes (`HttpOnly`, `SameSite`, `Secure` from `Identity`). The host writes it verbatim. **The
  design recommends the module's session as the host's authenticated session.** Then H1 holds by construction, and the host
  never attaches a user to a pre-authentication session of its own.
- **H7's message half.** Outcomes are typed so that the distinction the property forbids does not exist. `SignInOutcome`
  has `Signed { … }` and `Refused`, and no variant for "no such user". The host cannot reproduce a difference it is never
  given. **The timing half stays a host obligation the suite can only probe**, as the reply to peisear said, and the contract
  says so in those words.

## 8 — the client address

```rust
enum ClientAddressPolicy {
    FromHost,     // every RequestContext carries ClientAddress::Known(addr)
    Undeclared,   // the host cannot know it: per-address limits off, per-account limits on, and the module reports it
}
```

**Chosen at `open`, not per request.** A host that mixes the two would have per-address limits that mean nothing. With
`Undeclared`, the module disables per-address limiting **explicitly**, logs that at `open`, and exposes it in its status. It never
shares one bucket among everyone, which would let one client throttle every user.

## 9 — outbound calls: the host may provide transport, never policy

- **Mail: the host may provide the sender** (`Mail::Host(Arc<dyn MailSender>)`; the trait exists in `sui-id-core`), or use the
  module's SMTP. **The module composes every message**, including the links, from `Identity.issuer`. The host transports
  bytes. A failure to send is an error the module sees, not a silent one.
- **HIBP and federation: the module's own clients, never the host's.** The egress rules (lane G19, every DNS answer checked,
  private addresses refused) are what stop an upstream URL from reaching the host's internal network. A host HTTP client
  would carry none of them, silently.
- **The clock is the module's own.** A `Clock` trait exists, for tests. **A host-provided clock is not offered in
  production**: H5 says a skewed clock frees a locked account or strands a valid one, silently. So it stays behind the
  `test-support` feature, and H5 stops being an obligation.

## 10 — background work: started by `open`, stopped by drop

The service starts three things beside its listener (`cli/handlers.rs`, `runtime/gc.rs`):
- the mail outbox worker (`OutboxWorker`);
- the forgot-password worker (`ForgotPasswordWorker`);
- the 15-minute cleanup task.

**If a host had to remember to start them**, mail and reset messages would never be sent, and nothing would say so. So `open` spawns them on the host's tokio runtime, and dropping the
`Module`, or `Module::close().await`, stops them.
- **tokio is a stated requirement of contract V1.** The runtime is required, not an optional integration.

## 11 — versions: the contract is a type

- **`ContractVersion::V1` is a required field.** A breaking contract change adds `V2`, and the host's build fails until its
  developer reads what changed. RFC 119's D1 (*an embedding that does not state which version it satisfies is unsupported*)
  becomes a compile-time fact, not a claim.
- **Semver** applies to the `embed` module's API. Nothing outside `embed` is public API for embedders (§0).

## 12 — what an operator of the host sees

- **One status call**, `Module::status()`. It reports:
  - the storage backend and its namespace;
  - the key source;
  - pending migrations;
  - the client-address policy (§8);
  - an interrupted rotation, if one was recovered;
  - a kept backup past its date (the kept-backup warning).

  The same facts the service writes to its startup log, given to the host to show in its own console.
- **Backups**: `Module::backup_to(dir)`, as §2 gives it: the key always, and on SQLite the whole file through SQLite's
  backup API. No host copies a live SQLite file by hand.
- **`tracing`** events under the target `sui_id`, so a host's logging picks them up with no glue.

---

## 13 — what this design asks of other work

| Where | What |
|---|---|
| **RFC 009, stage 1** | The namespace (§2) is part of stage 1's design: a schema per backend, a file for SQLite, set as a connection setting. The liveness guard (§3) moves into the store's own `open` |
| **The split's RFC** (unwritten; no number) | §0's narrowed surface, then this design. §0 is a precondition for everything else |
| **RFC 119** | Its enumeration gains: master-key custody (peisear, 2026-11-26), the surface (§0), and the setup decisions (§1–§12). H2, H4 and H5 can then move from host obligations into the module, as above, which is D4's own direction |
| **The freeze** | Every build step here needs your exception. Designing and reviewing it does not |

## 14 — the owner's answers (2026-11-26), and what remains open

1. **Unique table names.** `@nabbisen`: *"I meant tables which sui-id uses possibly should have unique names."* Designed as
   N1 (§2): one fixed prefix, built into every name.
2. **An embedding that serves OpenID Connect: later.** `@nabbisen`: *"Later - accepted. However, should be recorded and
   controlled on schedule."* **Recorded as contract V2**, with entry conditions:
   - V1 has shipped;
   - its own freeze exception is granted;
   - planeter's clause is in it: an embedding's OpenID Connect surface passes the standalone's conformance suite.

   It goes on the ROADMAP's schedule as an unscheduled item with those conditions, so it is controlled, not forgotten.
3. **Who an administrator is.** `@nabbisen`: *"Sounds good. Any other option(s) ?"* The options, against both halves:

   | Option | What the audit names | Step-up (RFC 102) | Verdict |
   |---|---|---|---|
   | **A. The host's administrators are sui-id users** with the administrator role | a sui-id person | **works**: they re-authenticate as themselves | **Recommended** |
   | **B. A system principal acting for the host** | "the host", one principal for everyone | **impossible**: a principal has no credential to re-present, so every action that requires step-up would need an exemption | **Only for automation**, such as a provisioning job, and only for commands already marked `SystemPrincipalPermitted`. Never for a person |
   | **C. One service account, plus the host's own administrator id passed as an attribute** | a host-asserted name, recorded beside the service account | as B | **Rejected.** The recorded name reads like a verified identity and is not. An audit line that invites that misreading fails half 2. The honest version of C is B with no name at all |
   | **D. No administration in the module**: administrators use sui-id's own console or CLI against the same database | a sui-id person | works | **Available beside A, not instead of it.** It keeps sui-id's tested pages, but the host's own UI cannot manage users. Running the console needs the service alongside, which the liveness guard allows (a second shared holder) |

   **Recommended: A for people, B narrowly for automation, D as an option.** C is rejected.
   **Accepted**, `@nabbisen`, 2026-11-26: *"Your recommendation is accepted."*
4. **One SQLite file.** `@nabbisen`: *"It seems what we have to consider. Why is there such strange limitation ?"* **It was
   the architect's conservative default, not a principle.** §2 now supports it, with no setting for it at all: the host gives
   connection information and the module adds its tables there. The four facts that blocked it each have a remedy, and the one
   real cost (one writer per SQLite file) is stated.

5. **Wide database support.** `@nabbisen`, 2026-11-26: *"In my mental model, we should not force SQLite limitation.
   Instead, we should provide wide support about database kinds. Any good and considerate solution(s) ?"* The architect's
   proposal is §15.

**Still open, for the review:** whether N1's rename runs in RFC 009 stage 1, as proposed, or on its own; and whether
the PostgreSQL schema name should ever be the host's to choose. The architect's view is no, for N1's reason: a name that can
differ per deployment is one a tool or a guide can get wrong.

## 15 — wide database support, without delegating the guarantees

**The goal**, in `@nabbisen`'s words: not to force SQLite's limitations, and to support many kinds of database. **The
constraint** is the philosophy's first half. Every guarantee in RFC 009's §2 (the audit row atomic with its effect, the
chain that never forks, read-only handles, the floor checks in the schema) must hold on each kind. A kind on which one
silently fails is worse than no support for it.

**The options:**

| | How breadth is reached | What holds the guarantees | Verdict |
|---|---|---|---|
| **A. A backend per engine**, in sui-id | sui-id implements each engine | sui-id, proven per engine by one shared conformance suite in CI | **Recommended**, as the only way a guarantee is sui-id's own on every kind |
| **B. A storage trait the host implements** for its own database, any kind including document stores | unlimited | **each host's adapter**: atomicity, locking, the audit chain | **Rejected.** Every one of those fails silently when wrong, and RFC 119's D4 forbids delegating a silent failure. A conformance suite could probe an adapter, but not prove its transaction semantics |
| **C. One generic SQL layer** (an ORM's "any" driver) | whatever the library supports | the library's lowest common denominator, which hides exactly the differences the guarantees rest on (isolation, locking) | **Rejected as the mechanism.** A query builder is useful *inside* A (below) |
| **D. sui-id's own database of a supported kind, beside the host's** | every host, whatever it runs | sui-id, as in A | **Kept, as the fallback that turns no host away** |
| **E. sui-id as a separate service** | every host | sui-id | exists today; planeter's choice |

**The proposal is A with D. It reaches breadth through four parts.**

1. **A stated capability list decides which kinds can be supported at all.** It is a principle, not a list of favourites. A
   database engine can host sui-id natively only if it has:
   - multi-statement ACID transactions;
   - a lock usable for the audit chain's head and the liveness guard (a row lock or an advisory lock);
   - enforced `CHECK` constraints, or an equivalent the schema can state;
   - read-only transactions;
   - byte-exact binary storage for sealed columns.

   SQLite, PostgreSQL, MariaDB and MySQL (whose `CHECK`s are enforced from 8.0.16) appear to meet it. SQL Server and others
   may: **each is measured when it is proposed, never assumed.** A document store does not, so it gets D. That is the honest
   boundary, and it is a property of the engine, not a limitation sui-id chose.
2. **The engine layer is small, so each new kind is cheap.** Shared code keeps every guarantee's logic: Class A, the chain,
   the registry. An engine supplies only:
   - its SQL, generated where possible by one query builder from one definition (`sea-query` is a candidate that targets
     SQLite, PostgreSQL and MySQL; RFC 009's §4 criteria decide it);
   - its lock primitive;
   - its read-only transaction;
   - its migrations;
   - its way to make a real commit fail, for the rollback evidence.

   That is RFC 009's stage 1 seam, sized so the engine part stays a short list.
3. **One conformance suite** carries RFC 009's eleven guarantees and runs against every engine in CI. **An engine is
   supported exactly when it passes.** Adding a kind is then a mechanical route anyone can follow, including a contributor:
   - implement the short list;
   - add a CI service;
   - pass the suite;
   - add the threat-model boundary.
4. **Support levels have names, so nobody misreads them.**
   - **Supported**: in CI, suite green, every guarantee proven.
   - **Not supported**: `open` refuses the engine by name, and says to use D.

   There is **no "experimental" or "best effort" level**. A guarantee that might hold is a guarantee an operator cannot rely
   on, which half 2 forbids.

**The order follows RFC 009**: SQLite, then PostgreSQL, then MariaDB/MySQL, then by demand (SQL Server is the likeliest next).
Each kind is one stage, and each stage needs the freeze exception.
