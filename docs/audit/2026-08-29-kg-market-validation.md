# Audit: KG Market Validation — August 2026

Date: 2026-08-29

Scope: Market evidence for knowledge graphs in coding and code review, compared with aptu's current per-request review shape.

Companion documents: [KG Benchmark v2 — Cost + Value (ROI)](2026-08-28-kg-benchmark-v2-roi.md); `.handoff/01c-kg-market-research.json`; `.handoff/01d-audit-format-scout.json`

## Purpose

This audit asks whether knowledge graphs (KGs) for coding and code review have succeeded in the market, and whether that evidence transfers to aptu. The verdict is **NOT PERTINENT** for aptu's current per-request, diff-seeded, single-consumer graph shape. Several products and research systems demonstrate that a persistent code index or fact graph can be useful when it is built once and served repeatedly. That is a different economic and operational product from constructing a bounded graph during one review and discarding it afterward.

Market success therefore deserves factual credit, but it is not evidence that aptu's current integration improves review quality. The shape-matched evidence is aptu's direct experiment, which found no incremental catches and introduced safety and token-cost concerns.

## Scope and definitions

A **persistent prebuilt index** parses or analyzes a repository ahead of requests, stores relationships and symbols, and serves many users, agents, searches, or reviews over time. Its construction cost can be amortized across a large request volume and across consumers with different retrieval needs. Sourcegraph's precise navigation documentation describes this style of indexed code understanding, while Meta describes a fact graph queried by developer tools ([Sourcegraph precise code navigation](https://sourcegraph.com/docs/code-navigation/explanations/precise-code-navigation); [Meta Glean](https://engineering.fb.com/2021/09/22/developer-tools/glean/)).

A **per-request construction** starts from the current request, changed files, or diff, builds a graph for that operation, renders a bounded context, and then releases or expires it. Aptu's graph is diff-seeded, assembled for one review on a repository it does not own or pre-index, consumed by one review, and discarded. A cache can reduce repeated work in limited circumstances, but it does not create the multi-consumer, multi-year amortization available to a repository owner with a durable index.

The distinction is not whether both systems contain nodes and edges. It is who pays to construct them, how long they remain useful, how many requests consume them, and whether the system controls repository freshness and indexing policy.

## Market evidence

### Systems and evidence quality

| System | Graph used | Shape (persistent vs per-request) | Evidence quality |
| --- | --- | --- | --- |
| Sourcegraph SCIP | Precise symbol and relationship index for navigation and search | Persistent prebuilt index | Production; high confidence for navigation/search, not direct proof of review efficacy ([SCIP navigation](https://sourcegraph.com/docs/code-navigation/explanations/precise-code-navigation)) |
| Meta Glean | Production fact graph with Datalog-style queries for code facts | Persistent prebuilt index | Production fact graph; high confidence for search/navigation use cases ([Glean](https://engineering.fb.com/2021/09/22/developer-tools/glean/)) |
| Augment Code | Repository index combining code intelligence and retrieval | Persistent service-side index | Vendor-run study and product documentation; self-reported, with no independent replication ([real-time index](https://www.augmentcode.com/blog/a-real-time-index-for-your-codebase-secure-personal-scalable); [large repository study](https://www.augmentcode.com/blog/repo-scale-100M-line-codebase-quantized-vector-search)) |
| Greptile | Product marketing describes repository-aware review and code understanding | Service-side repository context, not demonstrated as aptu-style per-request construction | Vendor's 82% catch claim is self-reported and unvalidated by independent third parties; it is not causal proof of review quality ([Greptile comparison](https://www.greptile.com/content-library/best-ai-code-review-tools)) |
| CodeRabbit, Qodo, and Graphite | Review automation, agent workflows, and repository context; no demonstrated KG core mechanism in the cited material | Primarily service-side or request-time tooling; not evidence for a KG-shaped review benefit | Product comparisons are secondary evidence and do not establish a graph mechanism or causal review gain ([review tools comparison](https://macroscope.com/content/best-ai-code-review-tools-github-2026); [Graphite alternatives](https://codeant.ai/blogs/best-graphite-alternative-for-code-review)) |
| KGCompass, arXiv:2503.21710 | Research knowledge graph and agent workflow for repository repair | Research-built repository context, not a production multi-consumer review index | Preprint result: 58.3% SWE-bench Lite repair; a repository-repair benchmark, not PR review, and not independent production evidence ([KGCompass](https://arxiv.org/abs/2503.21710)) |

The strongest market evidence supports persistent indexing as infrastructure. Sourcegraph and Glean show durable code facts supporting navigation, search, and developer workflows. Those are successful uses, but they answer questions such as where a symbol is defined, which facts relate to it, or how to navigate a codebase. They do not establish that a graph injected into a single review causes better defect detection.

The weaker efficacy evidence needs careful labeling. Augment's studies are vendor-run and Greptile's catch percentage is self-reported. Neither claim should be presented as causal proof of review quality. KGCompass is a useful research signal, but its repair benchmark measures a different task and does not isolate the value of a graph from the rest of its system.

The named review products also do not provide a basis for saying that a KG is a core mechanism. A product can have repository indexing, retrieval, search, or agent context without using a knowledge graph in the sense relevant to aptu's optional structural graph. Absence of a demonstrated KG mechanism is not a negative efficacy finding; it is simply a reason not to infer one.

## Why the economics do not transfer

Persistent indexes amortize parsing and indexing across many consumers and many requests over years. A repository owner can refresh an index when the code changes, answer navigation queries, support search, feed multiple agents, and serve many reviewers. The fixed construction cost is spread across that workload, while stored relationships may be reused for purposes that no single review could justify.

Aptu does the opposite. It receives a review request for a repository it does not own or pre-index, seeds a bounded graph from the diff, traverses relevant structural relationships, renders context, and consumes that context once. The graph is then discarded. Construction, cache management, serialization, prompt assembly, and compatibility costs recur per review. A warm cache can help only when the same commit and graph inputs recur, and it cannot supply the durable multi-consumer workload that makes persistent indexing economical.

This is a structural mismatch rather than a tuning detail. A persistent index can spend more effort up front because future requests will repay that investment. Aptu must justify the graph against the marginal review that receives it. Its context competes for prompt budget with the diff, AST context, files, and existing call-graph context. The persistent-index market evidence cannot be transferred merely because both designs expose relationships between code entities.

## Aptu experimental evidence

The direct experiment is documented in [Audit: KG Benchmark v2 — Cost + Value (ROI)](2026-08-28-kg-benchmark-v2-roi.md). Across four synthetic fixtures, two runs per configuration, one model, and one scorer, KG produced **0/2 hits on every fixture**. No-KG caught two cases that KG missed. KG added approximately **+0.7% to +3.6% input-token overhead** on the fixtures. It also produced two KG-only confident false reassurances: the reviewer incorrectly treated a broken caller as updated and treated a wrong trait implementation as correct.

The pre-registered decision rule was satisfied: KG did not catch a defect that No-KG missed, and the clean-control false-positive condition did not provide a compensating benefit. The result is directional evidence against enabling this graph in aptu's current review path, not a claim that every graph system is ineffective.

The limitations are material. The experiment used four synthetic fixtures, one model, one scorer, and a small number of runs. Its strongest theoretical graph case was not demonstrated, and the fixtures cannot estimate prevalence across real repositories or review populations. The experiment is nevertheless the most relevant evidence here because it matches aptu's per-request, diff-seeded, single-consumer shape rather than a persistent-index product.

## Better optimization avenues

### Prompt caching

A stable system prompt, schema, instructions, and other repeated prefix material can reduce the cost and latency of repeated model requests without adding graph context. OpenAI documents prompt caching as an API capability based on reusable prompt prefixes ([OpenAI prompt caching](https://developers.openai.com/api/docs/guides/prompt-caching)). LangChain reports 49–80% reductions for stable prefixes in its deep-agent workflow; that result is framework-specific and should be treated as public evidence for a benchmark hypothesis, not a guaranteed aptu result ([LangChain deep-agent caching](https://www.langchain.com/blog/deep-agents-prompt-caching)). Aptu should measure cache hit behavior, latency, and billed input tokens while keeping request-specific diff material after the stable prefix.

### Outline and signature compression

Representing unchanged context with symbols, signatures, and docstrings before including full implementations may preserve navigational cues at lower token cost. This is a hypothesis, not an automatic improvement: the benchmark should compare defect catches, false reassurances, and token use against the current context on representative fixtures before adoption. Full implementations should remain available for changed or directly implicated code.

### Targeted structural expansion

Instead of constructing and rendering a general graph, expand only changed symbols to their callers, references, and tests. Attach provenance to every expanded item and enforce explicit token budgets. This is the cheap, review-focused version of the useful idea the graph attempted: provide a reviewer with a concrete cross-file impact slice without paying for a broad graph representation. A follow-up should test whether targeted expansion catches changed-symbol regressions that the existing AST and call-graph context miss, while separately scoring any false reassurance.

### Model tiering

Aptu already has model tiering. It selects a smaller or larger model based on estimated prompt size. The lower-risk optimization is to tune thresholds and routing using review quality, latency, and cost measurements rather than adding another context source by default. Any threshold change should be benchmarked on both small and large diffs, with a clean control and an explicit safety metric.

## Limitations

This market scan is not exhaustive. It prioritizes public sources that describe code indexing, fact graphs, review claims, research, and prompt optimization. Vendor claims are not independently validated here, and comparative product pages may simplify implementation details. No product claim is treated as causal proof of review quality.

The aptu benchmark is small-N and single-model, with synthetic fixtures and one scorer. Its verdict applies to aptu's current per-request, diff-seeded, single-consumer shape. It does not apply to a hypothetical future product that owns persistent repository indexes, serves many consumers, or can amortize construction over a durable workload.

## Recommendations

The verdict is **NOT PERTINENT** for aptu's current KG integration. Market evidence supports persistent code indexes and fact graphs for navigation, search, and repeated repository use, but it does not transfer to aptu's one-review graph economics. The direct aptu experiment found zero incremental KG hits, positive token overhead, and two unsafe KG-only reassurances. Remove the wiring unless a concrete product requirement justifies a time-boxed replacement experiment.

### Option A — Remove the KG wiring (recommended)

1. Remove the optional structural-graph adapter and review-context integration, while retaining `aptu-coder-core` because existing AST and call-graph functionality depends on it.

2. Remove graph configuration and its consistency warnings. Decide the `[graph]` migration policy explicitly; the recommended behavior is to warn and ignore legacy graph settings rather than fail configuration loading.

3. Remove graph cache, serialization, schema/version handling, and graph-specific wasm branches. Keep the existing AST, call-graph, prompt-budget, and review behavior unchanged.

4. Remove graph feature wiring from the core and CLI manifests, along with graph-only dependency declarations and lockfile changes as applicable. Do not remove the shared `aptu-coder-core` dependency.

5. Remove graph-specific metrics and the `graph_chars` and `graph_cache_hit` fields from the JSON contract, or make the contract decision explicit before implementation. The recommended contract is to remove fields that no longer describe emitted behavior and update contract tests accordingly.

6. Remove graph-only tests and documentation, but retain this audit and the direct ROI audit as decision evidence. Add or preserve regression coverage proving AST and existing call-graph context are unchanged.

7. Run the normal Rust formatting, lint, test, and wasm32 checks. Verify that review output, budget drop order, and non-graph context remain stable.

This scope removes recurring construction, cache, serialization, prompt, and maintenance cost without removing the AST or call-graph foundations that already serve review context.

### Option B — Keep gated and re-benchmark (not recommended)

Keep the feature only if maintainers have a concrete need for cross-file blast-radius analysis that existing context cannot satisfy. Before any broader use, inspect the rendered context for the broken-caller fixture and both false-reassurance cases, identify and fix a specific defect if one exists, and keep runtime enablement opt-in with a safety warning.

A justified re-benchmark would require larger and naturally occurring pull requests, faithful repository checkouts, multiple models, independent scoring, explicit provenance checks, and enough cross-file cases to isolate the proposed benefit. It should pre-register catch, miss, false-positive, and confident-false-reassurance criteria; include token, latency, cache, and serialization costs; and require a clear incremental-catch result over AST and existing call-graph context. Without that evidence, retaining the graph is maintenance debt rather than a validated optimization.

## Summary

Persistent code indexes and fact graphs have credible production value for navigation, search, and repeated repository use. Vendor review claims remain self-reported, and research repair results do not establish PR-review causality. Aptu's current graph is constructed per request, seeded by a diff, consumed once, and discarded; its economics therefore do not receive persistent-index amortization. The direct benchmark found 0/2 KG hits on every fixture, two No-KG catches, +0.7% to +3.6% input-token overhead, and two KG-only confident false reassurances. The appropriate decision for this shape is NOT PERTINENT and Option A removal, with caching, compression, targeted expansion, and model-tier tuning as better-scoped alternatives.

## Sources

- https://sourcegraph.com/docs/code-navigation/explanations/precise-code-navigation
- https://engineering.fb.com/2021/09/22/developer-tools/glean/
- https://www.augmentcode.com/blog/a-real-time-index-for-your-codebase-secure-personal-scalable
- https://www.augmentcode.com/blog/repo-scale-100M-line-codebase-quantized-vector-search
- https://www.greptile.com/content-library/best-ai-code-review-tools
- https://macroscope.com/content/best-ai-code-review-tools-github-2026
- https://codeant.ai/blogs/best-graphite-alternative-for-code-review
- https://arxiv.org/abs/2503.21710
- https://developers.openai.com/api/docs/guides/prompt-caching
- https://www.langchain.com/blog/deep-agents-prompt-caching
