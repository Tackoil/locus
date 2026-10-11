# Locus for Zed

This development extension recognizes `.locus` files and connects Zed to the
native `locus-lsp` executable over stdin/stdout. It supports the server's
diagnostics, completion, hover, go-to-definition, document formatting, and
document symbols. The bundled [Tree-sitter grammar](../tree-sitter-locus/README.md)
adds syntax highlighting, bracket matching, a declaration outline, and single-line
metadata. The grammar supports `locus 0 beta 6` constructors, functions, groups,
and named calls, including primed
identifiers, style properties, and TeX inside strings.

Beta 6 language server support includes function parameters and local bindings,
group and instance references, and completion, hover, and go-to-definition for
qualified members such as `checkout.path` and nested instance members.
Diagnostics use the CLI's text measurement engines and bounded layout feedback,
including automatic labels, measured shapes, and routed paths.
Validation runs in the background so completion and navigation remain available
while large diagrams solve. Diagnostics from superseded document versions are
discarded.

## Install locally

1. Install Rust through [rustup](https://rustup.rs/), Node.js/npm, Git, Python 3.9+
   and a current version of Zed.
2. From the Locus repository root, install the server:

   ```sh
   cargo install --path crates/locus-lsp --locked --force
   ```

   Ensure Cargo's binary directory (usually `~/.cargo/bin`) is on the PATH seen
   by Zed. Alternatively, configure an explicit binary path as shown below.
3. Prepare the extension from the repository root:

   ```sh
   npm ci --prefix ecosystem/tree-sitter-locus
   npm run generate --prefix ecosystem/tree-sitter-locus
   python3 ecosystem/zed/prepare.py
   ```

   The preparation command prints the absolute installation directory.
4. In Zed's command palette, run `zed: install dev extension` and select
   **`ecosystem/zed/target/dev-extension`**. Zed builds the extension and grammar,
   installing the `wasm32-wasip2` Rust target and WASI SDK if necessary.
   If you installed the earlier LSP-only extension, install this new directory
   to replace it.
5. Open a `.locus` file. The language selector should show **Locus**, with colored
   keywords, constructors, numbers, strings, and comments. Try hovering
   a declared name, jumping to its definition, or introducing an unknown name
   to see a diagnostic.

The extension is a standalone Cargo workspace because Zed compiles the adapter
to WebAssembly while the language server runs as a native executable.

Zed requires a Git URL and revision for every grammar. `prepare.py` makes a
local snapshot under `target/grammar-source` and writes its file URL and commit
into the staged extension manifest. It copies only grammar source files into
this generated repository; it does not commit or modify the main repository's
Git state. This supports uncommitted grammar development and checkout paths
containing spaces without hardcoding a developer's path in source control.
The source `extension.toml` is a base manifest; always install the prepared
directory so the grammar registration is included. Keep the generated snapshot
directory in place while using the dev extension.

## Use a local build

Build the server from the repository root:

```sh
cargo build --release -p locus-lsp --locked
```

Merge this into Zed's user settings or the project's `.zed/settings.json`,
replacing the example path with the absolute path to your build:

```json
{
  "lsp": {
    "locus-lsp": {
      "binary": {
        "path": "/absolute/path/to/locus/target/release/locus-lsp",
        "arguments": []
      }
    }
  },
  "languages": {
    "Locus": {
      "language_servers": ["locus-lsp"],
      "formatter": "language_server"
    }
  }
}
```

An explicit `binary.path` takes precedence over PATH discovery. Optional
`binary.arguments` and `binary.env` are forwarded to the process. The server
needs no arguments (including no `--stdio` flag). The extension does not
download or build the server automatically.

When updating an installed server, use `--force` as above even if the package
version has not changed during beta development. After rebuilding the server,
run `editor: restart language server`.

After editing the grammar, run `npm run generate --prefix ecosystem/tree-sitter-locus`.
After changing either the grammar, the extension, or its queries, rerun
`python3 ecosystem/zed/prepare.py`, then rebuild Locus from Zed's Extensions page.
Rerun preparation and reinstall the generated directory if you move the checkout.
If startup fails,
check `zed: open log` and verify the executable path or Zed's PATH.

## Development checks

From the repository root:

```sh
npm ci --prefix ecosystem/tree-sitter-locus
npm run generate --prefix ecosystem/tree-sitter-locus
npm test --prefix ecosystem/tree-sitter-locus
python3 ecosystem/zed/prepare.py
rustup target add wasm32-wasip2
cargo fmt --manifest-path ecosystem/zed/Cargo.toml -- --check
cargo clippy --manifest-path ecosystem/zed/Cargo.toml --locked --target wasm32-wasip2 -- -D warnings
cargo build --manifest-path ecosystem/zed/Cargo.toml --locked --release --target wasm32-wasip2
cargo test -p locus-lsp --locked
```

See Zed's [extension development guide](https://zed.dev/docs/extensions/developing-extensions)
and [language server configuration](https://zed.dev/docs/configuring-languages).
