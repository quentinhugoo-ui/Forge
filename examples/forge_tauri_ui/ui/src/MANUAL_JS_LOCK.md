# Manual JavaScript Lock

Forge has quit hand-written JavaScript sources. This file is the guardrail that keeps it that way.

Rules:

- no new hand-written `.js` file outside `ui/dist/**/*.js`,
- transitional app shell code is authored in `ui/src/shell/surface.ts`,
- section surfaces are authored beside their section in `ui/src/sections/**/surface.ts`,
- new behavior must be authored in `ui/src/**/*.ts`,
- all browser JavaScript must be generated into `ui/dist/**/*.js` by `scripts/build-ui-runtime.mjs`,
- `npm.cmd run audit:js-debt` must keep `manualJsFiles: []`.

Remaining manual JavaScript debt:

None.

Transitional TypeScript surfaces still being drained:

- `ui/src/shell/surface.ts`
- `ui/src/sections/trading/surface.ts`
- `ui/src/sections/banger/surface.ts`

Generated JavaScript is allowed only under:

- `ui/dist/**/*.js`

Vendor artifacts are separate and must stay hash-pinned under `ui/assets/vendor/**`.
