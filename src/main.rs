use axum::{ http::StatusCode, response::IntoResponse, routing::post, Json, Router };
use llama_cpp_2::{ llama_backend::LlamaBackend, model::{ LlamaModel, Special } };
use serde::{ Deserialize, Serialize };
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;

#[derive(Deserialize)]
struct PromptRequest {
    prompt: String,
}

#[derive(Serialize)]
struct PromptResponse {
    reply: String,
}

struct AppState {
    model: Arc<Mutex<LlamaModel>>,
    backend: LlamaBackend,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("afo_ai=debug,info").init();

    // Initialize llama backend
    let backend = LlamaBackend::init()?;

    // Load model (place your GGUF file in the project root or download at runtime)
    let model_path = std::env
        ::var("MODEL_PATH")
        .unwrap_or_else(|_| "./qwen2.5-0.5b-instruct-q4_k_m.gguf".to_string());

    tracing::info!("Loading model from {}", model_path);

    let model_params = llama_cpp_2::model::LlamaModelParams::default();
    let model = LlamaModel::load_from_file(&backend, &model_path, &model_params)?;

    let state = Arc::new(AppState {
        model: Arc::new(Mutex::new(model)),
        backend,
    });

    let app = Router::new()
        .route("/generate", post(generate))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{}", port);
    tracing::info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn generate(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    Json(req): Json<PromptRequest>
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let model = state.model.clone();
    let prompt = format!(
        "<|im_start|>system\nYou are a helpful interview assistant. Keep responses concise.<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n",
        req.prompt
    );

    let response_text = tokio::task
        ::spawn_blocking(move || {
            let mut model = model.blocking_lock();
            let mut ctx = model
                .new_context(&state.backend, llama_cpp_2::context::LlamaContextParams::default())
                .map_err(|e| format!("Context error: {}", e))?;

            let tokens = ctx
                .model()
                .tokenize(&prompt, false, Special::Tokenize)
                .map_err(|e| format!("Tokenize error: {}", e))?;

            let mut batch = llama_cpp_2::llama_batch::LlamaBatch::new(512, 1);
            for (i, token) in tokens.iter().enumerate() {
                batch.add(*token, i as i32, &[0], i == tokens.len() - 1)?;
            }

            ctx.decode(&mut batch)?;

            // Generate response (simplified – in production, stream tokens)
            let mut response = String::new();
            let mut sampler = ctx.sampler(
                llama_cpp_2::sampling::LlamaSamplerChainParams::default()
            )?;
            for _ in 0..256 {
                let token = sampler.sample(&ctx)?;
                if ctx.model().is_eog(token) {
                    break;
                }
                let piece = ctx.model().token_to_piece(token, Special::Tokenize)?;
                response.push_str(&piece);
                ctx.accept(token)?;
            }
            Ok::<_, String>(response)
        }).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(PromptResponse { reply: response_text }))
}
