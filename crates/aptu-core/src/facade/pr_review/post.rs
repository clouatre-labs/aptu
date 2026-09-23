// SPDX-License-Identifier: Apache-2.0

//! PR review posting: inline-comment dedup and summary writeback.

use tracing::{debug, instrument};

use super::DEFAULT_COMMENT_SIDE;
use super::analyze::DedupOutcome;
use crate::ai::types::{PrReviewComment, ReviewEvent};
use crate::auth::TokenProvider;
use crate::error::AptuError;
use crate::facade::issues::{WriteOutcome, permission_allows};
#[cfg(not(target_arch = "wasm32"))]
use crate::github::auth::create_client_from_provider;
use crate::github::graphql::ViewerPermission;
#[cfg(not(target_arch = "wasm32"))]
use crate::github::graphql::fetch_repo_viewer_permission;
use crate::github::pulls::{ReviewPostOutcome, SummaryPostOutcome};
#[cfg(not(target_arch = "wasm32"))]
use crate::github::pulls::{post_pr_review as gh_post_pr_review, update_pr_review_comment};
/// Pure helper shared by [`post_pr_review`] and its tests: resolves the dedup map
/// key for an existing review comment. Falls back to `original_line` when `line`
/// is `None` (comment is outdated after a re-push) so the key still matches an
/// outgoing comment targeting the original line. Returns `None` when no usable
/// line exists; such entries are excluded from the map (general PR comments are
/// never inline duplicates).
pub(crate) fn resolve_key(
    path: &str,
    line: Option<u64>,
    original_line: Option<u64>,
    side: Option<String>,
) -> Option<(String, u64, String)> {
    let line = line.or(original_line)?;
    let side = side.unwrap_or_else(|| DEFAULT_COMMENT_SIDE.to_string());
    Some((path.to_string(), line, side))
}

/// Pure helper shared by [`post_pr_review`] and its tests: builds the dedup map
/// from existing review comments keyed on `(path, line, side)`. The map value is
/// `(comment id, body)` so a duplicate can be updated in place when the rendered
/// body differs from what was previously posted.
///
/// A comment is only treated as owned by Aptu when it carries the marker AND
/// its author is a bot (`user.type == "Bot"`, surfaced as the `is_bot` flag on
/// `PrReviewCommentDetails`). GitHub
/// only assigns the `Bot` type to GitHub App / bot accounts, so a human cannot
/// spoof ownership by posting the marker. When the authenticated login is
/// resolvable (user PAT / OAuth token), the author must additionally match it —
/// this keeps multiple bot accounts from clobbering each other. When the login
/// cannot be resolved (GitHub App installation token), any Bot-type author
/// carrying the marker is accepted; the worst case is another bot's marker
/// comment being updated in place, a documented and acceptable tradeoff.
pub(crate) fn build_dedup_map(
    comments: &[crate::ai::types::PrReviewCommentDetails],
    authenticated_login: Option<&str>,
) -> std::collections::HashMap<(String, u64, String), (u64, String)> {
    comments
        .iter()
        .filter(|c| {
            c.body
                .trim_start()
                .starts_with(crate::triage::REVIEW_COMMENT_MARKER)
        })
        .filter(|c| c.is_bot)
        .filter(|c| authenticated_login.is_none_or(|login| c.author == login))
        .filter_map(|c| {
            resolve_key(&c.path, c.line, c.original_line, c.side.clone())
                .map(|key| (key, (c.id, c.body.clone())))
        })
        .collect()
}

/// Pure helper shared by [`post_pr_review`] and its tests: given the dedup map
/// and an outgoing comment, determines whether to post, skip, or update.
///
/// Comments with `line = None` always return `Post` (general PR comments are never
/// inline duplicates). The map key is `(path, line, side)` -- `commit_id` is
/// intentionally excluded so the dedup survives re-pushes (existing comments retain
/// their original SHA; only the key shape must be stable across pushes).
///
/// Comparison is keyed on the stored content hash (see
/// [`crate::triage::comment_content_hash`]) embedded in the rendered body:
/// identical semantic content yields `Skip` even when the rendering format
/// changed, and changed content yields `Update`. Legacy bodies posted before
/// the hash marker existed have no stored hash; for those the dedup falls back
/// to full rendered-body equality for one transition -- the update rewrites the
/// body with the hash marker, so the fallback self-heals after at most one
/// update.
pub(crate) fn dedup_outcome(
    dedup: &std::collections::HashMap<(String, u64, String), (u64, String)>,
    comment: &PrReviewComment,
) -> DedupOutcome {
    let Some(line) = comment.line.map(u64::from) else {
        return DedupOutcome::Post;
    };
    let key = (comment.file.clone(), line, DEFAULT_COMMENT_SIDE.to_string());
    let Some((existing_id, existing_body)) = dedup.get(&key) else {
        return DedupOutcome::Post;
    };
    let rendered = crate::triage::render_pr_review_comment_body(comment);
    let incoming_hash = crate::triage::comment_content_hash(comment);
    let matches = if let Some(stored_hash) = crate::triage::extract_comment_hash(existing_body) {
        stored_hash == incoming_hash
    } else {
        // Legacy body: compare against the legacy-style rendering (hash
        // line stripped) so an unchanged legacy comment is skipped without
        // a migration update; an update rewrites the body with the hash
        // marker, so the fallback self-heals after at most one update.
        debug!(
            comment_file = %comment.file,
            "inline comment body lacks hash marker; using legacy body-equality fallback"
        );
        rendered == *existing_body || crate::triage::strip_comment_hash(&rendered) == *existing_body
    };
    if matches {
        DedupOutcome::Skip
    } else {
        DedupOutcome::Update {
            comment_id: *existing_id,
            rendered_body: rendered,
        }
    }
}

/// Decision returned by [`summary_dedup_outcome`] for how to handle the Aptu
/// review summary comment (the issue comment carrying the
/// `<!-- APTU_REVIEW:<sha> -->` marker).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SummaryDedupOutcome {
    /// No existing summary comment; create one after posting the review.
    Post,
    /// Existing summary comment already covers this head SHA; skip entirely.
    Skip,
    /// Existing summary comment covers a different (or unknown) head SHA;
    /// patch it in place with the given comment ID.
    Update { comment_id: u64 },
}

/// Pure helper shared by [`post_pr_review`] and its tests: given the existing
/// Aptu summary comment (if any) and the current head SHA, decide whether to
/// post, skip, or update the summary.
///
/// Ownership of the existing comment is decided by the caller using the same
/// marker + Bot-type-author detection as the inline-comment dedup map (see
/// [`build_dedup_map`]); never via `octocrab current().user()`, which fails on
/// GitHub App installation tokens (see #1639). A legacy SHA-less marker is
/// treated as stale so the summary is refreshed. The same-SHA skip is
/// best-effort under concurrent runs (TOCTOU accepted; the worst case is a
/// duplicate summary, mitigated by patch-oldest on collision).
pub(crate) fn summary_dedup_outcome(
    existing: Option<(u64, Option<String>)>,
    head_sha: &str,
) -> SummaryDedupOutcome {
    match existing {
        None => SummaryDedupOutcome::Post,
        Some((comment_id, sha)) => match sha {
            Some(sha) if sha == head_sha => SummaryDedupOutcome::Skip,
            _ => SummaryDedupOutcome::Update { comment_id },
        },
    }
}

/// Posts a PR review to GitHub.
///
/// This function abstracts the credential resolution and API client creation,
/// allowing platforms to provide credentials via `TokenProvider` implementations.
///
/// # Arguments
///
/// * `provider` - Token provider for GitHub credentials
/// * `reference` - PR reference (URL, owner/repo#number, or number)
/// * `repo_context` - Optional repository context for bare numbers
/// * `summary_body` - Summary comment text (the single summary surface; posted
///   as an issue comment carrying the `<!-- APTU_REVIEW:<sha> -->` marker)
/// * `review_body` - PR review body text; must not contain the rendered summary
/// * `event` - Review event type (Comment, Approve, or `RequestChanges`)
/// * `comments` - Inline review comments; entries with `line = None` are silently skipped
/// * `commit_id` - Head commit SHA; omitted from the API payload when empty
/// * `existing_comments` - Existing inline review comments for dedup
/// * `dedup_summary` - When true, deduplicate the review summary against a
///   prior issue comment carrying the `<!-- APTU_REVIEW:<sha> -->` marker
///
/// # Returns
///
/// `ReviewPostOutcome` with the review ID and any per-comment fallback failures
/// (see [`crate::github::pulls::post_pr_review`] for the 422 fallback behavior).
/// The `summary` field reports Posted/Updated/Skipped for the summary comment.
/// The same-SHA skip is best-effort under concurrent runs (TOCTOU accepted;
/// worst case is a duplicate summary, mitigated by patch-oldest on collision).
///
/// # Errors
///
/// Returns an error if:
/// - GitHub token is not available from the provider
/// - PR cannot be parsed or found
/// - User lacks write access to the repository
/// - API call fails
#[cfg(not(target_arch = "wasm32"))]
#[instrument(skip(provider, comments, existing_comments), fields(reference = %reference, event = %event))]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub async fn post_pr_review(
    provider: &dyn TokenProvider,
    reference: &str,
    repo_context: Option<&str>,
    summary_body: &str,
    review_body: &str,
    event: ReviewEvent,
    comments: &[PrReviewComment],
    commit_id: &str,
    existing_comments: &[crate::ai::types::PrReviewCommentDetails],
    dedup_summary: bool,
) -> crate::Result<WriteOutcome<ReviewPostOutcome>> {
    use crate::github::issues::{create_issue_comment, list_issue_comments, update_issue_comment};
    use crate::github::pulls::parse_pr_reference;
    use crate::triage::parse_aptu_summary_marker;

    // Parse PR reference
    let (owner, repo, number) =
        parse_pr_reference(reference, repo_context).map_err(|e| AptuError::GitHub {
            message: e.to_string(),
        })?;

    // Create GitHub client from provider
    let client = create_client_from_provider(provider)?;

    // Gate on viewer permission: skip with an informational signal when denied
    let perm = fetch_repo_viewer_permission_cached(&client, &owner, &repo).await;
    if !pr_write_allowed(perm) {
        tracing::info!(
            repo = %format!("{owner}/{repo}"),
            "Viewer lacks write access; skipping PR review"
        );
        return Ok(WriteOutcome::Skipped);
    }

    // Build dedup map from existing review comments keyed on (path, line, side).
    // Comments with no usable line (line=None and original_line=None; general PR
    // comments) are excluded from the dedup map (they will never match an inline
    // comment which always has a line). Ownership requires BOTH the body marker
    // AND a Bot-type author (`user.type == "Bot"`, which GitHub only assigns to
    // bot/GitHub App accounts, so humans cannot spoof it). When the authenticated
    // login is resolvable (user PAT / OAuth token), the author must additionally
    // match it; when it is not (installation token), any Bot-type author carrying
    // the marker is accepted so dedup still works (see #1639).
    let authenticated_login = match client.current().user().await {
        Ok(user) => Some(user.login),
        Err(e) => {
            tracing::warn!(
                error = %e,
                "Could not resolve authenticated user; accepting any Bot-type marker comment as owned"
            );
            None
        }
    };
    let dedup = build_dedup_map(existing_comments, authenticated_login.as_deref());

    // Filter out outgoing comments that match an existing bot-authored comment.
    // General PR comments (line=None) are never checked against the dedup map.
    let mut filtered: Vec<PrReviewComment> = Vec::new();
    for c in comments {
        match dedup_outcome(&dedup, c) {
            DedupOutcome::Post => {
                filtered.push(c.clone());
            }
            DedupOutcome::Skip => {
                debug!(
                    path = %c.file,
                    line = ?c.line,
                    "Skipping duplicate inline comment (body unchanged)"
                );
            }
            DedupOutcome::Update {
                comment_id,
                rendered_body,
            } => {
                debug!(
                    path = %c.file,
                    line = ?c.line,
                    comment_id = comment_id,
                    "Updating duplicate inline comment with revised body"
                );
                if let Err(e) =
                    update_pr_review_comment(&client, &owner, &repo, comment_id, &rendered_body)
                        .await
                {
                    debug!(error = %e, "Failed to update duplicate inline comment; skipping");
                }
            }
        }
    }

    // Summary dedup: locate a prior Aptu summary issue comment (marker +
    // Bot-type author, never current().user(); see #1639). Same SHA -> skip
    // entirely; changed/legacy SHA -> patch in place; none -> create one.
    // Best-effort under concurrent runs (TOCTOU accepted; patch-oldest on
    // collision).
    let mut summary_outcome = SummaryPostOutcome::Posted;
    let mut pending_summary_update: Option<u64> = None;
    if dedup_summary {
        let existing = list_issue_comments(&client, &owner, &repo, number)
            .await
            .map_err(crate::error::aptu_error_from_anyhow)?
            .into_iter()
            .find(|c| c.is_bot && parse_aptu_summary_marker(&c.body).is_some())
            .map(|c| {
                let marker = parse_aptu_summary_marker(&c.body);
                (c.id, marker.and_then(|m| m.sha))
            });
        match summary_dedup_outcome(existing, commit_id) {
            SummaryDedupOutcome::Skip => {
                debug!("Head SHA unchanged; skipping review and summary comment");
                return Ok(WriteOutcome::Applied(ReviewPostOutcome {
                    review_id: 0,
                    failed_comments: Vec::new(),
                    summary: SummaryPostOutcome::Skipped,
                }));
            }
            SummaryDedupOutcome::Update { comment_id } => {
                debug!(comment_id = comment_id, "Updating existing summary comment");
                summary_outcome = SummaryPostOutcome::Updated;
                pending_summary_update = Some(comment_id);
            }
            SummaryDedupOutcome::Post => {}
        }
    }

    // Post the review
    let mut outcome = gh_post_pr_review(
        &client,
        &owner,
        &repo,
        number,
        review_body,
        event,
        &filtered,
        commit_id,
    )
    .await
    .map_err(crate::error::aptu_error_from_anyhow)?;

    // Create or update the summary comment (always, even with dedup disabled:
    // --no-dedup-summary bypasses the lookup but still posts the summary).
    if let Some(comment_id) = pending_summary_update {
        update_issue_comment(&client, &owner, &repo, comment_id, summary_body)
            .await
            .map_err(crate::error::aptu_error_from_anyhow)?;
    } else {
        create_issue_comment(&client, &owner, &repo, number, summary_body)
            .await
            .map_err(crate::error::aptu_error_from_anyhow)?;
    }
    outcome.summary = summary_outcome;

    Ok(WriteOutcome::Applied(outcome))
}

#[cfg(target_arch = "wasm32")]
#[allow(clippy::too_many_arguments)]
pub async fn post_pr_review(
    _provider: &dyn crate::auth::TokenProvider,
    _reference: &str,
    _repo_context: Option<&str>,
    _summary_body: &str,
    _review_body: &str,
    _event: crate::ai::types::ReviewEvent,
    _comments: &[crate::ai::types::PrReviewComment],
    _commit_id: &str,
    _existing_comments: &[crate::ai::types::PrReviewCommentDetails],
    _dedup_summary: bool,
) -> crate::Result<WriteOutcome<ReviewPostOutcome>> {
    crate::facade::wasm_unsupported!("post_pr_review");
}
/// Cache TTL for viewer permission lookups; short enough that revocations are
/// picked up quickly while avoiding redundant GraphQL calls when multiple write
/// operations target the same repository in one execution.
#[cfg(not(target_arch = "wasm32"))]
const VIEWER_PERMISSION_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(60);

#[cfg(not(target_arch = "wasm32"))]
type ViewerPermissionCache =
    std::collections::HashMap<(String, String), (std::time::Instant, Option<ViewerPermission>)>;

/// Process-wide cache of viewer permission lookups keyed by `(owner, repo)`.
/// The cache is process-local: entries live only for the lifetime of the
/// process and are dropped on exit. Within the 60s TTL a cached entry may
/// serve a stale permission result during very long-running bulk
/// operations; this is an accepted tradeoff.
///
/// # Assumption: one authenticated viewer per process
///
/// The cache key is not scoped to the authenticated identity because
/// `octocrab::Octocrab` does not expose its credential for fingerprinting.
/// The cache therefore assumes a single authenticated viewer per process,
/// which holds for all current consumers (the `aptu` CLI and the GitHub
/// Action each resolve one `TokenProvider` per execution). Library embedders
/// that rotate tokens across multiple viewers within one process must not
/// rely on this cache; a second viewer would observe the first viewer's
/// cached permission for the same `owner/repo` within the TTL window.
/// If multi-token support is ever needed, extend the cache key with a
/// credential fingerprint (e.g. a token hash) rather than removing the cache.
#[cfg(not(target_arch = "wasm32"))]
fn viewer_permission_cache() -> &'static std::sync::Mutex<ViewerPermissionCache> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<ViewerPermissionCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Fetches the viewer permission for `owner/repo`, memoizing the result across
/// calls within the cache TTL to avoid redundant GraphQL round-trips when
/// several write operations (e.g. posting a review and applying labels) run
/// against the same repository in a single execution. Because the cache is
/// process-local with a 60s TTL, results may be stale for permissions changed
/// mid-run by an external actor.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn fetch_repo_viewer_permission_cached(
    client: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
) -> Option<ViewerPermission> {
    let key = (owner.to_string(), repo.to_string());
    if let Ok(cache) = viewer_permission_cache().lock()
        && let Some((fetched_at, perm)) = cache.get(&key)
        && fetched_at.elapsed() < VIEWER_PERMISSION_CACHE_TTL
    {
        debug!(repo = %format!("{owner}/{repo}"), "Viewer permission cache hit");
        return *perm;
    }

    let perm = fetch_repo_viewer_permission(client, owner, repo)
        .await
        .ok()
        .flatten();
    if let Ok(mut cache) = viewer_permission_cache().lock() {
        cache.insert(key, (std::time::Instant::now(), perm));
    }
    perm
}

/// Gate predicate for PR write paths: denies only explicitly below-WRITE
/// viewer permissions (same semantics as [`super::issues::can_write`]).
pub(crate) fn pr_write_allowed(perm: Option<ViewerPermission>) -> bool {
    permission_allows(perm.map(|p| p.to_string()).as_deref())
}
