# Audit: Full Simplification Audit — September 2026

Date: 2026-09-20  
Base: 34a73b8 (v0.10.21)  
Scope: entire aptu workspace (crates/aptu-cli, crates/aptu-core, action.yml, ~37.6k LOC Rust) + companion aptu-github-app workflows  
Evidence: four parallel research passes (provider/legacy registry · CLI surface & external-usage sweep · modularity/long-file analysis · aptu-github-app integration & bug forensics), each verified against the actual tree; runtime evidence from clouatre-labs/clouatre.ca#1432

## Question

~99% of aptu usage is now `aptu pr review` (via `aptu-github-app` and the composite action) plus `issue triage`, `pr label`, `pr queue`, and `scan-security`. Everything is on the table: what can be deleted, what is legacy, where are the bugs, and what should be restructured for maintainability and performance?

## Answer (short)

1. **The biggest wins are deletions, not optimizations.** ~5,000–6,500 LOC (~15% of the workspace) is removable without touching any kept command: unused providers (Cerebras/Groq/ZenMux, pure registry data), the repository-curation subsystem, create/revert commands plus their git/patch.rs + sanitize.rs cascade, the `history` command, `models list`, and yaml/markdown output renderers. A further ~2,400 LOC lever — the AST/call-graph deep-review context — is unreachable in the dominant path (`--deep` is never set by the app) and deserves an explicit keep/remove decision (#1651).
2. **The duplicate-comment bug has two layers.** The inline dedup map is silently disabled under GitHub App installation tokens because the existing-comment fetch is gated on `client.current().user()` resolving (`pulls.rs:267-281`), so every re-run posts fresh comments (#1639, reproduced on clouatre.ca#1432: six near-identical comments at `public/knowledge-graph.json:189`). Separately, the review summary has no idempotency at all: the `<!-- APTU_REVIEW -->` marker is write-only and `gh_post_pr_review` is called unconditionally (#1640).
3. **The remaining code is reasonably factored below file level** — no dead-code warnings, no `#[deprecated]`, no FIXME/HACK debt. The maintainability work is concentrated in a handful of god-functions and three oversized files (`pulls.rs` 2,081; `review_context.rs` 1,943; `prompts/mod.rs` 1,424), plus duplicated reference parsing that is a latent correctness risk.

## Summary Table

| # | Finding | Verdict | Recommendation | Priority | Issue |
|---|---------|---------|----------------|----------|-------|
| F1 | Inline-comment dedup map empty under App installation tokens: existing-comment fetch gated on `client.current().user()` (`pulls.rs:267-281`); failure → `bot_login=""` → fetch skipped entirely; outdated comments with `line=null` also excluded (`pr_review.rs:417-418`) | CONFIRMED (reproduced) | Decouple fetch from bot-identity resolution; body-marker filtering; regression test under installation-token identity | HIGH | [#1639](https://github.com/clouatre-labs/aptu/issues/1639) |
| F2 | Review summary re-posted on every re-run: `<!-- APTU_REVIEW -->` marker written (`triage.rs:322-325`) but never read; `gh_post_pr_review` unconditional (`pr_review.rs:462`); action always passes `--force` (`action.yml:612`) | CONFIRMED | Marker carries head SHA; same-SHA skip, changed-SHA update-in-place | HIGH | [#1640](https://github.com/clouatre-labs/aptu/issues/1640) |
| F3 | Cerebras, Groq, ZenMux providers unused; pure data entries in registry (`consts.rs:17-24`, `config.rs:55-79`, `parsing.rs:285-344`) + action inputs + ~6 doc files | CONFIRMED | Remove (~40 LOC code + action inputs + docs; breaking) | HIGH | [#1641](https://github.com/clouatre-labs/aptu/issues/1641) |
| F4 | Repository-curation subsystem (`repos.toml`, `repos/` 820 LOC, `facade/repos.rs` 333, discovery 354, CLI `repo` tree): zero external invocations | CONFIRMED | Remove (~1,500 LOC) | HIGH | [#1642](https://github.com/clouatre-labs/aptu/issues/1642) |
| F5 | `issue/pr create` + `issue/pr revert` unused; hidden keystone dragging `git/patch.rs` (472), `sanitize.rs` (280), `tests/patch_integration.rs` (353) | CONFIRMED | Remove (~1,400+ LOC cascade; audit `security/validator.rs` callers first) | HIGH | [#1643](https://github.com/clouatre-labs/aptu/issues/1643) |
| F6 | `aptu history` command never invoked in the ~99% path; `effective_token_units` legacy deserialization shim (`history.rs:126-128`) is dead | CONFIRMED | Remove command + renderers + shim; keep minimal `AiStats` recording feeding action outputs | MEDIUM | [#1644](https://github.com/clouatre-labs/aptu/issues/1644) |
| F7 | `aptu models list` is docs-only convenience (~600 LOC); registry and tier routing unaffected | CONFIRMED (usage) | Remove; point docs at provider dashboards (maintainer decision flagged) | LOW | [#1645](https://github.com/clouatre-labs/aptu/issues/1645) |
| F8 | `--output yaml` and `--output markdown` renderers unreferenced anywhere; only json/github-annotations/sarif consumed; `render_text` scaffolding duplicated 5× (`output/pr.rs:267-373`) | CONFIRMED | Trim to `json\|github-annotations\|sarif`; extract shared table builder | MEDIUM | [#1646](https://github.com/clouatre-labs/aptu/issues/1646) |
| F9 | `action.yml` dead inputs (`command`/`subcommand` declared at `:292-297`, referenced by no step) plus 17 inputs never set by the only consumer; all outputs unused except `pr-review-outcome` | CONFIRMED | Delete dead inputs; prune unused set input-by-input; trim outputs | MEDIUM | [#1647](https://github.com/clouatre-labs/aptu/issues/1647) |
| F10 | God-functions: `run_issue_command` 234 lines (`commands/mod.rs:593-826`), `run_pr_command` 268 lines (`:827-1094`); `cli_outcome` match duplicated at `:700`/`:927`; cli tests repeat run/parse boilerplate ~20× | CONFIRMED | Per-subcommand handlers + single `report_outcome()` + `run_cli()` test helper | MEDIUM | [#1648](https://github.com/clouatre-labs/aptu/issues/1648) |
| F11 | `fetch_pr_details` 277 lines doing five jobs (`pulls.rs:93-369`); `parse_pr_reference`/`parse_owner_repo` duplicated across `pulls.rs:65`, `issues.rs:92`, `issues.rs:65`, `github/mod.rs:76` — drift risk | CONFIRMED | Split into core/files/comments fetchers; centralize parsing in `github/mod.rs` (also prerequisite for a clean #1639 fix) | MEDIUM | [#1649](https://github.com/clouatre-labs/aptu/issues/1649) |
| F12 | Three oversized AI-layer files mixing concerns: `review_context.rs` 1,943 (builder+budget+truncation), `facade/pr_review.rs` 1,053 (analyze+post+dedup+label), `prompts/mod.rs` 1,424 (`build_user_prompt` 115 lines) | CONFIRMED | Split into focused modules; extract AI-call stage; keep prompt text in `.md`/`.json` files | MEDIUM | [#1650](https://github.com/clouatre-labs/aptu/issues/1650) |
| F13 | AST/call-graph deep-review context (`ast_context.rs` 532 + symbol expansion + multi-language grammar deps, ~2,400 LOC with `review_context.rs`): largest single lever; `--deep` never set by aptu-github-app; no evidence of review-quality ROI | CONFIRMED (observation) | RESOLVED (#1668): `--deep` flag and deep-only symbol-expansion path removed; auto-triggered call-graph context kept behind the budget gate | MEDIUM | [#1651](https://github.com/clouatre-labs/aptu/issues/1651) |
| F14 | aptu-github-app: action pin drift (v0.10.21 in review/triage vs **v0.10.20** in scan-security vs v0.1.4-era wrapper refs); duplicate telemetry pipeline (action `metrics-aggregate` vs custom curl rollup with `\|\| true` swallowing failures); redundant warn step; dispatch-wrapper token smell | CONFIRMED | One pinned SHA (single source of truth), one telemetry pipeline, drop redundant-provider inputs after #1641 | MEDIUM | [aptu-github-app#252](https://github.com/clouatre-labs/aptu-github-app/issues/252) |

## Evidence Detail

### Usage-weighted surface

External invocation sweep across `action.yml`, all `aptu-github-app` workflows, `scripts/`, and `bench/`: kept commands are `pr review` (~6,000 LOC incl. review context), `issue triage` (~3,700), `pr label`, `pr queue`, `scan-security` (~2,900). Everything else in the command tree has zero invocations outside its own tests and docs. The removable set (F3–F8) totals ~5,000–6,500 LOC without touching the product path; #1651 adds ~2,400 more if approved. `bulk.rs` must NOT be removed with any "bulk operations" idea — it has no CLI command but powers `issue triage --since` batch mode used by the scheduled action run.

### Bug forensics (clouatre.ca#1432)

Six inline comments at `(public/knowledge-graph.json, 189, RIGHT)` with fresh IDs (4052003390 → 4053330767) across re-runs on 2026-09-19, bodies near-identical but not equal — exactly the pattern of an empty dedup map (everything takes `DedupOutcome::Post`). The dedup design itself (`dedup_outcome`, `pr_review.rs:327-348`, commit-id deliberately excluded from the key) is sound and tested; the fetch that feeds it is what fails under the App token. F2 is a distinct gap: even a perfectly deduped inline set still re-posts the summary review.

### Legacy scan

No `#[deprecated]`, no dead-code warnings (`cargo build` clean), no old-model-name fallback lists; model strings live only in the registry and docs. The only true legacy artifact found is the `effective_token_units` shim (folded into #1644). `serde(default)` fields in `history.rs` are cheap versioning shims and stay unless #1644 removes their owner. `ast-context` and `keyring` features are live (F13 covers the decision on the former).

### Maintainability

Below file level the code is well-factored; single-use helpers worth inlining are rare. The debt is concentrated: two CLI god-functions, one 277-line fetch orchestrator, three 1,000+ LOC files, and duplicated reference parsing. All refactors are behavior-preserving and sequenced in #1648–#1650; #1649 intentionally precedes or accompanies #1639 so the dedup fix lands in testable units.

## Recommendations by ROI

1. **Fix the two review bugs** (#1639, #1640) — user-visible correctness in the 99% path; #1649 is the enabling refactor.
2. **Delete F3–F6** (#1641–#1644) — ~4,000 LOC of unused providers, curation, create/revert, history; each is an independent, low-risk PR with a breaking-change release note.
3. **Ship the app fixes** (aptu-github-app#252) — pin drift means the security scan lags review; the swallowed telemetry failures hide delivery outages.
4. **Prune the contract** (#1645–#1647) — models list, yaml/markdown output, dead action inputs.
5. **Refactor for maintainability** (#1648–#1650) — behavior-preserving splits; land #1649 with #1639.
6. **Decide on `--deep`** (#1651) — biggest remaining lever; RESOLVED via #1668: the `--deep` flag and deep-only symbol-expansion path were removed while keeping auto-triggered call-graph context behind the budget gate.

Explicit do-NOTs: do not remove `bulk.rs` (used by triage batch), do not touch the `--output json` schema (contract), do not remove `get_stale` cache fallback or the `FallbackConfig` chain (live resilience features), do not remove `history.rs`'s `AiStats` core (feeds action outputs), do not build new features on the removed `create`/`revert` machinery.

## Impact (measured facts only)

- ~5,000–6,500 LOC deleted via F3–F8 (~15% of workspace); ~9,000+ if #1651 approved; corresponding dependency shrink from tree-sitter grammars and provider registry entries; faster builds and smaller release binaries.
- F1+F2 eliminate all duplicate comments on re-review — the reported production bug.
- F9–F14 reduce contract surface the action and app must keep in lockstep (currently three independent version pins).

## Future Work (non-blocking)

- Remove orphaned `PrCreateResult`/`create_pull_request` from `github/pulls.rs` (~120 LOC left behind by F5/#1656) — [#1660](https://github.com/clouatre-labs/aptu/issues/1660).
- Pre-registered telemetry gate for #1651 (AST-context non-zero rate threshold) using existing 7-day artifacts.
- Revisit `auth` OAuth device flow (~800 LOC): never used by the action/app; keep for local users today, but a candidate if CLI-only usage keeps shrinking.
- docs/audit/ housekeeping: historical audits reference removed features once F3–F6 land.

## Sources

- Codebase: `crates/aptu-core/src/{ai,github,facade,repos,git,security}`, `crates/aptu-cli/src/{commands,output}`, `action.yml` (all paths/line numbers as of 34a73b8)
- Runtime evidence: clouatre-labs/clouatre.ca#1432 review comments (IDs 4051976433–4053330767, 2026-09-19)
- Prior art: docs/audit/ methodology (parallel passes, verdict table, pre-registered decision gates per the KG audit series)
- Companion: clouatre-labs/aptu-github-app#252
