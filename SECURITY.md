# Security policy

## Supported versions

Senu is pre-1.0. Security fixes are made on the latest 0.x release only.

| Version | Supported |
|---|---|
| 0.3.x | yes |
| 0.2.x | no (upgrade to 0.3) |
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
(`senu::contract::check`, CT-1 to CT-12):

- Authentication comes before the permission, and the permission before
  any input validation.
- A route cannot be served without an authenticator: such a table is
  refused at startup.
- Principals, tenants and permissions are chosen only by server code. No
  request can carry them.
- Unknown and repeated parameters and values of the wrong type are refused.
- A write answers only `POST`, is refused with 403 when a browser sends it
  from another site (`Sec-Fetch-Site`, or `Origin` against `Host`), and no
  byte of its body is parsed before authentication and the permission. Its
  body is limited (64 KiB by default) by declared and by streamed length.
- A write's MCP tool is never marked read-only.
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

The cross-site check covers declared writes only. A `custom` route that
changes state behind a cookie or Basic credential must call
`auth::same_origin` itself. The check compares `Origin` with `Host`, so a
proxy must pass the original `Host` on, or the application must name its
public origin with `Endpoints::trusted_origin`. It does nothing against a
non-browser client that holds a valid credential.

Senu does not make a write atomic or idempotent, and over stdio every MCP
call runs as the one principal chosen at startup: if that principal may
write, whoever can start the process may write.

The `senu` binary is a local demo. It binds to loopback addresses only and
has no TLS; do not expose it.
