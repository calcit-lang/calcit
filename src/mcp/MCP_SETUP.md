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

- `list_namespaces` - 列出项目中的所有命名空间
- `list_namespace_definitions` - 列出命名空间中的所有函数和宏定义
- `get_package_name` - 获取当前 Calcit 项目的包名
- `read_namespace` - 读取命名空间的详细信息，包括导入规则和元数据
- `add_namespace` - 创建新的命名空间
- `delete_namespace` - 删除命名空间
- `update_namespace_imports` - 修改命名空间的导入规则
- `add_definition` - 创建新的函数或宏定义
- `delete_definition` - 删除函数或宏定义
- `overwrite_definition` - 完全覆盖现有的函数或宏定义
- `operate_definition_at` - 使用坐标精确更新函数定义的特定部分
- `operate_definition_at_with_leaf` - 使用坐标精确更新函数定义的特定部分
- `read_definition_at` - 读取函数定义中特定位置的内容
- `list_modules` - 列出项目中的所有模块
- `get_current_module` - 获取当前活动的模块
- `create_config_entry` - 创建新的模块
- `delete_config_entry` - 删除模块
- `calcit_parse_cirru_to_json` - 将 Cirru 语法解析为 JSON
- `format_json_to_cirru` - 将 JSON 格式化为 Cirru 语法
- `parse_cirru_edn_to_json` - 将 Cirru EDN 格式解析为 JSON
- `query_calcit_apis` - 查询 Calcit API 文档
- `query_calcit_reference` - 查询 Calcit 参考文档
- `list_api_docs` - 列出所有可用的 API 文档主题
- `list_guidebook_docs` - 列出所有可用的指南文档主题
- `read_configs` - 读取项目配置设置
- `update_configs` - 更新项目配置设置
- `list_dependency_docs` - 列出模块依赖的文档
- `read_dependency_definition_doc` - 读取依赖中特定定义的文档
- `read_dependency_module_doc` - 读取依赖中特定模块的文档
- `fetch_calcit_libraries` - 获取可用的 Calcit 库列表
- `start_calcit_runner` - 启动 Calcit 运行器后台服务，用于调试模式
- `grab_calcit_runner_logs` - 获取 Calcit 运行器的日志并清空队列
- `stop_calcit_runner` - 停止 Calcit 运行器服务并获取剩余日志
