local M = {}

M.defaults = {
    -- path to binary
    cmd = { "sneklsp", "lsp" },
    filetypes = { "python" },
    root_markers = {
        "pyproject.toml",
        "setup.py",
        "setup.cfg",
        "requirements.txt",
        ".git",
    },
    -- enable debug logging
    debug = false,
}

---@param opts table|nil
function M.setup(opts)
    opts = vim.tbl_deep_extend("force", M.defaults, opts or {})

    vim.api.nvim_create_autocmd("FileType", {
        pattern = opts.filetypes,
        callback = function(args)
            M.start(args.buf, opts)
        end,
        desc = "start sneklsp for Python files",
    })
end

---@param bufnr number
---@param opts table
function M.start(bufnr, opts)
    -- find root directory
    local root_dir = vim.fs.root(bufnr, opts.root_markers)

    if not root_dir then
        root_dir = vim.fn.getcwd()
    end

    local config = {
        name = "sneklsp",
        cmd = opts.cmd,
        root_dir = root_dir,
        capabilities = vim.lsp.protocol.make_client_capabilities(),
    }

    vim.lsp.start(config, {
        bufnr = bufnr,
        reuse_client = function(client, conf)
            -- reuse client if same name and root directory
            return client.name == conf.name and client.config.root_dir == conf.root_dir
        end,
    })
end

--- check if sneklsp is available
---@return boolean
function M.is_available()
    return vim.fn.executable("sneklsp") == 1
end

--- get sneklsp version
---@return string|nil
function M.version()
    local result = vim.system({ "sneklsp", "--version" }, { text = true }):wait()
    if result.code == 0 then
        return vim.trim(result.stdout)
    end
    return nil
end

--- get active sneklsp clients for a buffer
---@param bufnr number|nil Buffer number (defaults to current)
---@return table[] List of active sneklsp clients
function M.get_clients(bufnr)
    bufnr = bufnr or 0
    return vim.lsp.get_clients({ bufnr = bufnr, name = "sneklsp" })
end

--- stop sneklsp for a buffer
---@param bufnr number|nil Buffer number (defaults to current)
function M.stop(bufnr)
    for _, client in ipairs(M.get_clients(bufnr)) do
        client:stop()
    end
end

--- restart sneklsp for a buffer
---@param bufnr number|nil Buffer number (defaults to current)
function M.restart(bufnr)
    bufnr = bufnr or vim.api.nvim_get_current_buf()
    M.stop(bufnr)
    -- small delay to ensure clean shutdown
    vim.defer_fn(function()
        M.start(bufnr, M.defaults)
    end, 100)
end

return M
