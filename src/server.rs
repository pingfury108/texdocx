use axum::extract::DefaultBodyLimit;
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use txdx::{convert_to_docx, ConvertOptions};

#[derive(Debug, Deserialize)]
struct ConvertRequest {
    text: String,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

pub async fn serve(host: String, port: u16) -> anyhow::Result<()> {
    let addr: SocketAddr = format!("{host}:{port}").parse()?;
    let app = Router::new()
        .route("/health", get(health))
        .route("/convert", post(convert))
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024));

    eprintln!("API server listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

async fn convert(Json(request): Json<ConvertRequest>) -> Response {
    let options = ConvertOptions::default();

    let result =
        tokio::task::spawn_blocking(move || convert_to_docx(&request.text, &options)).await;

    match result {
        Ok(Ok(docx)) => {
            let mut headers = HeaderMap::new();
            headers.insert(
                CONTENT_TYPE,
                HeaderValue::from_static(
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                ),
            );
            if let Ok(value) = HeaderValue::from_str(r#"attachment; filename="output.docx""#) {
                headers.insert(CONTENT_DISPOSITION, value);
            }
            (headers, docx).into_response()
        }
        Ok(Err(err)) => error_response(StatusCode::BAD_REQUEST, err.to_string()),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

fn error_response(status: StatusCode, error: String) -> Response {
    (status, Json(ErrorResponse { error })).into_response()
}
