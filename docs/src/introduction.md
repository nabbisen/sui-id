# sui-id

> A self-hosted, single-binary OpenID Connect provider written in Rust.

sui-id is an Identity-as-a-Service you run yourself. It speaks OpenID Connect
on the front end, stores its data in a single encrypted SQLite file, and ships
as one statically linked binary. There is no separate database service, no
embedded JavaScript runtime, and no ambient cloud dependency.

## Who is this for?

sui-id is built for **single-organisation or single-product deployments** where:

- All users share one namespace (no per-customer isolation needed).
- All clients are first-party or trusted third-party applications.
- The deployment team is small — one or two operators.
- Operational simplicity matters as much as feature count.

If you need multi-tenant isolation, SAML, LDAP federation, or dynamic client
registration open to the internet, see the [scope statement](./getting-started/overview.md#scope).

## How to read this documentation

| You want to… | Start here |
|---|---|
| Run sui-id for the first time | [Quick start](./getting-started/quick-start.md) |
| Deploy and harden for production | [Deployment guide](./guides/deployment.md) |
| Configure running instance | [Operators reference](./guides/operators.md) |
| Point your application at sui-id | [OIDC API](./reference/oidc-api.md) |
| Understand the security model | [Deployment guide § Security hardening](./guides/deployment.md#security-hardening) |
| Contribute code or translations | [Contributing](./contributing/architecture.md) |
