# Local workbench automation

Mirabile automation is a local test and agent boundary, not a product remote-control service. It
adds no HTTP or WebSocket server, accepts no executable JavaScript macro, and does not alter the
normal browser database.

## Enablement and isolation

The browser bridge is compiled only with the non-default `workbench-automation` feature. It is
installed only when the page also has `mirabileAutomation=1` and a validated database query value
matching `mirabile-workbench-e2e-*` or `mirabile-workbench-dev-*`. A normal build continues to use
the `mirabile` IndexedDB database and has no bridge object.

The versioned bridge whitelist exposes snapshot, controls, typed execute, settlement, trace, macro
replay, and one-shot action-source metadata. The test-only peer exposes initialization, settled
snapshot/wait, and typed macro replay against the same isolated database. All mutations still pass
through `WorkbenchCoordinator` and `Application`; neither bridge has repository-write or arbitrary
evaluation access.

## Semantic controls and snapshots

Rendered native controls carry `data-mirabile-control`, canonical address, and semantic qualifier
attributes. `ControlAddress` is the public lookup key; scripts do not depend on DOM hierarchy,
`nth-child`, pixels, or coordinates. A descriptor includes the control kind and accessible label,
authoritative value, optional invalid local buffer, enabled/disabled reason, options with reasons,
interaction state, and entity identity.

`AutomationSnapshotV1` selects application activity, workspace/chart/view/calculation/editor state,
actual controls, coordinator/macro state, and recent trace. It excludes full Scene geometry and is
not a portable-resource or database dump.

## CLI and CDP boundary

`scripts/workbench-control.py` attaches only to Chromium's loopback debugging endpoint through the
standard-library client in `scripts/cdp_client.py`. Its stable JSON commands are `snapshot`,
`controls`, `get`, `set`, `click`, `select`, `check`, `key`, `execute`, `wait`, `trace`, `dom`,
`screenshot`, and `run`. Native interaction resolves `[data-mirabile-address]` internally after
validating the semantic address; the CLI deliberately exposes no arbitrary-evaluate command.

The scenario runner accepts checked-in JSON steps, waits for authoritative settlement, and returns
nonzero on protocol, lookup, assertion, or application failure. Macro replay uses the versioned
typed schema and settles after every action; it contains no timing or presentation primitives.

## Tests and failure artifacts

`scripts/test-workbench-e2e.sh` builds the application and Worker with automation enabled, creates a
unique Chromium profile and IndexedDB name, serves only on loopback, and uses a deterministic
1600x1000 desktop viewport for control screenshots. Semantic scenarios exercise the bridge without
DOM interaction. Control scenarios locate native elements by `ControlAddress` and send browser
click/input/change/keyboard events.

On a UI failure the runner preserves `screenshot.png`, `dom.html`, `controls.json`,
`application.json`, `trace.json`, `browser.log`, `scenario.json`, and `error.txt` under
`target/workbench-e2e-artifacts/<scenario>/`. The `artifact-smoke` expected failure proves this path.
`scripts/verify.sh` runs repository/Worker/XALEN browser contracts, both semantic levels, every
golden control journey, and the artifact proof locally; no hosted CI or external API is involved.
