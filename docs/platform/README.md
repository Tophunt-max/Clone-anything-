# Platform documentation

Hybrid AI software engineering platform (Android + Web) built on the original Craft Anything
engine. Phase 0 output:

| Document | Contents |
|---|---|
| [00-audit-report.md](00-audit-report.md) | What upstream contains, how `Engine::call` works, test results, blockers |
| [01-architecture.md](01-architecture.md) | Components, worker model, auth, authorization, agents, isolation |
| [02-integration-map.md](02-integration-map.md) | Folder layout and exactly how new code uses upstream crates |
| [03-tool-inventory.md](03-tool-inventory.md) | All 326 tools with effects, requirements and proposed policy (generated) |
| [tool-inventory.json](tool-inventory.json) | Same, with full input schemas (generated) |
| [04-database-design.md](04-database-design.md) | PostgreSQL schema, tenancy, retention, backup |
| [05-api-spec.md](05-api-spec.md) | REST v1 conventions, endpoints, real-time events |
| [06-ui-plan.md](06-ui-plan.md) | Design tokens and the 38 screens |
| [07-threat-model.md](07-threat-model.md) | Threats, controls and the phase that tests each |
| [08-roadmap.md](08-roadmap.md) | Phases, current status, blockers |
| [09-development.md](09-development.md) | Requirements and commands for every part |

Regenerate the inventory after an upstream sync:

```
cargo run -q -p forge-worker -- catalog > tools.json
python3 scripts/tool-inventory/render.py tools.json
```
