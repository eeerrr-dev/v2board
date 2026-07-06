# Reference Projects

This directory is for read-only upstream references that help audit the rewrite.

## `wyx2685-v2board`

`references/wyx2685-v2board` is a git submodule pointing at
`https://github.com/wyx2685/v2board`, currently pinned to commit
`7e77de9f4873b317157490529f7be7d6f8a62421`.

Use it as a historical oracle for backend behavior and legacy project shape.
Do not copy, import, serve, or deploy packaged legacy frontend bundles from this
reference into the main source tree. If old assets are needed for a comparison,
restore them only into temporary Docker paths or Docker volumes.
