use crate::core::types::{Config, Stats};

pub fn check(
    stats: &Stats,
    balance_cents: u64,
    config: &Config,
) -> Option<String> {
    if balance_cents < config.min_balance_cents {
        return Some(format!(
            "Balance {}¢ < {}¢ minimum",
            balance_cents, config.min_balance_cents
        ));
    }
    if stats.today_pnl_cents <= -config.max_daily_loss_cents {
        return Some(format!("Daily loss: {}¢", stats.today_pnl_cents));
    }
    if stats.current_streak <= -(config.max_consecutive_losses as i32) {
        return Some(format!(
            "{}× consecutive losses",
            stats.current_streak.abs()
        ));
    }
    None
}
