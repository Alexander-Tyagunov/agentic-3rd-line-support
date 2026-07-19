# Risky patterns

Named patterns the monitoring agent looks for, each with a Logging Query Language
(LQL) template. Known patterns also have a deterministic alert (see
`terraform/logging.tf`); the agent adds the emergent ones and enriches all of them.

> Always bound queries by `resource.type`, `severity`, and a `timestamp` window.

## payment_error_spike (known)
```
resource.type="cloud_run_revision"
jsonPayload.event="payment.failed"
severity>="ERROR"
```

## db_pool_exhaustion (known)
```
resource.type="cloud_run_revision"
jsonPayload.message=~"pool timeout|no connection available"
```

## orphaned_transaction (emergent — correlation)
Find captures, then check for missing `order.created` on the same trace:
```
resource.type="cloud_run_revision"
jsonPayload.event="payment.captured"
```
Then, per `trace`, confirm no:
```
resource.type="cloud_run_revision"
jsonPayload.event="order.created"
trace="projects/PROJECT_ID/traces/<TRACE_ID>"
```

## latency_creep (emergent — trend)
```
resource.type="cloud_run_revision"
jsonPayload.event="checkout"
jsonPayload.latency_ms>0
```
Compare the window's p99 against the baseline; flag sustained upward drift even
while inside SLO.

## inventory_oversell (emergent — invariant)
```
resource.type="cloud_run_revision"
jsonPayload.event="inventory.oversold"
```

## new_error_signature (emergent — novelty)
Cluster `severity>=ERROR` messages in the window; flag signatures not seen in the
prior baseline window.
