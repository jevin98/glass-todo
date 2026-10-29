# 数据 MCP 操作 / Data MCP operations

完整的客户端配置、11 项工具参数、自定义状态、版本冲突及直连客户端说明已移到 [MCP 接入与 API](mcp.md)。

待办正文、状态、顺序及状态定义通过共享数据 MCP 读写。修改前读取版本，自定义状态使用 `list_statuses` → `update_todo(status_id, expected_version)`，成功后再次读取确认。旧会话缺工具时检查服务端最新目录或使用项目中的 HTTP MCP 客户端。

程序不会为此修改客户端的全局 agent 规则。配置文件中的真实密钥、数据库和个人待办不属于公开仓库内容。
