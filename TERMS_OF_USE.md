# Terms of Use

sui-id is open-source software distributed under the Apache License 2.0.
Read the [LICENSE](LICENSE) file for the full legal text.

This page collects a few additional notes that operators and integrators
should be aware of. Nothing here overrides the license.

## You operate it; you own it

sui-id is software you run yourself. The maintainers do not operate any
hosted instance and have no visibility into your data, your users, or your
deployment. Backups, security patches, key custody, audit retention, and
incident response are your responsibility.

## No warranty, no SLA

The license states this in formal terms; we want to be plain about it too.
sui-id is offered "as is." There is no service level agreement, no support
contract by default, and no commitment to a release schedule.

If you need any of those things, you are welcome to fork the project, run it
on your own terms, and offer support to your own users. The license permits
this.

## Security disclosure

If you believe you have found a security issue, please follow the process in
[`.github/SECURITY.md`](.github/SECURITY.md). Do not file public issues for
suspected vulnerabilities.

## Trademarks

"sui-id" is the project name. The license grants no trademark rights. You
may describe your fork or your deployment as "based on sui-id" or
"compatible with sui-id"; please do not call your fork "sui-id" or imply
that it is the upstream project.

## Data the binary itself collects

None. sui-id does not phone home, does not send telemetry, and does not
contact any remote service that the operator has not configured. The single
network listener is the one defined in `[server].listen_addr`.

## Cryptographic exports

sui-id uses standard public cryptographic primitives (XChaCha20-Poly1305,
Ed25519, Argon2id, SHA-256). Operators are responsible for compliance with
any export-control regulations that apply to their jurisdiction.
