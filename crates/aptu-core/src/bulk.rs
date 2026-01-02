// SPDX-License-Identifier: Apache-2.0

//! Generic bulk processing with concurrent execution and retry logic.
//!
//! This module provides a reusable pattern for processing collections of items
//! concurrently with automatic retry on transient failures and progress tracking.
//! It's designed to work across all platforms (CLI, iOS/FFI, MCP) without any
//! CLI-specific dependencies.

use std::fmt::Display;

use anyhow::Result;
use backon::Retryable;
use futures::{StreamExt, stream};

use crate::{is_retryable_anyhow, retry_backoff};

/// Outcome of processing a single item in a bulk operation.
#[derive(Debug, Clone)]
pub enum BulkOutcome<T> {
    /// Item was processed successfully with a result.
    Success(T),
    /// Item was skipped (e.g., already processed).
    Skipped(String),
    /// Item processing failed with an error.
    Failed(String),
}

/// Result of a bulk processing operation.
#[derive(Debug, Clone)]
pub struct BulkResult<I, T> {
    /// Number of items processed successfully.
    pub succeeded: usize,
    /// Number of items that failed processing.
    pub failed: usize,
    /// Number of items that were skipped.
    pub skipped: usize,
    /// Detailed outcomes for each item (identifier, outcome).
    pub outcomes: Vec<(I, BulkOutcome<T>)>,
}

impl<I, T> Default for BulkResult<I, T> {
    fn default() -> Self {
        Self {
            succeeded: 0,
            failed: 0,
            skipped: 0,
            outcomes: Vec::new(),
        }
    }
}

/// Process a collection of items concurrently with retry logic and progress tracking.
///
/// # Type Parameters
///
/// * `I` - Item identifier type (must be Clone + Display for progress messages)
/// * `D` - Item data type (must be Clone + Send)
/// * `T` - Result type for successful processing
/// * `F` - Async processor function type
/// * `P` - Progress callback function type
///
/// # Arguments
///
/// * `items` - Collection of (identifier, data) pairs to process
/// * `processor` - Async function that processes a single item, returning:
///   - `Ok(Some(T))` for successful processing
///   - `Ok(None)` for skipped items
///   - `Err(e)` for failures (will retry if retryable)
/// * `progress_callback` - Called before processing each item with (current, total, `action_message`)
///
/// # Returns
///
/// A `BulkResult` containing counts and detailed outcomes for all items.
///
/// # Concurrency
///
/// Uses `buffer_unordered(5)` to process up to 5 items concurrently, respecting
/// rate limits and avoiding overwhelming external APIs.
///
/// # Retry Logic
///
/// Automatically retries transient failures (network errors, rate limits) using
/// exponential backoff. Non-retryable errors (validation, permissions) fail immediately.
///
/// # Example
///
/// ```rust,no_run
/// use aptu_core::bulk::{process_bulk, BulkResult};
/// use anyhow::Result;
///
/// async fn process_item(id: &str) -> Result<Option<String>> {
///     // Process the item...
///     Ok(Some(format!("Processed {}", id)))
/// }
///
/// # async fn example() -> Result<()> {
/// let items = vec![
///     ("item1".to_string(), "data1"),
///     ("item2".to_string(), "data2"),
/// ];
///
/// let result = process_bulk(
///     items,
///     |(_id, data)| async move { process_item(data).await },
///     |current, total, action| {
///         println!("[{}/{}] {}", current, total, action);
///     },
/// ).await;
///
/// println!("Succeeded: {}, Failed: {}, Skipped: {}",
///     result.succeeded, result.failed, result.skipped);
/// # Ok(())
/// # }
/// ```
pub async fn process_bulk<I, D, T, F, Fut, P>(
    items: Vec<(I, D)>,
    processor: F,
    progress_callback: P,
) -> BulkResult<I, T>
where
    I: Clone + Display + Send + 'static,
    D: Clone + Send + 'static,
    T: Send + 'static,
    F: Fn((I, D)) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<Option<T>>> + Send,
    P: Fn(usize, usize, &str) + Send + Sync + 'static,
{
    let total = items.len();
    let progress_callback = std::sync::Arc::new(progress_callback);
    let processor = std::sync::Arc::new(processor);

    // Process items concurrently with buffer_unordered(5) for rate limit awareness
    let mut tasks = Vec::new();
    for (idx, (id, data)) in items.into_iter().enumerate() {
        let id_clone = id.clone();
        let data_clone = data.clone();
        let progress_callback = progress_callback.clone();
        let processor = processor.clone();

        let task = async move {
            // Call progress callback before processing
            progress_callback(idx + 1, total, &format!("Processing {id}"));

            // Process with retry logic
            let id_for_retry = id_clone.clone();
            let data_for_retry = data_clone.clone();
            let result = (|| {
                let processor = processor.clone();
                let id = id_for_retry.clone();
                let data = data_for_retry.clone();
                async move { processor((id, data)).await }
            })
            .retry(retry_backoff())
            .when(is_retryable_anyhow)
            .notify(|err, dur| {
                tracing::warn!(
                    error = %err,
                    delay_ms = dur.as_millis(),
                    item = %id_clone,
                    "Retrying after transient failure"
                );
            })
            .await;

            (id_clone, result)
        };

        tasks.push(task);
    }

    let outcomes = stream::iter(tasks)
        .buffer_unordered(5)
        .collect::<Vec<_>>()
        .await;

    // Categorize outcomes and build result
    let mut bulk_result = BulkResult::default();

    for (id, result) in outcomes {
        match result {
            Ok(Some(value)) => {
                bulk_result.succeeded += 1;
                bulk_result.outcomes.push((id, BulkOutcome::Success(value)));
            }
            Ok(None) => {
                bulk_result.skipped += 1;
                bulk_result
                    .outcomes
                    .push((id, BulkOutcome::Skipped("Skipped".to_string())));
            }
            Err(e) => {
                bulk_result.failed += 1;
                bulk_result
                    .outcomes
                    .push((id, BulkOutcome::Failed(e.to_string())));
            }
        }
    }

    bulk_result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_successful_processing() {
        let items = vec![
            ("item1".to_string(), 1),
            ("item2".to_string(), 2),
            ("item3".to_string(), 3),
        ];

        let result = process_bulk(
            items,
            |(id, value)| async move { Ok(Some(format!("{}: {}", id, value * 2))) },
            |_current, _total, _action| {},
        )
        .await;

        assert_eq!(result.succeeded, 3);
        assert_eq!(result.failed, 0);
        assert_eq!(result.skipped, 0);
        assert_eq!(result.outcomes.len(), 3);
    }

    #[tokio::test]
    async fn test_mixed_outcomes() {
        let items = vec![
            ("success".to_string(), 1),
            ("skip".to_string(), 2),
            ("fail".to_string(), 3),
        ];

        let result = process_bulk(
            items,
            |(id, _value)| async move {
                match id.as_str() {
                    "success" => Ok(Some("done".to_string())),
                    "skip" => Ok(None),
                    "fail" => Err(anyhow::anyhow!("Processing failed")),
                    _ => unreachable!(),
                }
            },
            |_current, _total, _action| {},
        )
        .await;

        assert_eq!(result.succeeded, 1);
        assert_eq!(result.failed, 1);
        assert_eq!(result.skipped, 1);
        assert_eq!(result.outcomes.len(), 3);
    }

    #[tokio::test]
    async fn test_progress_callback_invocation() {
        use std::sync::{Arc, Mutex};

        let items = vec![("item1".to_string(), 1), ("item2".to_string(), 2)];

        let progress_calls = Arc::new(Mutex::new(Vec::new()));
        let progress_calls_clone = progress_calls.clone();

        let _result = process_bulk(
            items,
            |(_id, _value)| async move { Ok(Some("done".to_string())) },
            move |current, total, action| {
                progress_calls_clone
                    .lock()
                    .unwrap()
                    .push((current, total, action.to_string()));
            },
        )
        .await;

        let calls = progress_calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, 1);
        assert_eq!(calls[0].1, 2);
        assert_eq!(calls[1].0, 2);
        assert_eq!(calls[1].1, 2);
    }
}
