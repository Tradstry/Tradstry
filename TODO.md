# Tradstry Product Roadmap

Tradstry should close the loop between what a trader planned, what they executed,
and what they learned. The product promise is:

> Plan the risk. Sync the execution. Find the deviation. Improve the process.

Work through this roadmap in priority order. Each feature should receive its own
design and implementation plan before development begins.

## North-star metric

**Closed-loop trades:** trades that were planned, brokerage-synced, and reviewed.

Supporting measures:

- Percentage of connected users who complete their first closed-loop trade within 7 days.
- Percentage of weekly active traders who complete a weekly review.
- Percentage of brokerage fills successfully reconciled with Tradstry records.
- Change in unplanned risk and principle violations after 20 reviewed trades.

## P0 — Brokerage Sync Trust Center

Make brokerage data understandable and trustworthy before building more analytics
on top of it.

- [x] Discover the connected brokerage and its underlying brokerage accounts.
- [x] Show which brokerage account is assigned to the current Tradstry workspace.
- [x] Show the last successful sync and current sync state.
- [x] Show the next scheduled sync.
- [x] Display sync progress instead of leaving the user with an unresponsive button.
- [x] Show the latest imported transaction, holding, and balance totals.
- [x] Show skipped, duplicated, pending, and failed transaction counts.
- [x] Detect missing fills and balance discrepancies.
- [x] Reconcile broker fill counts against Tradstry fill counts.
- [x] Present actionable connection errors, including reauthorization instructions.
- [x] Add retry and reconnect actions with clear success and failure feedback.
- [x] Add a "Report incorrect data" flow with safe diagnostic identifiers.
- [x] Add regression coverage for multi-account workspace assignment.
- [x] Add Webull-specific fixtures for cash, margin, and mixed/event accounts.

### Done when

- A trader can tell which broker account feeds each workspace.
- A trader can see whether a sync is running, completed, delayed, or failed.
- Tradstry can explain discrepancies without requiring the user to inspect logs.

## P1 — Plan vs. Actual Trade Review

Connect position plans directly to brokerage executions and show where the trader
deviated from the plan.

- [x] Match plans to fills using workspace, symbol, direction, and time window.
- [x] Allow the user to confirm or correct an automatic match.
- [x] Compare planned entries with actual fills.
- [x] Compare planned shares and risk with actual shares and maximum risk.
- [x] Calculate entry slippage and risk-budget drift.
- [x] Detect unplanned scale-ins and skipped planned tranches.
- [x] Compare the planned stop with actual broker exits. Profit targets are
      deliberately deferred.
- [x] Calculate fee-aware realized R from the plan's frozen dollar risk.
- [x] Show which trading principles were followed or violated.
- [x] Ask the trader what caused meaningful deviations.
- [x] Save an immutable review snapshot to History and the Journal.

### Done when

- A completed trade produces a clear planned-versus-actual review.
- Every reported deviation links to the executions and plan values that caused it.
- The user can turn the review into a journal entry without re-entering trade data.

## P1 — Automatic Journal Inbox

Convert completed brokerage positions into review-ready journal drafts.

- [x] Group related fills into round-trip trades.
- [x] Pre-fill symbol, direction, executions, average entry and exit, size, and
      realized result.
- [ ] Surface fees and holding period explicitly in the review.
- [x] Attach the matching position plan when available.
- [ ] Suggest a playbook using deterministic rules before AI assistance.
- [ ] Suggest possible principle violations with supporting evidence.
- [ ] Attach a chart covering the relevant trading window.
- [x] Ask only the minimum review questions: intended setup, plan adherence, and lesson.
- [x] Support quick notes.
- [ ] Support screenshots and voice notes.
- [x] Track unreviewed drafts in the brokerage inbox.
- [ ] Send process-focused reminders for unreviewed drafts.
- [x] Handle fill merges, partial exits, overnight positions, and editable reversal
      groupings.
- [ ] Handle corporate actions, splits, and remaining unmatched-fill edge cases.

### Done when

- A normal synced trade can be journaled in under 60 seconds.
- No brokerage fill is silently dropped or incorrectly assigned.
- Automatic suggestions remain editable and visibly distinguishable from user input.

## P2 — What-if Leak Detector

Quantify the cost of repeated execution and discipline mistakes using deterministic
counterfactuals before generating an AI explanation.

- [ ] Measure results with and without trades carrying a selected violation.
- [ ] Calculate the effect of unplanned additions and excess position size.
- [ ] Compare planned stops and targets with actual trade management.
- [ ] Compare planned and improvised executions within the same playbook.
- [ ] Identify setup, time-of-day, symbol, and market-condition interactions.
- [ ] Display sample size, assumptions, and confidence warnings.
- [ ] Label small samples as early indications rather than conclusions.
- [ ] Link every finding to the supporting trades.
- [ ] Allow a finding to become a draft principle or playbook change.
- [ ] Track whether acting on a finding improves later process metrics.

### Done when

- Every number is reproducible without an LLM.
- AI explains evidence but does not invent calculations or causal claims.
- A trader can open the exact trades behind every conclusion.

## P2 — Lightweight Trade Replay

Build review-focused replay before considering a full trading simulator or
backtesting engine.

- [ ] Render an intraday chart with actual entries and exits.
- [ ] Overlay planned entries, stop, targets, and tranche risk.
- [ ] Provide a scrubber and variable playback speed.
- [ ] Add a mode that hides future candles during review.
- [ ] Allow timestamped notes, drawings, and annotations.
- [ ] Highlight plan deviations at the moment they occurred.
- [ ] Allow one alternate exit or trade-management scenario.
- [ ] Save lessons back to the Journal, Playbook, or Principles.
- [ ] Support sharing a replay through a permissioned read-only link.

### Done when

- A trader can reconstruct the decision sequence of a real trade.
- Replay remains a review tool and does not expand into order simulation prematurely.

## P2 — Evidence-backed AI and MCP Workflows

Make AI useful through grounded actions rather than another generic trading chatbot.

- [ ] Link every AI conclusion to supporting trades and calculations.
- [ ] Add "Show me every example" actions that open a filtered ledger.
- [ ] Convert an insight into a draft trading principle.
- [ ] Convert a strong trade into a playbook example.
- [ ] Generate process-focused daily and weekly reviews.
- [ ] Expose closed-loop review workflows through MCP.
- [ ] Verify authenticated MCP initialization, tool discovery, and tool execution.
- [ ] Add precise tool schemas, safety annotations, documentation, and golden prompts.
- [ ] Require user confirmation for AI-authored mutations.
- [ ] Measure the funnel from agent installation through the first completed review.

### Done when

- AI outputs are evidence-backed, inspectable, and safe to correct.
- The agent analyzes and drafts; the trader decides and approves.

## P3 — Coach and Mentor Review

Let a trader securely share selected evidence with a coach or accountability partner.

- [ ] Create revocable, permissioned read-only access.
- [ ] Share a selected trade, week, report, replay, or playbook.
- [ ] Allow comments on trades, executions, plans, and principles.
- [ ] Support coach-created review assignments.
- [ ] Show before-and-after discipline and process comparisons.
- [ ] Notify the trader when feedback is available.
- [ ] Add audit logs and clear access-expiration controls.
- [ ] Prevent shared viewers from accessing unrelated workspaces or brokerage data.

### Done when

- The trader controls exactly what is shared, with whom, and for how long.
- A coach can give useful feedback without receiving brokerage credentials or full-account access.

## Deliberate non-goals for now

- [ ] Do not build a full trading simulator before validating lightweight replay.
- [ ] Do not build a large technical backtesting engine yet.
- [ ] Do not chase hundreds of additional metrics without a decision they support.
- [ ] Do not provide trade signals or tell users what security to buy or sell.
- [ ] Do not build another generic AI chat experience without evidence-backed actions.
- [ ] Do not add a social feed before closed-loop review is working and retained.

## Delivery rules for every roadmap item

- [ ] Start with the user problem and a measurable success criterion.
- [ ] Inspect the existing data model and flows before proposing architecture.
- [ ] Use deterministic calculations for financial facts; use AI only for explanation
      or genuinely unstructured work.
- [ ] Preserve workspace and brokerage-account boundaries in every query and mutation.
- [ ] Include empty, loading, partial, stale, and failed states in the design.
- [ ] Add regression coverage for multi-account brokerage behavior.
- [ ] Validate the complete flow locally with realistic data.
- [ ] Keep external mutations and AI-authored changes behind explicit user approval.
- [ ] Ship one independently valuable slice before expanding the feature.
