//! Per-bot spending limits: enforced before the model call, durable, fail closed.

use permission_engine::Policy;
use protocol::{Event, RunId};
use provider_catalog::ChatChunk;
use serde_json::json;
use task_runtime::RunState;

use crate::bot_limits::{self, BOT_LIMIT_REACHED, BOT_LIMIT_WARNING, BotLimits, Period};
use crate::tests::{Env, Fake, Reply, cfg, make_loop, profile, setup, text};
use crate::{AgentLoop, ModelPrice};

/// $1 per token: a test call (10 in, 5 out) costs $15, the first-call estimate is $4010.
fn priced() -> crate::LoopConfig {
    let mut c = cfg(Policy::full());
    c.max_tokens = Some(10);
    c.prices.insert(
        "fake/m".into(),
        ModelPrice {
            input_per_mtok: 1_000_000.0,
            output_per_mtok: 1_000_000.0,
            cached_per_mtok: None,
        },
    );
    c
}

fn set_limits(env: &Env, limits: &BotLimits) {
    env.storage
        .set_bot_spend_limits(
            env.user,
            env.bot,
            &serde_json::to_string(limits).unwrap(),
            &json!({}),
        )
        .unwrap();
}

fn daily(f: impl FnOnce(&mut bot_limits::PeriodLimit)) -> BotLimits {
    let mut l = BotLimits::default();
    f(&mut l.daily);
    l
}

async fn finish(
    lp: &AgentLoop,
    rx: &mut tokio::sync::broadcast::Receiver<crate::shared::StreamItem>,
    run: RunId,
) -> (RunState, Vec<Event>) {
    let state = lp.wait(run).await.unwrap();
    let mut evs = Vec::new();
    while let Ok(i) = rx.try_recv() {
        evs.push(i.envelope.event);
    }
    (state, evs)
}

fn run_error(evs: &[Event]) -> Option<String> {
    evs.iter().find_map(|e| match e {
        Event::RunFinished(f) => f.error.clone(),
        _ => None,
    })
}

#[tokio::test]
async fn hard_limit_stops_the_run_before_the_model_call() {
    let env = setup();
    set_limits(&env, &daily(|d| d.hard_usd = Some(100.0)));
    let fake = Fake::new(vec![text("hello")]);
    let lp = make_loop(&env.storage, env.bot, &fake, &[], profile(""), priced());
    let mut rx = lp.subscribe();
    let run = lp.run_turn(env.chat, "hi").unwrap();
    let (state, evs) = finish(&lp, &mut rx, run).await;
    assert_eq!(state, RunState::Failed);
    assert_eq!(fake.n_requests(), 0, "no provider request may be made");
    let msg = run_error(&evs).expect("typed error");
    assert!(
        msg.contains(&format!("[{BOT_LIMIT_REACHED} period=daily metric=usd")),
        "{msg}"
    );
    let audit = env.storage.list_audit(env.user).unwrap();
    assert!(audit.iter().any(|a| a.action == "bot_limit.blocked"));
}

#[tokio::test]
async fn unknown_price_counts_tokens_only_and_unreported_usage_is_not_free() {
    // No price known: a tiny money cap cannot block, a token cap does.
    let env = setup();
    set_limits(&env, &daily(|d| d.hard_usd = Some(0.01)));
    let fake = Fake::new(vec![text("a")]);
    let lp = make_loop(
        &env.storage,
        env.bot,
        &fake,
        &[],
        profile(""),
        cfg(Policy::full()),
    );
    let run = lp.run_turn(env.chat, "hi").unwrap();
    assert_eq!(lp.wait(run).await.unwrap(), RunState::Succeeded);
    let u = bot_limits::period_usage(
        &env.storage,
        env.user,
        env.bot,
        Period::Daily,
        storage::now_ms(),
    )
    .unwrap();
    assert_eq!((u.usd, u.tokens), (0.0, 15));

    // Provider reports no usage: the start-of-call estimate counts instead.
    let env = setup();
    set_limits(&env, &daily(|d| d.hard_usd = Some(5_000.0)));
    let silent = Reply::Chunks(vec![ChatChunk::TextDelta("x".into())]);
    let fake = Fake::new(vec![silent, text("never")]);
    let lp = make_loop(&env.storage, env.bot, &fake, &[], profile(""), priced());
    let run = lp.run_turn(env.chat, "one").unwrap();
    assert_eq!(lp.wait(run).await.unwrap(), RunState::Succeeded);
    let u = bot_limits::period_usage(
        &env.storage,
        env.user,
        env.bot,
        Period::Daily,
        storage::now_ms(),
    )
    .unwrap();
    assert!(u.usd >= 4_000.0 && u.unreported_calls == 1, "{u:?}");
    let run = lp.run_turn(env.chat, "two").unwrap();
    assert_eq!(lp.wait(run).await.unwrap(), RunState::Failed);
    assert_eq!(fake.n_requests(), 1);
}

#[tokio::test]
async fn unreadable_limits_fail_closed_and_new_bots_get_the_default_cap() {
    let env = setup();
    env.storage
        .set_bot_spend_limits(env.user, env.bot, "{not json", &json!({}))
        .unwrap();
    let fake = Fake::new(vec![text("hello")]);
    let lp = make_loop(
        &env.storage,
        env.bot,
        &fake,
        &[],
        profile(""),
        cfg(Policy::full()),
    );
    let run = lp.run_turn(env.chat, "hi").unwrap();
    assert_eq!(lp.wait(run).await.unwrap(), RunState::Failed);
    assert_eq!(fake.n_requests(), 0);

    // A bot without a stored row gets the modest default daily cap.
    let other = protocol::BotId::new();
    env.storage
        .insert_bot(&storage::Bot {
            id: other,
            user_id: env.user,
            name: "new".into(),
            config: json!({}),
            created_at_ms: storage::now_ms(),
        })
        .unwrap();
    let l = bot_limits::load_limits(&env.storage, other).unwrap();
    assert_eq!(l, BotLimits::default_for_new_bot());
    assert!(l.daily.hard_usd.is_some());
}

#[tokio::test]
async fn soft_limit_notifies_once_at_80_and_once_at_100() {
    let env = setup();
    set_limits(&env, &daily(|d| d.soft_usd = Some(18.0)));
    let replies = (0..4).map(|_| text("ok")).collect();
    let fake = Fake::new(replies);
    let lp = make_loop(&env.storage, env.bot, &fake, &[], profile(""), priced());
    let mut rx = lp.subscribe();
    let mut all = Vec::new();
    for _ in 0..4 {
        let run = lp.run_turn(env.chat, "hi").unwrap();
        let (state, evs) = finish(&lp, &mut rx, run).await;
        assert_eq!(state, RunState::Succeeded, "a soft limit never stops a run");
        all.extend(evs);
    }
    let warnings: Vec<String> = all
        .iter()
        .filter_map(|e| match e {
            Event::Error(r) if r.message.contains(BOT_LIMIT_WARNING) => Some(r.message.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        warnings.iter().filter(|m| m.contains("level=80")).count(),
        1,
        "{warnings:?}"
    );
    assert_eq!(
        warnings.iter().filter(|m| m.contains("level=100")).count(),
        1,
        "{warnings:?}"
    );
}

#[tokio::test]
async fn parallel_calls_of_one_bot_cannot_all_pass_just_under_the_cap() {
    // The cap fits ONE first-call estimate ($4010) but not two.
    let env = setup();
    set_limits(&env, &daily(|d| d.hard_usd = Some(5_000.0)));
    let chat2 = protocol::ChatId::new();
    env.storage
        .insert_chat(&storage::Chat {
            id: chat2,
            user_id: env.user,
            bot_id: env.bot,
            title: "c2".into(),
            created_at_ms: storage::now_ms(),
        })
        .unwrap();
    let fake = Fake::new(vec![Reply::Hang, text("never")]);
    let lp = make_loop(&env.storage, env.bot, &fake, &[], profile(""), priced());
    let first = lp.run_turn(env.chat, "one").unwrap();
    for _ in 0..200 {
        if fake.n_requests() == 1 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert_eq!(fake.n_requests(), 1, "first call is in flight");
    let second = lp.run_turn(chat2, "two").unwrap();
    assert_eq!(lp.wait(second).await.unwrap(), RunState::Failed);
    assert_eq!(
        fake.n_requests(),
        1,
        "second call never reached the provider"
    );
    lp.cancel(first).unwrap();
}
