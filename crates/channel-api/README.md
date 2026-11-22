# channel-api

Lightweight messaging transport types and the object-safe `Channel` trait, with
shared one-time pairing invites, approval button policy and a conformance helper.
There are no daemon, HTTP, protocol, secret-store or runtime dependencies.

## Writing a channel adapter

Implement `Channel` using opaque service identifiers and authenticated sender
identities. Keep service cursors and wire formats private, advertise capabilities,
redact outgoing text, and preserve the bridge's pairing and authorization checks.
Run `conformance::assert_transport` with a fake transport in your adapter tests.

See [the contributor guide](../../docs/contributing/writing-a-channel.md) for
lifecycle registration, persistence requirements, approval rules and tests.
