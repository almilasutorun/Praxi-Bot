//! One-time pairing invites and brute-force lockout, independent of persistence.
//!
//! Hold the adapter's state lock across `check`, durable identity persistence,
//! and `consume`. A valid invite is consumed only after persistence succeeds.

use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

/// Validity window of a one-time 6-digit pairing code (10 minutes).
pub const PAIR_CODE_TTL: Duration = Duration::from_secs(10 * 60);

/// Maximum wrong `/pair` guesses (from anyone) before pending codes are invalidated.
///
/// This is the global brute-force cap: a 6-digit code survives at most this many wrong
/// guesses in total, no matter how many service accounts they come from. Because only
/// one live code exists at a time (see [`PairingInvites::create`]) the total guessing
/// budget per code is `MAX_PAIR_ATTEMPTS`, i.e. an attacker's success chance is about
/// `MAX_PAIR_ATTEMPTS / 1_000_000`.
///
/// Known trade-off (accepted): the counter is shared, so any channel user can burn the
/// owner's code with `MAX_PAIR_ATTEMPTS` wrong `/pair` messages. That is only a
/// denial of pairing, never an authentication bypass; the owner generates a new code, or
/// pre-pairs through adapter configuration, which does not
/// depend on codes at all and is the fallback path.
pub const MAX_PAIR_ATTEMPTS: u32 = 5;

/// Maximum wrong `/pair` guesses per channel user before that user is locked out.
pub const MAX_PAIR_ATTEMPTS_PER_USER: u32 = 5;

/// How long a user stays locked out after exceeding [`MAX_PAIR_ATTEMPTS_PER_USER`].
pub const PAIR_LOCKOUT: Duration = Duration::from_secs(60 * 60);

/// Upper bound for per-user bookkeeping (memory-exhaustion protection).
const MAX_TRACKED_USERS: usize = 10_000;

#[derive(Debug, Clone)]
struct PendingCode {
    code: String,
    expires_at: Instant,
    failed_attempts: u32,
}

/// Result of checking an invite; success still needs durable persistence.
#[derive(Debug)]
pub enum PairingMatch {
    /// Persist the identity, then consume this ticket under the same state lock.
    Valid(PairingTicket),
    /// The invite expired.
    Expired,
    /// No invite matched.
    InvalidCode,
    /// The sender is locked out; do not reply.
    RateLimited,
}

/// A valid invite position, usable only while the adapter holds its state lock.
#[derive(Debug)]
pub struct PairingTicket(usize);

/// Volatile invites and per-sender wrong-guess tracking.
#[derive(Debug)]
pub struct PairingInvites<U> {
    pending_codes: Vec<PendingCode>,
    pair_failures: HashMap<U, (u32, Option<Instant>)>,
}

impl<U> Default for PairingInvites<U> {
    fn default() -> Self {
        Self {
            pending_codes: Vec::new(),
            pair_failures: HashMap::new(),
        }
    }
}

fn ct_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

impl<U: Eq + Hash> PairingInvites<U> {
    /// Register a code generated with the adapter's OS CSPRNG. Replaces live invites.
    /// Capture `now` before acquiring the adapter state lock.
    pub fn create(&mut self, code: String, ttl: Duration, now: Instant) {
        // Drop all older live codes; keep recently expired ones briefly so we can report
        // `Expired` vs `InvalidCode`.
        self.pending_codes
            .retain(|c| c.expires_at <= now && c.expires_at + Duration::from_secs(600) > now);
        self.pending_codes.push(PendingCode {
            code,
            expires_at: now + ttl,
            failed_attempts: 0,
        });
    }

    /// Check a sender and code without consuming a successful invite.
    /// Capture `now` before acquiring the adapter state lock.
    pub fn check(&mut self, user_id: U, code: &str, now: Instant) -> PairingMatch {
        let candidate = code.trim();

        // Per-user brute-force lockout.
        if let Some((_, Some(until))) = self.pair_failures.get(&user_id) {
            if *until > now {
                return PairingMatch::RateLimited;
            }
            self.pair_failures.remove(&user_id);
        }

        // Compare against every pending code (no early exit on the first byte mismatch).
        let mut matched: Option<usize> = None;
        for (idx, pending) in self.pending_codes.iter().enumerate() {
            if ct_eq(&pending.code, candidate) && matched.is_none() {
                matched = Some(idx);
            }
        }
        if let Some(idx) = matched {
            if self.pending_codes[idx].expires_at <= now {
                self.pending_codes.remove(idx);
                return PairingMatch::Expired;
            }
            return PairingMatch::Valid(PairingTicket(idx));
        }

        // Wrong code: count it against the user and against all live codes.
        if self.pair_failures.len() >= MAX_TRACKED_USERS {
            self.pair_failures
                .retain(|_, (_, until)| until.is_some_and(|u| u > now));
        }
        let entry = self.pair_failures.entry(user_id).or_insert((0, None));
        entry.0 += 1;
        if entry.0 >= MAX_PAIR_ATTEMPTS_PER_USER {
            entry.1 = Some(now + PAIR_LOCKOUT);
        }
        for pending in &mut self.pending_codes {
            if pending.expires_at > now {
                pending.failed_attempts += 1;
            }
        }
        self.pending_codes
            .retain(|c| c.failed_attempts < MAX_PAIR_ATTEMPTS);

        PairingMatch::InvalidCode
    }

    /// Consume a valid invite after the adapter durably stores the paired identity.
    /// Must use a ticket from `check` without interleaving other invite operations.
    #[allow(clippy::needless_pass_by_value)] // The non-Clone ticket must be consumed once.
    pub fn consume(&mut self, user_id: &U, ticket: PairingTicket) {
        self.pending_codes.remove(ticket.0);
        self.pair_failures.remove(user_id);
    }
}
