# Claude Code instructions

@AGENTS.md
@docs/CONVENTIONS.md

The two files above are the project guide and the conventions of this starter pack. Follow them.

- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings` and `cargo test --workspace` before saying
  a change is done, one command at a time (the tests share a database).
- Templates and `.ftl` files are compiled in: restart the server after changing them.
- Do not add assistant or tool attribution lines to commit messages or pull requests.
- Keep `docs/CONVENTIONS.md`, the module README and the matching tutorial in step with the code.
