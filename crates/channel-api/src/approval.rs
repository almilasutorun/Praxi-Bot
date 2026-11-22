//! Shared approval button policy. Rendering and callback authorization stay in
//! the adapter: offer controls only after all required details can be reviewed.

use crate::InlineButton;

/// Assemble one approval row. Detailed requests never offer a standing grant;
/// task control is offered only when the complete card fits on screen.
#[must_use]
pub fn approval_row(
    has_details: bool,
    offer_run_control: bool,
    once: InlineButton,
    for_task: Option<InlineButton>,
    always: InlineButton,
    deny: InlineButton,
) -> Vec<InlineButton> {
    let mut row = vec![once];
    if offer_run_control && let Some(button) = for_task {
        row.push(button);
    }
    if !has_details {
        row.push(always);
    }
    row.push(deny);
    row
}
