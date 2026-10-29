use crate::statuses::StatusColor;
use crate::{config::Config, core::Core, store::Status};
use rmcp::{
    ServerHandler,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock as Content, ServerCapabilities, ServerConfig},
    tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use schemars::JsonSchema;
use serde::Deserialize;
use std::{net::TcpListener, sync::Arc, thread::JoinHandle};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
struct Tools {
    core: Arc<Core>,
}
#[derive(Deserialize, JsonSchema)]
struct ListArgs {
    #[serde(default)]
    status: Option<Status>,
    #[serde(default)]
    status_id: Option<String>,
}
#[derive(Deserialize, JsonSchema)]
struct CreateArgs {
    /// Plain text; supports newlines, up to 16384 characters.
    content: String,
}
#[derive(Deserialize, JsonSchema)]
struct UpdateArgs {
    id: i64,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    status: Option<Status>,
    /// Exact status ID from list_statuses; use this for custom statuses. Do not also set status.
    #[serde(default)]
    status_id: Option<String>,
    /// Version from list_todos/get_todo. Supply it to avoid overwriting another client.
    #[serde(default)]
    expected_version: Option<i64>,
}
#[derive(Deserialize, JsonSchema)]
struct IdArgs {
    id: i64,
}
#[derive(Deserialize, JsonSchema)]
struct DeleteArgs {
    id: i64,
    #[serde(default)]
    expected_version: Option<i64>,
}
#[derive(Deserialize, JsonSchema)]
struct ReorderArgs {
    /// All current pending/in_progress IDs in the desired order, without duplicates.
    ids: Vec<i64>,
}
#[derive(Deserialize, JsonSchema)]
struct CreateStatusArgs {
    name: String,
    kind: Status,
    color: StatusColor,
}
#[derive(Deserialize, JsonSchema)]
struct UpdateStatusArgs {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    kind: Option<Status>,
    #[serde(default)]
    color: Option<StatusColor>,
    expected_version: i64,
}
#[derive(Deserialize, JsonSchema)]
struct DeleteStatusArgs {
    id: String,
    /// Existing status ID to receive all affected todos, including deleted items available for undo.
    replacement_id: String,
    expected_version: i64,
}

fn output<T: serde::Serialize>(result: anyhow::Result<T>) -> CallToolResult {
    match result {
        Ok(value) => CallToolResult::success(vec![Content::text(
            serde_json::to_string(&value).unwrap_or_default(),
        )]),
        Err(e) => CallToolResult::error(vec![Content::text(e.to_string())]),
    }
}

#[tool_router]
impl Tools {
    #[tool(
        description = "List saved todos in manual order. pending and in_progress are active; done items are retained. Read versions before changing existing items."
    )]
    fn list_todos(&self, Parameters(args): Parameters<ListArgs>) -> CallToolResult {
        output(self.core.list().map(|items| {
            items
                .into_iter()
                .filter(|t| {
                    args.status.is_none_or(|s| s == t.status)
                        && args
                            .status_id
                            .as_ref()
                            .is_none_or(|key| key == &t.status_id)
                })
                .collect::<Vec<_>>()
        }))
    }
    #[tool(description = "Read one todo by its stable numeric ID, including its current version.")]
    fn get_todo(&self, Parameters(args): Parameters<IdArgs>) -> CallToolResult {
        output(self.core.get(args.id))
    }
    #[tool(
        description = "Create a pending todo with plain text, including multiple lines. Persists immediately."
    )]
    fn create_todo(&self, Parameters(args): Parameters<CreateArgs>) -> CallToolResult {
        output(self.core.create(&args.content))
    }
    #[tool(
        description = "Change content and/or status. status sets a built-in classification (pending, in_progress, done). To select a custom status, pass status_id from list_statuses instead of status. Pass expected_version from the latest read."
    )]
    fn update_todo(&self, Parameters(args): Parameters<UpdateArgs>) -> CallToolResult {
        if args.content.is_none() && args.status.is_none() && args.status_id.is_none() {
            return output::<()>(Err(anyhow::anyhow!(
                "至少指定 content、status 或 status_id"
            )));
        }
        output(self.core.update_with_status(
            args.id,
            args.content.as_deref(),
            args.status,
            args.status_id.as_deref(),
            args.expected_version,
        ))
    }
    #[tool(
        description = "Soft-delete a todo by ID. The application can undo deletion. Supply expected_version for conflict protection."
    )]
    fn delete_todo(&self, Parameters(args): Parameters<DeleteArgs>) -> CallToolResult {
        output(self.core.delete(args.id, args.expected_version))
    }
    #[tool(
        description = "Restore the most recently deleted todo. Works after an application restart as well."
    )]
    fn undo_delete(&self) -> CallToolResult {
        output(self.core.undo())
    }
    #[tool(
        description = "Set manual order of active todos. Include every pending/in_progress ID exactly once; completed records keep their own order."
    )]
    fn reorder_todos(&self, Parameters(args): Parameters<ReorderArgs>) -> CallToolResult {
        output(self.core.reorder(&args.ids))
    }
    #[tool(
        description = "List built-in and custom status definitions. Todo.status is its compatible classification; Todo.status_id is the selected status. Use status_id in update_todo for custom labels."
    )]
    fn list_statuses(&self) -> CallToolResult {
        output(self.core.statuses())
    }
    #[tool(
        description = "Create a reusable custom status (name 1-10 characters, palette color, and pending/in_progress/done classification). Persists locally and refreshes the note."
    )]
    fn create_status(&self, Parameters(args): Parameters<CreateStatusArgs>) -> CallToolResult {
        output(self.core.create_status(&args.name, args.kind, args.color))
    }
    #[tool(
        description = "Edit a custom status name/color/classification. Read list_statuses first and pass its version. Changing classification also increments affected todo versions."
    )]
    fn update_status(&self, Parameters(args): Parameters<UpdateStatusArgs>) -> CallToolResult {
        output(self.core.update_status(
            &args.id,
            args.name.as_deref(),
            args.kind,
            args.color,
            Some(args.expected_version),
        ))
    }
    #[tool(
        description = "Delete a custom status and atomically reassign all affected todos to replacement_id. Todo contents are preserved. Built-in statuses cannot be deleted. Read list_statuses first."
    )]
    fn delete_status(&self, Parameters(args): Parameters<DeleteStatusArgs>) -> CallToolResult {
        output(
            self.core
                .delete_status(&args.id, &args.replacement_id, Some(args.expected_version)),
        )
    }
}

#[tool_handler]
impl ServerHandler for Tools {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build()).with_instructions("GlassTodo stores personal todos locally. Query existing items before edits and pass expected_version; on a version conflict read again. Mark work in_progress when starting and done only when finished. Do not create duplicate todos on retries. All writes are persisted before success and refresh the floating note. Deletion is recoverable via undo_delete. Read list_statuses to discover custom status IDs, then set update_todo.status_id; status remains the compatible pending/in_progress/done classification.")
    }
}

async fn authorize(
    axum::extract::State(token): axum::extract::State<Arc<String>>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::{http::StatusCode, response::IntoResponse};
    let expected = format!("Bearer {token}");
    let provided = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    // Compare every byte for equal-length tokens instead of early-return comparison.
    let valid = provided.len() == expected.len()
        && provided
            .bytes()
            .zip(expected.bytes())
            .fold(0u8, |v, (a, b)| v | (a ^ b))
            == 0;
    if !valid {
        return (StatusCode::UNAUTHORIZED, "Local MCP bearer token required").into_response();
    }
    next.run(req).await
}

pub fn spawn(
    core: Arc<Core>,
    socket: TcpListener,
    token: String,
    stop: CancellationToken,
    cfg: Config,
) -> anyhow::Result<JoinHandle<()>> {
    Ok(std::thread::Builder::new()
        .name("local-mcp".into())
        .stack_size(512 * 1024)
        .spawn(move || {
            let result = (|| -> anyhow::Result<()> {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                runtime.block_on(async move {
                    let config = StreamableHttpServerConfig::default()
                        .with_legacy_session_mode(true)
                        .with_json_response(true)
                        .with_max_request_body_bytes(256 * 1024)
                        .with_cancellation_token(stop.child_token())
                        .enforce_origin_validation();
                    let service = StreamableHttpService::new(
                        move || Ok(Tools { core: core.clone() }),
                        Arc::new(LocalSessionManager::default()),
                        config,
                    );
                    let routes = axum::Router::new().nest_service("/mcp", service).layer(
                        axum::middleware::from_fn_with_state(Arc::new(token), authorize),
                    );
                    let listener = tokio::net::TcpListener::from_std(socket)?;
                    axum::serve(listener, routes)
                        .with_graceful_shutdown(stop.cancelled_owned())
                        .await?;
                    Ok::<(), anyhow::Error>(())
                })?;
                Ok(())
            })();
            if let Err(e) = result {
                cfg.log(&format!("MCP 服务异常: {e:#}"));
            }
        })?)
}
