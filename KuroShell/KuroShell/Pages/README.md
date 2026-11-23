# Shell pages

Each page lives in `Pages/<Name>/` as `<Name>Page.xaml` plus code-behind. Register
the page id in `PageRegistry.cs` and add a descriptor in `KuroShell.Core` if the
id is new.

## Add a page

1. Run `cargo xtask new-shell-page <Name>` when the scaffolder exists, or copy
   `Settings/SettingsPage.xaml` and rename the class and folder.
2. Add `public const string <Name> = "<Name>";` to `KuroShell.Core/PageIds.cs` when
   the id is shared with tests or navigation.
3. Register in `PageCatalog.CreateDefault()` (or call `PageRegistry.Register` at
   startup) with an `IPageDescriptor` and a factory that returns the `UserControl`.
4. Navigate from `MainWindow` (or a host `Frame`) with
   `PageRegistry.ResolveView(pageId)` once the host is wired.
5. Put non-UI logic in `KuroShell.Core` and cover registry rules with
   `dotnet test KuroShell/KuroShell.Tests`.

## Built-in ids (phase 17 slice)

| Id | Folder | Notes |
|----|--------|-------|
| Settings | `Settings/` | Pause, HUD text, restart actions |
| Health | `Health/` | API, pause, router, voice status |
| Phrases | `Phrases/` | Searchable phrases with Try (safe only) |

Unknown ids throw `PageNotFoundException` with the list of known page names.