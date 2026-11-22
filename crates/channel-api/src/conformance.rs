//! Reusable adapter tests. Run against an isolated fake transport, never a live
//! account. The caller supplies a receive fixture and bounds the test's runtime.

use futures_util::StreamExt;

use crate::{Channel, ChatRef, IncomingMessage, InlineButton, OutgoingMessage, Result};

/// Exercise receive, send, editing and callback acknowledgement through `dyn Channel`.
/// Fixtures must make the advertised operations available and return the same
/// message id when editing. Additional adapter tests must inspect actual wire
/// requests, redaction, pairing and authorization: this is a transport smoke test.
///
/// # Errors
/// Propagates adapter errors.
///
/// # Panics
/// A fixture event differs, a message id is empty, or an edit changes its id.
pub async fn assert_transport(
    channel: &dyn Channel,
    expected: &[IncomingMessage],
    to: ChatRef,
    callback_id: &str,
) -> Result<()> {
    let caps = channel.capabilities();
    let mut incoming = channel.receive().await?;
    for event in expected {
        assert_eq!(incoming.next().await.as_ref(), Some(event));
    }
    let mut outgoing = OutgoingMessage::text("hello from trait");
    if caps.inline_buttons {
        outgoing = outgoing.with_buttons(vec![vec![InlineButton::new("Approve once", "once")]]);
    }
    let id = channel.send(to.clone(), outgoing).await?;
    assert!(!id.is_empty(), "send must return an opaque message id");
    if caps.message_editing {
        let edited = channel
            .send(to, OutgoingMessage::edit(id.clone(), "edited from trait"))
            .await?;
        assert_eq!(edited, id, "edit must preserve the message id");
    }
    if caps.inline_buttons {
        channel
            .answer_callback(callback_id, Some("Approved"))
            .await?;
    }
    Ok(())
}
