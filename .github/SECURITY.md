# Security policy

sui-id handles authentication, so security reports get a real response.

## Reporting a vulnerability

If you believe you have found a security issue in sui-id:

1. **Do not file a public GitHub issue.** Public issues become indexable
   immediately and put other operators at risk.
2. **Open a [private security advisory](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing/privately-reporting-a-security-vulnerability)**
   on the repository, or email `nabbisen@scqr.net` .
3. Include enough information for the maintainers to reproduce: version
   (`sui-id --version` once that flag exists; for now the git SHA),
   configuration, and a minimal example.

## What you can expect

- An acknowledgement within a small number of days.
- A discussion of severity and timeline before any public disclosure.
- Credit in the changelog and security advisory unless you ask for
  anonymity.

## What's in scope

Anything that allows:

- Authentication bypass.
- Privilege escalation between users, between users and admins, or from a
  client to an admin.
- Token forgery or unauthorised acceptance of a token issued by sui-id.
- Reading or writing the encrypted SQLite columns without the master key.
- Breaking the audit log's append-only property.
- Denial of service that survives a process restart.

## Out of scope

- Findings that require already having root on the host.
- Findings that depend on the operator misusing a configuration option that
  is documented as dangerous.
- Best-practice nits on otherwise-safe code (please file a normal issue or
  PR).
- Vulnerabilities in upstream Rust crates — those should go to the relevant
  upstream project, though we appreciate a heads-up.
