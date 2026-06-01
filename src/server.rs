use axum::extract::DefaultBodyLimit;
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Response};
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
        .route("/", get(index))
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

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
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

const INDEX_HTML: &str = r#"<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>txdx</title>
  <style>
    :root {
      color-scheme: light;
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }
    body {
      margin: 0;
      background: #f6f7f9;
      color: #1f2328;
    }
    main {
      max-width: 1180px;
      margin: 0 auto;
      padding: 32px 20px;
    }
    header {
      display: flex;
      align-items: baseline;
      justify-content: space-between;
      gap: 16px;
      margin-bottom: 16px;
    }
    h1 {
      margin: 0;
      font-size: 24px;
      font-weight: 650;
    }
    .status {
      min-height: 22px;
      color: #57606a;
      font-size: 14px;
      text-align: right;
    }
    textarea {
      box-sizing: border-box;
      width: 100%;
      min-height: 460px;
      resize: vertical;
      padding: 16px;
      border: 1px solid #d0d7de;
      border-radius: 6px;
      background: #fff;
      color: #1f2328;
      font: 16px/1.65 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
      outline: none;
    }
    textarea:focus {
      border-color: #0969da;
      box-shadow: 0 0 0 3px rgba(9, 105, 218, 0.15);
    }
    .actions {
      display: flex;
      gap: 10px;
      justify-content: flex-end;
      margin-top: 14px;
    }
    button {
      min-width: 132px;
      height: 40px;
      border: 0;
      border-radius: 6px;
      background: #1f883d;
      color: #fff;
      font-size: 15px;
      font-weight: 600;
      cursor: pointer;
    }
    button:disabled {
      cursor: wait;
      opacity: 0.65;
    }
    .secondary {
      background: #0969da;
    }
    .preview {
      min-height: 360px;
      margin-top: 24px;
      padding: 18px;
      border: 1px solid #d0d7de;
      border-radius: 6px;
      background: #eaeef2;
      overflow: auto;
    }
    .preview:empty::before {
      content: "DOCX 预览会显示在这里";
      color: #6e7781;
      font-size: 14px;
    }
  </style>
  <script src="https://cdn.jsdelivr.net/npm/jszip@3.10.1/dist/jszip.min.js"></script>
  <script src="https://cdn.jsdelivr.net/npm/docx-preview@0.3.7/dist/docx-preview.min.js"></script>
</head>
<body>
  <main>
    <header>
      <h1>txdx</h1>
      <div id="status" class="status"></div>
    </header>
    <textarea id="text">在$\triangle ABC$中，内角$A,B,C$对边分别为$BC=a,CA=b,AB=c$，
且满足$\cos B=\frac{1}{2}$，$a^2=2b\sqrt{3}$。

（1）求$\triangle ABC$的面积$S$；
（2）若$b+c=6$，求$a$的值。

$$S=\frac{1}{2}ac\sin B$$

$$\therefore a=2\sqrt{3}$$</textarea>
    <div class="actions">
      <button id="previewBtn" class="secondary">预览 DOCX</button>
      <button id="downloadBtn">下载 DOCX</button>
    </div>
    <div id="preview" class="preview"></div>
  </main>
  <script>
    const previewBtn = document.getElementById('previewBtn');
    const downloadBtn = document.getElementById('downloadBtn');
    const textarea = document.getElementById('text');
    const statusEl = document.getElementById('status');
    const previewEl = document.getElementById('preview');
    let latestBlob = null;
    let latestText = '';

    async function generateDocx() {
      const text = textarea.value;
      if (!text.trim()) {
        throw new Error('请输入文本');
      }

      if (latestBlob && latestText === text) {
        return latestBlob;
      }

      const response = await fetch('/convert', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ text })
      });

      if (!response.ok) {
        let message = '生成失败';
        try {
          const data = await response.json();
          if (data.error) message = data.error;
        } catch (_) {}
        throw new Error(message);
      }

      latestBlob = await response.blob();
      latestText = text;
      return latestBlob;
    }

    function setBusy(isBusy) {
      previewBtn.disabled = isBusy;
      downloadBtn.disabled = isBusy;
    }

    function downloadBlob(blob) {
      const url = URL.createObjectURL(blob);
      const link = document.createElement('a');
      link.href = url;
      link.download = 'output.docx';
      document.body.appendChild(link);
      link.click();
      link.remove();
      URL.revokeObjectURL(url);
    }

    previewBtn.addEventListener('click', async () => {
      setBusy(true);
      statusEl.textContent = '生成预览中...';

      try {
        const blob = await generateDocx();
        previewEl.innerHTML = '';
        await docx.renderAsync(blob, previewEl, null, {
          className: 'docx',
          inWrapper: true,
          ignoreWidth: false,
          ignoreHeight: false
        });
        statusEl.textContent = '预览已更新';
      } catch (err) {
        statusEl.textContent = err.message || '生成失败';
      } finally {
        setBusy(false);
      }
    });

    downloadBtn.addEventListener('click', async () => {
      setBusy(true);
      statusEl.textContent = '生成中...';

      try {
        const blob = await generateDocx();
        downloadBlob(blob);
        statusEl.textContent = '已下载 output.docx';
      } catch (err) {
        statusEl.textContent = err.message || '生成失败';
      } finally {
        setBusy(false);
      }
    });
  </script>
</body>
</html>"#;
