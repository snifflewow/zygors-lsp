-- Neovim LSP configuration for zygorguide-lsp
-- Add this to your Neovim config (init.lua or a plugin file)

-- Option 1: Auto-attach to Lua files in Guides/ directories
vim.api.nvim_create_autocmd('FileType', {
  pattern = 'lua',
  callback = function(args)
    local path = vim.api.nvim_buf_get_name(args.buf)
    if not path:match('Guides/') then
      return
    end

    -- Find workspace root (directory containing pfQuest-epoch)
    local root = vim.fs.dirname(
      vim.fs.find({ 'pfQuest-epoch', 'pfQuest' }, {
        upward = true,
        path = vim.fs.dirname(path),
      })[1] or ''
    )

    if root == '' then
      return
    end

    vim.lsp.start({
      name = 'zygorguide-lsp',
      cmd = { vim.fn.expand('~/Repos/zygor_test/zygorguide-lsp/target/release/zygorguide-lsp') },
      root_dir = root,
    })
  end,
})

-- Option 2: If using lspconfig, add a custom server config
-- (requires nvim-lspconfig plugin)
--[[
local lspconfig = require('lspconfig')
local configs = require('lspconfig.configs')

if not configs.zygorguide then
  configs.zygorguide = {
    default_config = {
      cmd = { vim.fn.expand('~/Repos/zygor_test/zygorguide-lsp/target/release/zygorguide-lsp') },
      filetypes = { 'lua' },
      root_dir = lspconfig.util.root_pattern('pfQuest-epoch', 'pfQuest'),
      settings = {},
    },
  }
end

lspconfig.zygorguide.setup({
  on_attach = function(client, bufnr)
    -- Only attach to guide files
    local path = vim.api.nvim_buf_get_name(bufnr)
    if not path:match('Guides/') then
      vim.lsp.buf_detach_client(bufnr, client.id)
    end
  end,
})
--]]
