use axum::{ extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router };
use serde::{ Deserialize, Serialize };
use std::sync::Arc;
use tower_http::cors::CorsLayer;

#[derive(Clone)]
struct AppState {
    ollama_url: String,
    model: String,
}

#[derive(Deserialize)]
struct PromptRequest {
    prompt: String,
}

#[derive(Serialize)]
struct PromptResponse {
    reply: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let state = Arc::new(AppState {
        ollama_url: std::env
            ::var("OLLAMA_URL")
            .unwrap_or_else(|_| "http://ollama:11434".to_string()),
        model: std::env::var("MODEL").unwrap_or_else(|_| "qwen2.5".to_string()),
    });

    let app = Router::new()
        .route("/generate", post(generate))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{}", port);

    tracing::info!("Server running on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn generate(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PromptRequest>
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let client = reqwest::Client::new();

    let res = client
        .post(format!("{}/api/generate", state.ollama_url))
        .json(
            &serde_json::json!({
            "model": state.model,
            "prompt": format!(
                "You are a helpful interview assistant. Keep answers concise.\nUser: {}\nAssistant:",
                req.prompt
            ),
            "stream": false
        })
        )
        .send().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let json: serde_json::Value = res
        .json().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let reply = json["response"].as_str().unwrap_or("No response").to_string();

    Ok(Json(PromptResponse { reply }))
}
