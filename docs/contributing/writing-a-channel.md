# Writing a channel adapter

`channel-api` defines the transport contract used by messaging adapters. It has
no dependency on the daemon, HTTP clients, protocol crate, secret storage, or a
runtime. `channel-telegram` is the existing implementation; future adapters for
[Discord, Slack and Matrix](https://github.com/emreocell/Praxi-Bot/issues/207)
can use the same contract.

Implement `channel_api::Channel` with `async_trait`. `receive` returns a boxed
stream of `IncomingMessage`; `send` sends a new message or edits the indicated
message and returns its opaque id; `answer_callback` acknowledges an interaction.
Advertise only supported operations in `ChannelCaps`. Attachments currently use
`IncomingKind::Unsupported` with a human label; this refactor adds no file or
voice delivery.

Keep chat, sender, event and message identifiers opaque at the shared boundary.
Use `SenderIdentity.id` from the service's authenticated event, never a display
name or a value in message text. Namespace persisted destinations with
`protocol::ChannelRef`. Service polling cursors, delivery recovery, API errors
and wire types belong inside the adapter. `ChannelError::Adapter` retains a
concrete error's display and source. Telegram retains numeric convenience types
and its existing durable inbox representation, converting only at the shared
trait boundary.

Reuse `pairing::PairingInvites` under the adapter's state lock. Generate codes
with the OS CSPRNG, call `create`, then `check` on redemption. Capture the time
before acquiring the state lock, as Telegram does today. For a valid ticket,
persist the authenticated paired identity **before** calling `consume`, without
releasing the lock or interleaving invite operations. A storage failure must
leave the code usable and the sender unpaired. The helper preserves one active
code, expiry, a global wrong-guess budget and per-sender lockout. Persistent
pairing records, refusal flood limits and chat-to-bot/thread mappings remain
adapter responsibilities; Telegram's existing file schema is unchanged.

Use `approval::approval_row` to build buttons: detailed requests never get an
Always button, and task control requires the complete card on screen. The bridge
must first render all required details, redact before splitting, suppress
controls when details are masked/unavailable, and revoke local approval tokens
if delivery fails. A callback must still match the paired identity, originating
chat, pending request and permitted scope before calling the daemon. The button
helper does not grant permission.

Keep tokens in `secret-store`, redact outgoing text and callback acknowledgements,
and preserve allowlists, refusal limits, owner authentication and the permission
engine. Receiving an event through `Channel` alone does not authorize an action.

The CLI constructs adapters in `apps/cli/src/channels.rs::registered_channels`.
Implement the daemon's separate `ChannelControl` lifecycle interface for the new
runtime and add it to that list. The existing daemon registry handles leadership,
start/stop, status and pairing routes; safe mode skips registration. Adapter CLI
configuration can remain service-specific.

Run `channel_api::conformance::assert_transport` from an async test against an
isolated fake transport. Supply expected incoming events, a destination and a
callback id; wrap it in your test runtime's timeout. It checks object-safe receive,
send, message-id-preserving edits and callbacks according to capabilities. See
`crates/channel-api/src/tests.rs` for a fake with nonnumeric ids that also drives
shared pairing and detailed approval policy, and Telegram's `tests.rs` for a mock
HTTP implementation. Separately assert real wire requests, redaction,
private-chat filtering, pairing persistence/lockout and approval ownership. No
test should require real credentials or live service access.
