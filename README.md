# Overview
Sneklsp is a neovim focused LSP for Python development


## Setup
This project is experimental and official package support is not available

1. Build the project
```bash
cargo build --release
```

2. Enable the parser in init.lua or after/plugin/lsp.lua
```lua
vim.lsp.enable('sneklsp')
```

3. Create a ~/.config/nvim/lsp/sneklsp.lua
```lua
---@type vim.lsp.Config
return {
    cmd = { '/absolute/path/to/target/release/sneklsp', 'lsp' },
    filetypes = { 'python' },
    root_markers = {
        'pyproject.toml',
        'setup.py',
        'setup.cfg',
        'requirements.txt',
        '.git',
    },
}
```


## Running benchmarks
```bash
cargo bench --workspace            # runs all benchmarks
cargo bench -p sneklsp_parser      # runs parser benchmarks
cargo bench -p sneklsp_tokenizer   # runs tokenizer benchmarks

```
