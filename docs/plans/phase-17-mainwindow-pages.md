# Phase 17: MainWindow pages and PageRegistry

- Size: M
- Depends on: phase 16
- Behavior change: none for existing pages. Navigation moves behind PageRegistry
  so new pages are folder + registry line.
- ADRs: 0003 (shell analogue)
- Protected paths: none typically.

## Goal

Extract shell pages into `Pages/<Name>/` with a PageRegistry; MainWindow becomes
frame chrome only. Scaffold target for `xtask new-shell-page` (already in phase 11
templates) becomes real.

## Scope

In: Page folders, registry, settings page host bound to schema-driven model stub,
README for Pages.

Out: New product pages (health, what-can-I-say in 26D); theme file extraction can
start here as `themes/kuro` resources if small.

## Context card

Read:
- `docs/plans/01-architecture.md` KuroShell tree
- `MainWindow.xaml` / code-behind
- Existing pages/views

Create:
- `KuroShell/KuroShell/Pages/` with one folder per page
- `PageRegistry.cs`
- `Pages/README.md`

Edit: MainWindow to host Frame + registry; xtask template paths if needed.

Do not touch: Capsule placement; engine protocol beyond existing bindings.

Commands:
```powershell
dotnet test KuroShell/KuroShell.Tests
cargo xtask gate fast
```

## Tests first

- Registry resolves known pages; unknown name fails clearly.
- Navigation smoke test.

## Steps

1. Move each page into its folder with XAML + code-behind.
2. Register in PageRegistry.
3. Slim MainWindow.
4. Document add-a-page guide.

## Acceptance

- Agent-ability: add shell page without editing MainWindow.xaml except registry.
- Tests green; UI smoke manual checklist in PR.

## Risks

- XAML resource paths break: update namespace and pack URIs carefully.
