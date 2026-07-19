# Runbooks

> How to reason about known failure modes. The triage agent cites these in the
> `root_cause_hypothesis` and `suggested_fix` of a ticket.

## payments: gateway timeout burst
- **Signal:** spike in `payment.failed` with `reason="gateway_timeout"` / HTTP 504.
- **Usually:** upstream gateway slowness or a retry storm.
- **Fix direction:** bounded retries with jittered backoff; circuit breaker;
  confirm idempotency keys so retries don't double-charge.
- **Severity hint:** S2 if sustained > 5 min; S1 if capture success < SLO.

## checkout: DB connection pool exhaustion
- **Signal:** `error="pool timeout"` / "no connection available".
- **Usually:** pool too small for concurrency, or connections leaked on an error path.
- **Fix direction:** ensure connections are released on all paths; size the pool;
  add a pool-saturation metric.
- **Severity hint:** S2.

## payments: orphaned transaction
- **Signal:** `payment.captured` with no matching `order.created` (same `trace`)
  within 60s. Violates the payments invariant.
- **Usually:** order write happens after capture with no compensation on failure.
- **Fix direction:** outbox/saga pattern; retry order creation; reconcile job;
  alert on the invariant.
- **Severity hint:** S1/S2 — this loses customer money silently.

## inventory: oversell (planted code bug)
- **Signal:** `inventory.oversold` event, or stock going negative.
- **Root cause:** stock is decremented before checking availability (race/logic bug).
- **Fix direction:** check-then-decrement atomically; add a regression test. This
  is the defect the **coding agent** is expected to fix.
- **Severity hint:** S2.
