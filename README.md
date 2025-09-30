# xdr-ls

Language server for [RFC4506 XDR](https://datatracker.ietf.org/doc/html/rfc4506)
files. Supports XDR files with
[xdrpp](https://xdrpp.github.io/xdrpp/index.html) syntax extensions (lines
starting with `%` and `namespace` blocks).

## Current features

* [goto definition](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_definition)
    * Note, the server will also attempt to respond to goto definition requests
      in header files where replacing `.h` with `.x` results in one of the
      known `.x` files.
* [find references](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_references)

## Known limitations

* File updates are not supported.
* Uses LSP `root_uri`.
* Assumes all `.x` files within `root_uri` are XDR files.
* Assumes all `.x` files are ASCII.
* VS Code extension assumes generated header files live in an `xdr` folder.

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

If you want to also be able to use the goto definition/tagfunc for generated
header files, you can adjust the config as follows.

```lua
vim.lsp.config("xdr-ls", {
    cmd = {"xdr-ls"},
    filetypes = {"rpcgen", "cpp"},
    root_markers = {".git/"}
})
```

If you also have `clangd` enabled, `vim.lsp.tagfunc()` might end up with the
`clangd` result first. When trying to use `CTRL-]`, you can either then use
`]t` (or `:tnext`) or you can set it up so that `x` files get priority for the
tagfunc. (Useful things to look at for `:help` include `lsp-defaults` and
`tagfunc`). For example,

```lua
function xdr_tagfunc(pattern, flags)
    local orig = vim.lsp.tagfunc(pattern, flags)
    local res = {}
    for _, loc in ipairs(orig) do
        -- TODO
        if loc.filename:sub(-2) == ".x" then
            table.insert(res, loc)
        end
    end
    for _, loc in ipairs(orig) do
        -- TODO
        if loc.filename:sub(-2) ~= ".x" then
            table.insert(res, loc)
        end
    end
    return res
end
vim.lsp.config("xdr-ls", {
    cmd = {"xdr-ls"},
    filetypes = {"rpcgen", "cpp"},
    root_markers = {".git/"},
    on_attach = function(client, bufnr)
        if vim.bo[bufnr].tagfunc == "v:lua.vim.lsp.tagfunc" then
            vim.bo[bufnr].tagfunc = "v:lua.xdr_tagfunc"
        end
    end
})
vim.lsp.enable("xdr-ls")
```

## Using in VS Code

Once [built](#Building), make sure you have `xdr-ls` somewhere in your `$PATH`.
Then, either build [xdr-ls-code](https://github.com/drebelsky/xdr-ls-code) from
source and add the `.vsix` extension or download one of the pre-built `.vsix`s
from the [releases page](https://github.com/drebelsky/xdr-ls-code/releases). In
VS Code, open the extensions menu, and then click the three dots in the upper
right of the pane and click "Install from VSIX...".
