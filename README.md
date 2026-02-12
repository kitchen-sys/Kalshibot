# kalshi-bot

Autonomous BTC trading bot for Kalshi's 15-minute binary contracts. Rust cron job that asks Claude whether to buy YES or NO every 15 minutes, places the order, and exits.

## How It Works

Every 15 minutes, cron fires a Rust binary that:

1. Cancels any stale resting orders from the previous cycle
2. Checks if the last trade settled (win/loss) and updates the ledger
3. Runs deterministic risk checks (balance floor, daily loss cap, streak limit)
4. Fetches the active BTC Up/Down market from Kalshi
5. Fetches the orderbook and live BTC price data from Binance
6. Sends everything to Claude Opus 4.6 — market state, orderbook, BTC momentum, performance stats, trade history
7. Claude returns BUY (side, shares, price) or PASS with reasoning
8. Validates and clamps the decision, checks for duplicate positions
9. Places the order on Kalshi (or logs it in paper mode)
10. Exits

The AI never writes files. All stats are computed deterministically in Rust from an append-only markdown ledger.

## Architecture

Hexagonal architecture — every external boundary is a swappable trait.

```
                    ┌─────────────────────────────┐
                    │         CORE DOMAIN          │
                    │  (pure Rust, no IO, no deps) │
                    │                              │
                    │  • engine.rs  (10-step cycle) │
                    │  • risk.rs    (limit checks)  │
                    │  • stats.rs   (ledger math)   │
                    │  • types.rs   (domain types)  │
                    └──────────┬──────────────────┘
                               │ uses traits (ports)
            ┌──────────────────┼──────────────────────┐
            │                  │                       │
    ┌───────▼──────┐   ┌──────▼───────┐   ┌──────────▼────────┐
    │  Exchange    │   │  Brain       │   │  Notifier         │
    │  (Kalshi)    │   │  (Claude)    │   │  (Telegram)       │
    └──────────────┘   └──────────────┘   └───────────────────┘
```

Swap the exchange, the AI, or the notification layer independently. Core domain is pure functions — unit-testable with zero network.

## Project Structure

```
kalshi-bot/
├── src/
│   ├── main.rs                   # Entry point, config, lockfile
│   ├── safety.rs                 # Lockfile, startup validation, live-mode gate
│   ├── core/
│   │   ├── engine.rs             # The 10-step trading cycle
│   │   ├── risk.rs               # Pure risk checks
│   │   ├── stats.rs              # Compute stats from ledger
│   │   └── types.rs              # All domain types
│   ├── ports/
│   │   ├── exchange.rs           # Exchange trait
│   │   ├── brain.rs              # Brain trait
│   │   └── notifier.rs           # Notifier trait
│   └── adapters/
│       ├── kalshi/               # Kalshi API + RSA-PSS auth
│       ├── openrouter.rs         # Claude via OpenRouter
│       └── telegram.rs           # Telegram alerts
├── brain/
│   ├── prompt.md                 # System prompt (you edit, AI reads)
│   ├── ledger.md                 # Append-only trade log
│   └── stats.md                  # Computed performance stats
└── logs/
    └── cron.log                  # Cron output
```

## Setup

### Prerequisites

- Rust toolchain (stable)
- Kalshi account with API access + RSA key pair
- OpenRouter API key
- (Optional) Telegram bot token for alerts

### Environment Variables

Create a `.env` file:

```bash
# Kalshi
KALSHI_API_KEY_ID=your-api-key-uuid
KALSHI_PRIVATE_KEY_PATH=./kalshi_private_key.pem
KALSHI_BASE_URL=https://api.elections.kalshi.com
KALSHI_SERIES_TICKER=KXBTC15M

# AI
OPENROUTER_API_KEY=sk-or-v1-...

# Safety
PAPER_TRADE=true
CONFIRM_LIVE=false
```

### Build & Run

```bash
cargo build --release

# Paper trading (default — no real orders)
./target/release/kalshi-bot

# Live trading (real money)
PAPER_TRADE=false CONFIRM_LIVE=true ./target/release/kalshi-bot
```

### Cron Setup

Run every 15 minutes, offset by 1 minute to avoid market open/close edges:

```bash
crontab -e
```

```
1,16,31,46 * * * * cd /path/to/kalshi-bot && ./target/release/kalshi-bot >> logs/cron.log 2>&1
```

## Risk Limits

All hardcoded — no config knobs to accidentally blow up:

| Limit | Default | What It Does |
|-------|---------|--------------|
| Max shares per trade | 2 | Position size cap |
| Max daily loss | $10 | Stop trading for the day |
| Max consecutive losses | 7 | Stop trading until a win |
| Min balance | $5 | Don't trade below this floor |
| Min time to expiry | 2 min | Don't enter dying markets |

## How the AI Decides

Claude receives a full context package each cycle:

- **Market data**: yes/no bid/ask, last price, volume, open interest
- **Orderbook**: full depth on both sides
- **BTC price data**: spot, 15m/1h momentum, SMA, volatility, recent candles (from Binance)
- **Performance**: win rate, streak, P&L, max drawdown
- **Trade history**: last 20 trades with outcomes

The system prompt (`brain/prompt.md`) teaches Claude to:

- Evaluate asymmetric risk/reward on both sides of every contract
- Lower conviction threshold for cheap options (<30¢) where R/R is favorable
- Set limit order prices relative to the bid/ask spread
- Size positions based on edge magnitude (5–9pt → 1 share, 10+ → 2 shares)
- PASS only when there's no edge AND no asymmetric opportunity

## Safety

- **Lockfile** (`/tmp/kalshi-bot.lock`): PID-based, prevents double execution from cron overlap
- **Live mode gate**: `PAPER_TRADE=true` by default. Must explicitly set both `PAPER_TRADE=false` and `CONFIRM_LIVE=true`
- **Order-first writes**: Order placed on Kalshi before ledger write. If the order fails, ledger stays clean — no phantom trades
- **Ledger backup**: `brain/ledger.md.bak` created before every write
- **Atomic stats**: Written to `.tmp` then renamed
- **Parse failure = PASS**: If Claude returns garbage JSON, the bot does nothing

## Kalshi Auth

RSA-PSS with SHA-256, MGF1(SHA-256), salt length 32 bytes. Message format: `{timestamp_ms}{METHOD}{path}`. Supports both PKCS#1 and PKCS#8 PEM key formats.

## Cost

~$0.05 per cycle via OpenRouter → ~$5/day at 96 cycles.

## License

MIT
