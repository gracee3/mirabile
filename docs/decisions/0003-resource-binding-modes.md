# 0003: Follow, Pinned, and Inline bindings

Status: Accepted

## Decision

`ResourceBinding<T>` retains three materially different modes: `Follow` resolves the current
revision, `Pinned` resolves one exact revision, and `Inline` embeds a value without independent
resource identity.

## Consequences

Projection and inspectors must not fabricate resource IDs for inline values. Binding origin is
distinct from configuration precedence: where a value came from does not explain why its layer
won.
