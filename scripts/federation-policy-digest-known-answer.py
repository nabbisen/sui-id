#!/usr/bin/env python3.14
"""RFC 096-B2 S3: an independent, hashlib-only re-implementation of the
policy digest (RFC 096 `:424-437`), for one fixed example policy.

This is deliberately not a transcription of the Rust code -- it is a
second, independent reading of RFC 096 `:424-437`'s own words, built
from `hashlib` alone. The Rust test
(`crates/sui-id-store/src/federation_policy/digest/tests.rs`,
`known_answer_vector_matches_the_independent_python_implementation`)
pins the hex digest this script prints. Two implementations of one
encoding agreeing is the evidence that the encoding is what RFC 096
`:424-437` says; one implementation compared with itself proves nothing.

Field-tag order (the architect's own choice, recorded in the Rust code
and here identically): 1 issuer, 2 client ID, 3 sealed-secret-envelope
SHA-256 (explicit absent/present), 4 token-auth-method tag, 5 scopes,
6 ID-token algorithms, 7 endpoint origins, 8 HTTPS ports, 9 provision
mode.
"""

import hashlib
import sys

DOMAIN = b"sui-id/federation-provider-policy/v1\0"

TAG_ISSUER = 1
TAG_CLIENT_ID = 2
TAG_SECRET = 3
TAG_TOKEN_AUTH_METHOD = 4
TAG_SCOPES = 5
TAG_ID_TOKEN_ALGS = 6
TAG_ENDPOINT_ORIGINS = 7
TAG_HTTPS_PORTS = 8
TAG_PROVISION_MODE = 9


def scalar(tag: int, data: bytes) -> bytes:
    return bytes([tag]) + len(data).to_bytes(4, "big") + data


def item_list(tag: int, items: list[bytes]) -> bytes:
    out = bytes([tag]) + len(items).to_bytes(2, "big")
    for item in items:
        out += len(item).to_bytes(4, "big") + item
    return out


def policy_digest(
    issuer: str,
    client_id: str,
    secret_envelope: bytes | None,
    token_auth_method_present: bool,
    scopes: list[str],
    id_token_algs: list[str],
    origins: list[str],
    ports: list[int],
    provision_mode: str,
) -> bytes:
    buf = DOMAIN
    buf += scalar(TAG_ISSUER, issuer.encode())
    buf += scalar(TAG_CLIENT_ID, client_id.encode())

    if secret_envelope is None:
        secret_field = bytes([0])
    else:
        secret_field = bytes([1]) + hashlib.sha256(secret_envelope).digest()
    buf += scalar(TAG_SECRET, secret_field)

    buf += scalar(TAG_TOKEN_AUTH_METHOD, bytes([1 if token_auth_method_present else 0]))

    buf += item_list(TAG_SCOPES, [s.encode() for s in scopes])
    buf += item_list(TAG_ID_TOKEN_ALGS, [a.encode() for a in id_token_algs])
    buf += item_list(TAG_ENDPOINT_ORIGINS, [o.encode() for o in sorted(origins)])
    buf += item_list(TAG_HTTPS_PORTS, [p.to_bytes(2, "big") for p in sorted(ports)])

    buf += scalar(TAG_PROVISION_MODE, provision_mode.encode())

    return hashlib.sha256(buf).digest()


def main() -> int:
    # The fixed example provider (matches the Rust test's own fixture
    # exactly -- see `known_answer_vector_matches_the_independent_python_
    # implementation`).
    digest = policy_digest(
        issuer="https://accounts.google.com",
        client_id="test-client-id",
        secret_envelope=bytes(range(48)),  # 48 arbitrary, fixed bytes
        token_auth_method_present=True,
        scopes=["openid", "email"],
        id_token_algs=["RS256"],
        origins=["https://accounts.google.com"],
        ports=[443],
        provision_mode="link_only",
    )
    print(digest.hex())

    # Vector 2 (S3 review): the cases one-item lists cannot exercise. No
    # secret; three scopes; two algorithms; three origins given unsorted,
    # one with a non-default port, so two ports. Lists are passed in their
    # canonical order except origins and ports, which this script sorts
    # itself, as the encoding says.
    digest2 = policy_digest(
        issuer="https://login.example.com/tenant/v2.0",
        client_id="public-client",
        secret_envelope=None,
        token_auth_method_present=False,
        scopes=["openid", "email", "profile"],
        id_token_algs=["RS256", "EdDSA"],
        origins=[
            "https://login.example.com:8443",
            "https://api.example.com",
            "https://login.example.com",
        ],
        ports=[8443, 443],
        provision_mode="provision_on_first_login",
    )
    print(digest2.hex())
    return 0


if __name__ == "__main__":
    sys.exit(main())
