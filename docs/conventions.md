# Naming Conventions

## Events

Format: `<Entity><Action><Outcome>`

Examples:
- `CartItemAdded`
- `PaymentCaptureFailed`
- `UserLoginSucceeded`
- `OrderCreated`

Failure events end with `Failed`, `Rejected`, or `Timeout`:
- `CartItemAddFailed`
- `LoginRejected`
- `PaymentGatewayTimeout`

## Metrics

Format: `<feature>_<measurement>`

Examples:
- `add_item_latency_ms`
- `payment_failure_rate`
- `login_duration_ms`
- `session_creation_success_rate`

## Errors

Format: `snake_case_failure_reason`

Examples:
- `item_not_found`
- `payment_gateway_timeout`
- `session_expired`
- `invalid_input`

## Feature IDs

Format: `lowercase_underscore`

Examples:
- `add_item_to_cart`
- `user_login`
- `process_payment`
- `search_listings`

## Correlation IDs

- Format: UUID v4
- Required on ALL events without exception
- Must propagate through the entire feature execution chain
- Every log line during feature execution must include `correlation_id`
