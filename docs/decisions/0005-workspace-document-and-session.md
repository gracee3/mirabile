# 0005: Workspace document and session lifetimes

Status: Accepted

## Decision

`WorkspaceDocument` stores durable, intentional composition such as saved chart instances, view
instances, slot assignments, resource bindings, and durable display/layout configuration.
`WorkspaceSession` stores current interaction such as active/selected charts and views, unsaved
draft charts, and temporary display overrides.

Durable edits update the session's document projection and mark it dirty. Only explicit Save
Workspace writes the next canonical document revision. A temporary override can be promoted into
durable configuration, which makes the document dirty.

## Consequences

Navigation and temporary interaction do not create canonical revisions. Unsaved session state may
be lost at session end; later device-local recovery can be added without polluting portable state.
