# Q3 plan

Working notes on what's shipping this quarter. The structured tags below
let our tooling read task state without keeping a parallel JSON file in
sync. On GitHub this renders as plain markdown — the wrapping elements
are unknown HTML to the renderer, so the bullets just show their inner
text and nothing else.

## Phase 1: Foundation

<phase id="1" status="in-progress">

**Goal:** ship the *foundational* tools so phase 2 can land cleanly. We
need **all three** sub-tasks done before we move on.

- <task id="1.1" status="todo">design the schema DSL</task>
- <task id="1.2" status="todo">ship the node binding</task>
- <task id="1.3" status="done">write the README</task>

</phase>

## Phase 2: Hardening

<phase id="2" status="todo">

**Goal:** prove _performance parity_ with `scraper` and shore up the
mutator with property tests.

- <task id="2.1" status="todo">add property tests for the mutator</task>
- <task id="2.2" status="todo">benchmark the selector against scraper</task>

</phase>

End of plan.
