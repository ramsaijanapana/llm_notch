# Lane 7 handoff — Native onboarding and integration UI

**Branch:** `feat/lane-7-ui`  
**Base:** `25d056a`  
**Status:** `LANE_7_COMPLETE`

## Summary

Implemented consent-lite onboarding, connect/diff/apply flows, `ConnectorUserStatus` health presentation, backup/restore, repair/disable, and capability-gated decision surfaces per contract freeze v2 and Fable UX consensus.

## Delivered UI

| Surface | Behavior |
|---------|----------|
| Onboarding (5 steps) | Consent + documented paths → Get started → detect; display; connect agents (user-scope default, per-file checkboxes); diff review + apply progress; shortcuts |
| Integrations panel | `ConnectorUserStatus` badges + guidance (Cursor hooks, Codex `/hooks`); Connect / Repair / Disable; diff review; per-file apply progress; backup list + restore preview |
| Decision surface | Dashboard/session focused; no overlay controls; hidden when `decisionResponse: false` or `!hasActionablePayload`; delivery microcopy |
| Native client seam | `detect`, `preview`, `apply`, `repair`, `rollback`, `listConnectorBackups`, `getPendingDecisions`, `respondDecision` |

## Files touched

- `src/features/native-dashboard/components/OnboardingFlow.tsx`
- `src/features/native-dashboard/components/integrations/**`
- `src/features/native-dashboard/components/decisions/DecisionSurface.tsx`
- `src/features/native-dashboard/utils/integrationLabels.ts`
- `src/features/native-dashboard/types/contracts.ts`
- `src/app/NativeSurfaces.tsx`
- `src/native/commands.ts`, `client.ts` types, `FakeNativeClient.ts`, `TauriNativeClient.ts`, `contracts.ts`
- `e2e/native-integration-flows.spec.ts`, `e2e/native-dashboard.spec.ts`

## Tests

```bash
npm run typecheck          # pass
npm run test:run           # 159 passed
npx biome check --write <new files>  # pass (repo-wide lint has pre-existing CRLF noise)
```

Playwright: `e2e/native-integration-flows.spec.ts` added (consent/detect, health/connect, diff review). Run with `npm run test:e2e` when browsers installed.

## Blockers / follow-ups for other lanes

| Item | Owner | Notes |
|------|-------|-------|
| `list_connector_backups` Tauri command | Platform/Connectors | TS contract + FakeNativeClient implemented; Tauri invoke will fail until Rust command registered |
| `get_pending_decisions` / `respond_decision` | Decision broker | UI + FakeNativeClient ready; no Tauri handlers yet |
| Production apply in non-preview builds | Connectors | UI sends `planId` only; backend apply/remove enabled per lane 1 handoff |
| Decision stream wiring | Decision broker | Pending decisions polled on dashboard mount; should move to stream frames when broker lands |

## UX notes (Fable consensus)

- No manual JSON/env editing in copy
- Overlay remains entry-only (`assertNoApproveDenyControls` preserved)
- Partial apply shows honest per-file outcomes
- Codex `actionNeeded` uses guided `/hooks` external trust copy only
