import { Gauge } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "ui";
import { OPEN_BOT_LIMITS_EVENT, type LimitReached } from "../lib/botLimits";
import styles from "./BotLimitCard.module.css";

/** A run stopped by a bot's hard limit: raise it, wait for the reset, or leave it stopped. */
export function BotLimitCard({ limit }: { limit: LimitReached }) {
  const { t, i18n } = useTranslation();
  const [choice, setChoice] = useState<"wait" | "stop" | null>(null);
  const resets = limit.resetsMs ? new Date(limit.resetsMs).toLocaleString(i18n.language, { dateStyle: "medium", timeStyle: "short" }) : null;
  return (
    <div className={styles.card} role="alert" data-testid="bot-limit-card">
      <div className={styles.head}>
        <Gauge size={16} aria-hidden />
        <strong>{t(`botLimits.card.title.${limit.period}`)}</strong>
      </div>
      <p className={styles.body}>{t(`botLimits.card.body.${limit.metric}`)}</p>
      {choice === "wait" ? (
        <p className={styles.note}>{resets ? t("botLimits.card.waiting", { when: resets }) : t("botLimits.card.waitingUnknown")}</p>
      ) : null}
      {choice === "stop" ? <p className={styles.note}>{t("botLimits.card.stopped")}</p> : null}
      {choice === null ? (
        <div className={styles.actions}>
          <Button size="sm" onClick={() => window.dispatchEvent(new CustomEvent(OPEN_BOT_LIMITS_EVENT))} data-testid="bot-limit-raise">
            {t("botLimits.card.raise")}
          </Button>
          <Button size="sm" variant="secondary" onClick={() => setChoice("wait")} data-testid="bot-limit-wait">
            {t("botLimits.card.wait")}
          </Button>
          <Button size="sm" variant="ghost" onClick={() => setChoice("stop")} data-testid="bot-limit-stop">
            {t("botLimits.card.stop")}
          </Button>
        </div>
      ) : null}
    </div>
  );
}
