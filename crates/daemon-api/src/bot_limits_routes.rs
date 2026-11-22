//! Per-bot spending limits (`/v1/bots/{id}/spend-limits`, ADR-0034).
//!
//! Enforcement lives in `agent-core` (`bot_limits.rs`, checked before every
//! model call). This module reads the limits with their current usage and
//! lets the owner change them. Changing them is owner-only (see `owner.rs`)
//! and every change is written to the audit log in the same transaction.

use agent_core::bot_limits::{
    BotLimits, DEFAULT_DAILY_HARD_TOKENS, DEFAULT_DAILY_HARD_USD, Period, PeriodLimit, load_limits,
    period_usage,
};
use axum::http::HeaderMap;
use axum::{Json, extract::Path as AxumPath, extract::State};
use protocol::BotId;
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{ApiError, AppState, error::ErrorBody, routes::ApiJson};

/// Limits of one period. `null` = not set. Soft limits notify at 80% and 100%;
/// hard limits stop model calls before they happen.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PeriodLimitDto {
    /// Notify at 80% and 100% of this amount (USD, 0.01 to 1,000,000).
    #[serde(default)]
    pub soft_usd: Option<f64>,
    /// Stop model calls beyond this amount (USD, 0.01 to 1,000,000).
    #[serde(default)]
    pub hard_usd: Option<f64>,
    /// Notify at 80% and 100% of this many tokens.
    #[serde(default)]
    pub soft_tokens: Option<u64>,
    /// Stop model calls beyond this many tokens.
    #[serde(default)]
    pub hard_tokens: Option<u64>,
}

/// Daily, weekly and monthly limits of one bot.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BotLimitsDto {
    /// Per local calendar day.
    #[serde(default)]
    pub daily: PeriodLimitDto,
    /// Per local calendar week (Monday start).
    #[serde(default)]
    pub weekly: PeriodLimitDto,
    /// Per local calendar month.
    #[serde(default)]
    pub monthly: PeriodLimitDto,
}

/// What one period has used so far.
#[derive(Debug, Serialize, ToSchema)]
pub struct PeriodUsageDto {
    /// Period start (ms since epoch).
    pub start_ms: i64,
    /// When the counter resets (ms since epoch).
    pub resets_ms: i64,
    /// Money counted in the period (USD). Models without a known price add nothing here.
    pub usd: f64,
    /// Tokens counted in the period.
    pub tokens: u64,
    /// Calls whose provider reported no usage; counted by a conservative estimate.
    pub unreported_calls: u64,
}

/// Usage of the three periods.
#[derive(Debug, Serialize, ToSchema)]
pub struct BotLimitUsageDto {
    /// Today.
    pub daily: PeriodUsageDto,
    /// This week.
    pub weekly: PeriodUsageDto,
    /// This month.
    pub monthly: PeriodUsageDto,
}

/// A bot's limits, their usage and the default applied to bots that were never configured.
#[derive(Debug, Serialize, ToSchema)]
pub struct BotSpendLimitsDto {
    /// The limits in force.
    pub limits: BotLimitsDto,
    /// `true` while the bot has no saved limits and the safe default applies.
    pub is_default: bool,
    /// What each period has used.
    pub usage: BotLimitUsageDto,
    /// Daily hard cap of a bot that was never configured (USD).
    pub default_daily_hard_usd: f64,
    /// Daily hard cap of a bot that was never configured (tokens).
    pub default_daily_hard_tokens: u64,
    /// Now, as the daemon sees it (ms since epoch).
    pub now_ms: i64,
}

/// `PUT /v1/bots/{id}/spend-limits`.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PutBotSpendLimitsInput {
    /// The new limits; every field left `null` is unlimited. Replaces all previous values.
    pub limits: BotLimitsDto,
}

fn period_of(l: &PeriodLimit) -> PeriodLimitDto {
    PeriodLimitDto {
        soft_usd: l.soft_usd,
        hard_usd: l.hard_usd,
        soft_tokens: l.soft_tokens,
        hard_tokens: l.hard_tokens,
    }
}

fn to_dto(l: &BotLimits) -> BotLimitsDto {
    BotLimitsDto {
        daily: period_of(&l.daily),
        weekly: period_of(&l.weekly),
        monthly: period_of(&l.monthly),
    }
}

fn from_dto(d: &BotLimitsDto) -> BotLimits {
    let p = |d: &PeriodLimitDto| PeriodLimit {
        soft_usd: d.soft_usd,
        hard_usd: d.hard_usd,
        soft_tokens: d.soft_tokens,
        hard_tokens: d.hard_tokens,
    };
    BotLimits {
        daily: p(&d.daily),
        weekly: p(&d.weekly),
        monthly: p(&d.monthly),
    }
}

fn dto(s: &AppState, bot: BotId) -> Result<BotSpendLimitsDto, ApiError> {
    let limits = load_limits(s.storage(), bot).map_err(ApiError::internal)?;
    let is_default = s.storage().get_bot_spend_limits(bot)?.is_none();
    let now_ms = storage::now_ms();
    let usage = |p: Period| -> Result<PeriodUsageDto, ApiError> {
        let u = period_usage(s.storage(), s.user_id(), bot, p, now_ms)?;
        Ok(PeriodUsageDto {
            start_ms: u.start_ms,
            resets_ms: u.end_ms,
            usd: u.usd,
            tokens: u.tokens,
            unreported_calls: u.unreported_calls,
        })
    };
    Ok(BotSpendLimitsDto {
        limits: to_dto(&limits),
        is_default,
        usage: BotLimitUsageDto {
            daily: usage(Period::Daily)?,
            weekly: usage(Period::Weekly)?,
            monthly: usage(Period::Monthly)?,
        },
        default_daily_hard_usd: DEFAULT_DAILY_HARD_USD,
        default_daily_hard_tokens: DEFAULT_DAILY_HARD_TOKENS,
        now_ms,
    })
}

/// A bot's spending limits and what each period has used.
#[utoipa::path(get, path = "/v1/bots/{id}/spend-limits", tag = "usage", security(("bearer" = [])),
    params(("id" = Uuid, Path, description = "Bot id")),
    responses((status = 200, body = BotSpendLimitsDto), (status = 404, body = ErrorBody)))]
pub(crate) async fn get_bot_spend_limits(
    State(s): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Json<BotSpendLimitsDto>, ApiError> {
    let bot = s.bot_of_user(BotId(id))?;
    Ok(Json(dto(&s, bot.id)?))
}

/// Replace a bot's spending limits (owner only). Effective for the next model
/// call of every run of the bot, including scheduled and channel-triggered ones.
/// The change is audited as `bot.spend_limits_changed`.
#[utoipa::path(put, path = "/v1/bots/{id}/spend-limits", tag = "usage", security(("bearer" = [])),
    params(("id" = Uuid, Path, description = "Bot id")),
    request_body = PutBotSpendLimitsInput,
    responses((status = 200, body = BotSpendLimitsDto), (status = 400, body = ErrorBody),
              (status = 404, body = ErrorBody)))]
pub(crate) async fn put_bot_spend_limits(
    State(s): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<Uuid>,
    ApiJson(input): ApiJson<PutBotSpendLimitsInput>,
) -> Result<Json<BotSpendLimitsDto>, ApiError> {
    // Defence in depth next to `OWNER_ROUTES` and the peer denial.
    s.require_owner(&headers)?;
    let bot = s.bot_of_user(BotId(id))?;
    let new = from_dto(&input.limits);
    new.validate().map_err(ApiError::bad_request)?;
    // Before-value for the audit entry; an unreadable stored value reads as "unknown".
    let before = load_limits(s.storage(), bot.id)
        .ok()
        .map(|l| serde_json::to_value(to_dto(&l)).unwrap_or_default());
    let raw = serde_json::to_string(&new).map_err(ApiError::internal)?;
    s.storage().set_bot_spend_limits(
        s.user_id(),
        bot.id,
        &raw,
        &json!({
            "bot_id": bot.id.to_string(),
            "before": before,
            "after": to_dto(&new),
        }),
    )?;
    Ok(Json(dto(&s, bot.id)?))
}
