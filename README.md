# xdr-ls

Language server for [RFC4506 XDR](https://datatracker.ietf.org/doc/html/rfc4506)
files. Supports XDR files with
[xdrpp](https://xdrpp.github.io/xdrpp/index.html) syntax extensions (lines
starting with `%` and `namespace` blocks).

## Current features

* [goto definition](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_definition)
* [find references](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_references)

## Known limitations

* File updates are not supported.
* Uses LSP `root_uri`.
* Assumes all `.x` files within `root_uri` are XDR files.
* Assumes all `.x` files are ASCII.

## Building

* clone the repo
* run `cargo build`

## Using in Neovim

Once [built](#Building), take note of the executable location. Then, add an
appropriate configuration—the following sample configuration assumes the
executable is somewhere in `$PATH` and that the XDR files you're looking at
live in a git repo.

```lua
vim.lsp.config("xdr-ls", {
    cmd = {"xdr-ls"},
    filetypes = {"rpcgen"},
    root_markers = {".git/"}
})
vim.lsp.enable("xdr-ls")
```
