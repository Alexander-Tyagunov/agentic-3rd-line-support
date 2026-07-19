# Service catalog

> Synthetic services emitted by `apps/synthetic-shop`. Used to attribute findings
> to an owner and understand blast radius.

## checkout
- **Owner:** payments-team
- **Depends on:** payments, inventory
- **SLO:** 99.5% of `checkout` requests succeed; p99 latency < 800 ms
- **Emits events:** `checkout`, `add_to_cart`

## payments
- **Owner:** payments-team
- **Depends on:** external payment gateway
- **SLO:** 99.9% of `payment.captured`; capture within 60s of authorization
- **Emits events:** `payment.captured`, `payment.failed`
- **Invariant:** every `payment.captured` MUST be followed by an `order.created`
  with the same `trace` within 60s. A violation = *orphaned transaction*.

## inventory
- **Owner:** catalog-team
- **SLO:** never sell below 0 stock
- **Emits events:** `inventory.reserved`, `inventory.oversold`
- **Invariant:** available stock is checked BEFORE decrement. `inventory.oversold`
  should never appear — if it does, it is a code defect (the planted bug).

## auth
- **Owner:** identity-team
- **SLO:** 99.9% token validation success
- **Emits events:** `auth.login`, `auth.denied`
