# Model Reference

Valid model IDs for each provider supported by Squad Station.
Use these values in the `model:` field of `squad.yml`.

All IDs below are confirmed from live sources (fetched 2026-04-24).

---

## Claude Code (`provider: claude-code`)

Source: https://docs.anthropic.com/en/docs/about-claude/models/overview

| Model ID | API alias | Tier | Summary |
|---|---|---|---|
| `claude-opus-4-7` | `claude-opus-4-7` | Most capable | Complex reasoning, long-horizon agentic tasks; 1M ctx, 128k output |
| `claude-sonnet-4-6` | `claude-sonnet-4-6` | Balanced | Best speed/intelligence ratio; 1M ctx, 64k output |
| `claude-haiku-4-5-20251001` | `claude-haiku-4-5` | Fast | Fastest, near-frontier intelligence; 200k ctx, 64k output |

**Tier shortnames** — `opus`, `sonnet`, `haiku` — are accepted by Claude Code CLI and kept
for backward compatibility with existing configs. They are not listed in the Anthropic API
docs; verify with `claude --help` if you hit issues.

**Legacy IDs** (still valid per Anthropic docs):
`claude-opus-4-6`, `claude-sonnet-4-5`, `claude-opus-4-5`, `claude-opus-4-1`

---

## Codex CLI (`provider: codex`)

Source: https://developers.openai.com/codex/models (fetched 2026-04-24)
Source: https://github.com/openai/codex issue #486 (default model confirmation)

| Model ID | Tier | Notes |
|---|---|---|
| `gpt-5.5` | Most capable | Requires ChatGPT sign-in; not available with API key auth |
| `gpt-5.4` | Flagship | Professional coding; available with API key |
| `gpt-5.4-mini` | Fast | Lightweight, ~2× faster for routine tasks |
| `gpt-5.3-codex` | Specialized | Optimized for complex software engineering |
| `gpt-5.3-codex-spark` | Preview | ChatGPT Pro only; text-only research preview |
| `gpt-5.2` | Legacy | Previous general-purpose model |
| `o4-mini` | Reasoning/Fast | Default model (confirmed via GitHub issue #486) |

---

## Gemini CLI (`provider: gemini-cli`)

Source: https://github.com/google-gemini/gemini-cli
`packages/core/src/config/models.ts` (DEFAULT_* and PREVIEW_* constants)

| Model ID | Constant | Tier | Summary |
|---|---|---|---|
| `gemini-2.5-pro` | `DEFAULT_GEMINI_MODEL` | Most capable | Stable; highest capability |
| `gemini-2.5-flash` | `DEFAULT_GEMINI_FLASH_MODEL` | Balanced | Stable; fast with 1M context |
| `gemini-2.5-flash-lite` | `DEFAULT_GEMINI_FLASH_LITE_MODEL` | Fast | Stable; optimized for speed |
| `gemini-3.1-pro-preview` | `PREVIEW_GEMINI_3_1_MODEL` | Preview | Enhanced reasoning |
| `gemini-3.1-flash-lite-preview` | `PREVIEW_GEMINI_3_1_FLASH_LITE_MODEL` | Preview | Latest lightweight preview |
| `gemini-3-flash-preview` | `PREVIEW_GEMINI_FLASH_MODEL` | Preview | High-speed next-gen |
| `gemini-3-pro-preview` | `PREVIEW_GEMINI_MODEL` | Preview | Next-gen pro |

**CLI shorthand aliases** (GEMINI_MODEL_ALIAS_* constants in models.ts):
`auto`, `pro`, `flash`, `flash-lite`

> Preview models receive 2 weeks deprecation notice.

---

## Updating this file

When providers release new models, update **both** this file **and** `valid_models_for()`
in `src/config.rs` — the Rust whitelist is the authoritative source for validation.

Include the source URL and fetch date when adding new entries.
