# Security policy

## Supported versions

Senu is pre-1.0. Security fixes are made on the latest 0.x release only.

| Version | Supported |
|---|---|
| 0.2.x | yes |
| 0.1.x | no (the seed; never published as a crate) |

## Reporting a vulnerability

Please report a suspected vulnerability privately, not in a public issue.
Use GitHub's private vulnerability reporting for this repository:
https://github.com/efg-technologies/senu/security/advisories/new

Include the version or commit, what you observed, and the smallest steps
that show it. You will get an acknowledgement, and we will tell you whether
we can reproduce it and what we plan to do. We ask you to give us a
reasonable time to release a fix before you publish details.

## What Senu promises, and what it does not

Senu is not audited security software. It has had no independent review,
and it makes no guarantee beyond what its tests check.

What the tests check for every declared route and tool
(`senu::contract::check`, CT-1 to CT-7):

- Authentication comes before the permission, and the permission before
  any input validation.
- A route cannot be served without an authenticator: such a table is
  refused at startup.
- Principals, tenants and permissions are chosen only by server code. No
  request can carry them.
- Unknown, repeated and non-string parameters are refused.
- Errors are fixed codes and phrases that repeat neither the input nor a
  secret, and `OpError::Internal` carries no detail.
- Credentials in the query string are never read, and exactly one
  `Authorization` header is accepted by the built-in authenticators.
- Responses carry `Cache-Control: no-store` and
  `X-Content-Type-Options: nosniff`, and HTML pages a Content-Security-Policy.

What stays with the application:

- TLS, HSTS, rate limiting, lockout, request logging and timeouts.
- The strength, storage and rotation of secrets. The built-in Basic and
  Bearer authenticators compare static secrets with
  `subtle::ConstantTimeEq`; a length mismatch is detected before the byte
  comparison.
- Its own `Authenticate` implementation, its `custom` routes and its
  assets, which the contract does not cover.
- The business rules inside its operations, including tenant filtering.

Senu has no write operations in 0.2, so it has no CSRF protection either;
do not add state-changing `custom` routes behind an ambient browser
credential without your own protection.

The `senu` binary is a local demo. It binds to loopback addresses only and
has no TLS; do not expose it.
