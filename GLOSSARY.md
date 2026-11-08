# Ultra Clipboard

Ultra Clipboard preserves clipboard history and captures screenshots on macOS
and Windows.

## Language

**Capture session**:
One screenshot capture attempt, from its start through selection, cancellation,
or handoff to an editor. A late result from a previous attempt does not belong
to the current capture session.

**Editor handoff**:
The transition from a frozen selection overlay to the editor displaying the
selected image. The overlay remains until the editor is ready or the handoff
ends unsuccessfully.

**Clipboard resource**:
An image, thumbnail, or app/file icon managed by Ultra Clipboard for its history
or current app list. A user's original file copied as a file path is not a
clipboard resource.

**History overwrite**:
Replacing the current history and its resources with a backup's contents.
Unlike history merge, it does not retain the current history alongside imported
items.

**Storage relocation**:
Moving the current Ultra Clipboard data to a different storage location while
preserving the history and settings.
