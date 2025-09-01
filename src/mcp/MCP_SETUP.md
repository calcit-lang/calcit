## Quick Start

To add the Calcit MCP server to your Gemini CLI, you need to edit your `settings.json` file.

1.  **Locate your `settings.json` file.** On macOS and Linux, this is typically found at `~/.gemini/settings.json`.

2.  **Add the following configuration** to the `mcpServers` object in your `settings.json` file:

    ```json
    "calcit": {
      "httpUrl": "http://localhost:7200/mcp/",
      "trust": true
    }
    ```

    If the `mcpServers` object doesn't exist, you'll need to add it. Here is an example of a complete `settings.json` file with the Calcit MCP server configured:

    ```json
    {
      "selectedAuthType": "vertex-ai",
      "theme": "Default",
      "preferredEditor": "vscode",
      "mcpServers": {
        "calcit": {
          "httpUrl": "http://localhost:7200/mcp/",
          "trust": true
        }
      }
    }
    ```

## Start the server

To start the Calcit MCP server, run the following command:

```bash
cargo run --bin cr-mcp -- --compact-file calcit/compact.cirru
```

This will start the server on port 7200.

## Available Tools

Once the server is running and configured in your Gemini CLI, you will have access to the following tools:

*   `list_definitions`
*   `read_namespace`
*   `add_definition`
*   `reload_file`
*   `list_modules`
*   `parse_cirru_to_edn`
*   `format_cirru_from_edn`
*   `read_file`
*   `write_file`
*   `list_files`
*   `get_project_info`
*   `git_commit`
*   `git_diff`
*   `git_blame`
*   `search_code`
*   `run_shell`