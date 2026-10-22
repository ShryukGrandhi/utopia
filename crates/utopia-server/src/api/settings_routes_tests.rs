//! 对话模型的测试要按任务真用的请求形状问（#1354）：只问一句纯 chat，拒绝工具的端点照样「已连通」

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};

/// 纯 chat、流式都答，见到 `tools` 就 400：#1353 里那种端点
async fn refuses_tools(Json(body): Json<Value>) -> Response {
    if body.get("tools").is_some() {
        let err = json!({ "error": { "message": "tools are not supported by this model" } });
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }
    if body["stream"] == true {
        let sse = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"OK\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n",
        );
        return ([("content-type", "text/event-stream")], sse).into_response();
    }
    Json(json!({ "choices": [{ "message": { "content": "OK" } }] })).into_response()
}

#[tokio::test]
async fn a_model_that_refuses_tools_fails_the_test() -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}", listener.local_addr()?);
    let router = Router::new().route("/chat/completions", post(refuses_tools));
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = utopia_llm::LlmClient::new(&endpoint, None, "scripted");

    let result = super::probe_chat(&client).await;
    server.abort();
    assert_eq!(result["ok"], false, "{result}");
    let error = result["error"].as_str().unwrap_or_default();
    assert!(error.contains("chat ok"), "{error}");
    assert!(error.contains("streaming ok"), "{error}");
    assert!(
        error.contains("tools refused") && error.contains("tools are not supported"),
        "{error}"
    );
    Ok(())
}
