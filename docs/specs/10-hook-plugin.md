# 10 Hook plugin (`HOOK`)

> **Status: not written yet.** This spec is a placeholder. Its features and acceptance criteria
> still need to be written by the owner or a contributor. Until then, M7 has no acceptance criteria.

## 1. Purpose and scope

The hook plugin captures text from a running game and sends it to the app. It is a separate,
optional plugin, not part of the core app (D6). It lives in `plugins/hook/` in this repository until
it stabilizes (D19), and ships as a 32-bit and a 64-bit package (D1).

The core app does not depend on it: text from external tools that copy to the clipboard, or that
post to the local API, works without it.

## 2. Goals

To be written. Starting points:

- Captured text reaches the app through the local input API with a context id (see the API spec).
- The plugin talks to the app only through the plugin protocol and the local API (see the PLG spec),
  so any plugin bitness works with any app bitness.
- If the plugin or its helper stops or crashes, the app keeps running.

## 3. Features

To be written. Relevant background: `../product-direction.md` (D6, D19, O13),
`../analysis/features.md` §3 (legacy text capture features and their Rewrite dispositions),
`../analysis/migration-plan.md` Phase 7.

## 4. Out of scope

- Hooking or injection code in the core app (D6).
- Legacy `__stdcall` plugin DLLs (D15).

## 5. Open questions

- O13: Japanese-locale launch: reimplement, or point to external tools? (Recommendation: external tools.)

## 6. Traceability

| Feature | Priority | Milestone | Decisions |
|---|---|---|---|
| *(none yet)* | | M7 | D1, D6, D19 |
