# 096-B1 stage 14 — the live run against Google: a runbook

> ## Deferred 2026-11-14, to `0.91.0`'s cycle.
> `@nabbisen`: *"Authorized. Defer it. However, it should be controlled properly in
> the schedule."* `0.90.0` ships without it. **It now runs on `0.91.0`'s prepared
> commit, before `0.91.0`'s tag**, and that tag waits for its record unless a
> further deferral is recorded. M4-B cannot close, and M6 cannot enter, without
> it (ROADMAP, cycle L's row). **§1's commit below is superseded**: the architect
> writes `0.91.0`'s prepared commit there when it lands.
>
> **Prepare before the slot comes**, so the run is not blocked on setup:
> - the Google OAuth client of §2, with its client ID ready to hand over
>   (the secret stays with you);
> - if the architect is to drive the admin-console steps, the Claude in Chrome
>   extension, installed, running, and signed in to the same account.
>
> Even in a split run, three parts stay yours: creating the client, starting
> the server with the secret in your own terminal, and the two Google
> sign-ins (steps 7 and 10).

**What it proves.** RFC 096 `:1088-1089` asks for *"one representative
public upstream integration using owner-supplied test credentials outside
CI, with secrets absent from artifacts."* This is the first time sui-id
checks a provider against a real upstream, enables it, and signs someone
in through it. Every run so far used mocks.

**What it does not prove.** *"A live happy path does not substitute for
hostile-provider evidence"* (`:1093-1094`). That is 096-A's corpus, and
096-C's.

## 0 — safety, before anything

- **Use a throwaway instance:** a new directory, with its own database and
  key file. **Never point this at a sui-id you use.**
- **The client secret lives only in your terminal's environment.**
  - Type it with `read -rs`, never on the command line, so it stays out of
    shell history.
  - Never write it into a file. The config only *names* the variable.
  - **Never paste it to the architect, or anywhere else.**
- **Bind to loopback only**, `127.0.0.1:8801`. Nothing outside your machine
  can reach it.
- **Prefer a dedicated test Google account.** Its email becomes a local user
  in the throwaway database, and nowhere else.
- **Afterwards,** stop sui-id, delete the directory, and `unset
  GOOGLE_CLIENT_SECRET`. Keep or delete the Google OAuth client as you wish;
  it is yours alone.

## 1 — the commit

0.90.0's preparation landed as
**`1955bc5305d06e575cac344e233c3f97bf4d14e6`** (2026-11-13). `main` is one
commit past it: the commit that wrote this hash here, which changes
nothing but this file. So check out the exact commit, detached, and build
that from a clean tree:

```sh
cd <your sui-id checkout>
git status --short                                   # must print nothing
git fetch origin
git checkout --detach 1955bc5305d06e575cac344e233c3f97bf4d14e6
git log -1 --format=%H    # must print 1955bc5305d06e575cac344e233c3f97bf4d14e6
cargo build --release -p sui-id --locked
```

Afterwards, `git checkout main` returns you to the branch.

## 2 — a Google OAuth client

In Google Cloud Console, create an **OAuth client ID**, type **Web
application**, with exactly this **Authorized redirect URI**:

```text
http://localhost:8801/auth/federated/callback
```

Note the **client ID**. The **client secret** stays in the console until §4.

## 3 — the throwaway configuration

```sh
mkdir ~/sui-id-live && cd ~/sui-id-live
<checkout>/target/release/sui-id --print-sample-config > sui-id.toml
```

Edit `sui-id.toml`:
- `issuer = "http://localhost:8801"` (the name must match the redirect URI
  above exactly);
- keep `listen_addr = "127.0.0.1:8801"` and `cookie_secure = false`;
- point `db_path` and `key_file` into this directory;
- **delete** the line `federation_providers = []`;
- append the Google block from the configuration reference's example
  (`docs/src/reference/configuration.md`, "`[[federation_providers]]`"),
  with your client ID, `client_secret_env = "GOOGLE_CLIENT_SECRET"`, and
  `provision_mode = "provision_on_first_login"`. Keep its `allowed_origins`
  exactly as the example gives them: this run checks them too.

Then, **before starting anything**, run the upgrade guide's own pre-check:

```sh
<checkout>/target/release/sui-id preview-upgrade --config sui-id.toml
```

**Expected:** no configuration error.

## 4 — start it

In the same terminal:

```sh
read -rs GOOGLE_CLIENT_SECRET && export GOOGLE_CLIENT_SECRET
<checkout>/target/release/sui-id --config sui-id.toml 2>&1 | tee server.log
```

## 5 — the steps, and what each should show

Record **PASS** or **FAIL** for each, and on a FAIL, the exact text the page
showed.

| # | Do | Expected |
|---|---|---|
| 1 | Watch the startup log | one `WARN` naming the provider and saying to visit `/admin/federation-providers` and press "Check and enable" |
| 2 | Open the printed setup URL, and create the admin | the admin console |
| 3 | **Admin → Federation providers** | one row: Google, Disabled, Requires review, "—" for the check column, "No" linked users |
| 4 | **Check only** | "Every check passed." |
| 5 | **Check and enable** | "The provider was checked and enabled.", and the row Enabled, Validated, with a time |
| 6 | Sign out. Open the login page | a **"Sign in with Google"** button |
| 7 | Press it, and sign in with the test Google account | back at sui-id, signed in as a new user, on that user's own security page (`/me/security`) |
| 8 | Sign in as the admin. **Users →** that user | a *Federated sign-in* section with Google |
| 9 | **Unlink**. Read the confirm page, then confirm | the page says the unlink cannot be undone, and that open sessions stay valid until the account is disabled |
| 10 | Sign out, and press "Sign in with Google" again | refused on the login page with "This sign-in could not be linked to a local account automatically. Contact an administrator." It is the same message whether or not a local account exists, by design, so the page cannot be used to learn who has an account |
| 11 | **Federation providers → Disable** | "The provider was disabled."; the login button is gone |
| 12 | Stop sui-id, remove the Google block from `sui-id.toml`, start again, then **Delete** | the confirm page states how many users' sign-in ends; then the row is gone |

## 6 — what to send back

Into a new file, `.git-exclude/live-runs/096-b1-stage14-<date>.md`:
- the commit from §1, and `sui-id --version`;
- the table above, with PASS or FAIL and, for a FAIL, the page's exact text;
- **from `server.log`, only the lines at `WARN` or `ERROR`**.

**Before saving, read every line you paste, and remove:**
- any URL containing `code=` or `state=`;
- any token, cookie or secret;
- your Google account's email address.

The architect reads the file, fixes what it finds, and runs the release gates
again before the tag. **Nothing from this run goes into git** except the
architect's summary of it.
