mod engine;
use axum::{
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use engine::Engine;
use tauri_plugin_secure_keystore::{SecureKeystoreExt, ItemKey, SetItemRequest};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;
#[derive(Clone)]
struct Shared {
    engine: Engine,
    gate: Arc<Mutex<()>>,
    credentials_loaded: Arc<Mutex<bool>>,
    credential_warning: Arc<Mutex<Option<String>>>,
    host: Arc<Mutex<Option<Host>>>,
}
struct Host {
    token: String,
    http: tokio::task::JoinHandle<()>,
    udp: tokio::task::JoinHandle<()>,
}
#[derive(Clone)]
struct Web {
    shared: Shared,
    token: String,
}
async fn api(
    State(w): State<Web>,
    Path(method): Path<String>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let expected = format!("Bearer {}", w.token);
    let authorized =
        headers.get("authorization").and_then(|h| h.to_str().ok()) == Some(expected.as_str());
    let active = w
        .shared
        .host
        .lock()
        .await
        .as_ref()
        .is_some_and(|h| h.token == w.token);
    if !authorized || !active {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"Неверный токен подключения"})),
        ));
    }
    if ![
        "metadata",
        "models",
        "sessions",
        "projects",
        "create_chat",
        "open_project",
        "set_permission",
        "select_provider",
        "agent_message",
        "agent_append",
        "agent_event",
        "agent_preview",
        "agent_tool",
        "agent_pending",
        "provider_key",
    ]
    .contains(&method.as_str())
    {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error":"Удалённая смена провайдера запрещена"})),
        ));
    }
    w.shared
        .engine
        .dispatch(&method, body)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(json!({"error":e}))))
}

fn credential_id(p: &engine::Provider) -> String {
    format!("provider:{}:{}:{}:{}:{}:{}",p.protocol.len(),p.protocol,p.base_url.len(),p.base_url,p.name.len(),p.name)
}
async fn restore_credentials(app:&tauri::AppHandle,state:&Shared) {
    let mut loaded=state.credentials_loaded.lock().await;
    if *loaded {return;}
    let profiles=state.engine.data.lock().await.profiles.clone();
    let mut failed=false;
    for profile in profiles {
        let handle=app.clone();let key=credential_id(&profile);
        match tauri::async_runtime::spawn_blocking(move||handle.secure_keystore().get_item(ItemKey{key})).await {
            Ok(Ok(value))=>if let Some(secret)=value.value {
                let mut d=state.engine.data.lock().await;
                for p in &mut d.profiles {if credential_id(p)==credential_id(&profile){p.key=secret.clone();}}
                if let Some(p)=&mut d.provider {if credential_id(p)==credential_id(&profile){p.key=secret;}}
            },
            _=>failed=true,
        }
    }
    if failed {*state.credential_warning.lock().await=Some("Не удалось открыть системное хранилище ключей. На Linux разблокируй GNOME Keyring / KWallet и перезапусти приложение; ключ можно ввести для текущего запуска.".into());}
    *loaded=true;
}

#[tauri::command]
async fn rpc(
    app: tauri::AppHandle,
    state: tauri::State<'_, Shared>,
    method: String,
    body: Value,
    url: Option<String>,
    token: Option<String>,
) -> Result<Value, String> {
    if let Some(url) = url.filter(|s| !s.is_empty()) {
        engine::validate_url(&url)?;
        let parsed = reqwest::Url::parse(&url).map_err(|e| e.to_string())?;
        if parsed.scheme() == "http" {
            let host = parsed.host_str().unwrap_or("");
            let lan = host == "localhost"
                || host.parse::<std::net::IpAddr>().is_ok_and(|ip| match ip {
                    std::net::IpAddr::V4(v) => {
                        v.is_private() || v.is_loopback() || v.is_link_local()
                    }
                    std::net::IpAddr::V6(v) => {
                        v.is_loopback() || v.is_unique_local() || v.is_unicast_link_local()
                    }
                });
            if !lan {
                return Err("Для подключения через интернет используйте HTTPS. HTTP разрешён только для локальных IP.".into());
            }
        }
        if ![
            "metadata",
            "models",
            "sessions",
            "projects",
            "create_chat",
            "open_project",
            "set_permission",
            "select_provider",
            "agent_message",
            "agent_append",
            "agent_event",
            "agent_preview",
            "agent_tool",
            "agent_pending",
            "provider_key",
        ]
        .contains(&method.as_str())
        {
            return Err("Метод запрещён для удалённого подключения".into());
        }
        let r = state
            .engine
            .client
            .post(format!("{}/api/{}", url.trim_end_matches('/'), method))
            .bearer_auth(token.unwrap_or_default())
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = r.status();
        let v: Value = r.json().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(v["error"].as_str().unwrap_or("Ошибка сервера").into());
        }
        return Ok(v);
    }
    let _credential_gate = if ["configure", "forget_key"].contains(&method.as_str()) {Some(state.gate.lock().await)} else {None};
    restore_credentials(&app, &state).await;
    let mut body = body;
    if method == "forget_key" {
        let name = body["name"].as_str().ok_or("Нет имени провайдера")?;
        let profile = state.engine.data.lock().await.profiles.iter().find(|p|p.name == name).cloned().ok_or("Провайдер не найден")?;
        let key = credential_id(&profile);
        let handle=app.clone();
        tauri::async_runtime::spawn_blocking(move || handle.secure_keystore().delete_item(ItemKey{key})).await.map_err(|_|"Хранилище недоступно")?.map_err(|_|"Не удалось удалить сохранённый ключ. Проверь хранилище системы.")?;
        let mut d=state.engine.data.lock().await;
        for p in &mut d.profiles {if credential_id(p)==credential_id(&profile){p.key.clear();}}
        if let Some(p)=&mut d.provider {if credential_id(p)==credential_id(&profile){p.key.clear();}}
        return Ok(json!({"ok":true}));
    }
    if method == "configure" {
        // An empty field means keep the existing secret only for this exact
        // provider identity. Never reuse a key after the destination changes.
        if body["provider"]["key"].as_str().unwrap_or("").is_empty() {
            if let Ok(profile)=serde_json::from_value::<engine::Provider>(body["provider"].clone()) {
                let d=state.engine.data.lock().await;
                if let Some(old)=d.profiles.iter().find(|p|credential_id(p)==credential_id(&profile)) {
                    body["provider"]["key"]=json!(old.key);
                }
            }
        }
    }
    let configured_profile = if method == "configure" {serde_json::from_value::<engine::Provider>(body["provider"].clone()).ok()} else {None};
    let mut result=state.engine.dispatch(&method, body).await?;
    if method == "configure" {
        let p=configured_profile.ok_or("Нет провайдера")?;
        let has_key=!p.key.is_empty();
        let saved=if has_key {
            let handle=app.clone();let key=credential_id(&p);let value=p.key;
            tauri::async_runtime::spawn_blocking(move||handle.secure_keystore().set_item(SetItemRequest{key,value})).await.is_ok_and(|v|v.is_ok())
        } else {false};
        result["key_saved"]=json!(saved);
        let warning=if has_key&&!saved {Some("Ключ работает в этом запуске, но не сохранён: системное хранилище недоступно. На Linux нужен разблокированный Secret Service (GNOME Keyring или KWallet).".to_string())} else {None};
        *state.credential_warning.lock().await=warning.clone();
        result["credential_warning"]=json!(warning);
    }
    if method == "metadata" {result["credential_warning"]=json!(*state.credential_warning.lock().await);}
    Ok(result)
}
#[tauri::command]
async fn host_start(state: tauri::State<'_, Shared>) -> Result<Value, String> {
    if cfg!(target_os = "android") {
        return Err("Сервер запускается на Linux".into());
    }
    let mut host = state.host.lock().await;
    if let Some(h) = host.as_ref() {
        return Ok(json!({"token":h.token,"port":8787,"name":"Velocity Harness"}));
    }
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8787")
        .await
        .map_err(|e| format!("Порт 8787: {e}"))?;
    let socket = tokio::net::UdpSocket::bind("0.0.0.0:8788")
        .await
        .map_err(|e| format!("Discovery: {e}"))?;
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let web = Web {
        shared: state.inner().clone(),
        token: token.clone(),
    };
    let app = Router::new()
        .route(
            "/health",
            get(|| async { Json(json!({"app":"Velocity Harness","version":env!("CARGO_PKG_VERSION")})) }),
        )
        .route("/api/{method}", post(api))
        .layer(DefaultBodyLimit::max(524288))
        .with_state(web);
    let http = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    let udp = tokio::spawn(async move {
        let mut buf = [0u8; 128];
        loop {
            let Ok((n, addr)) = socket.recv_from(&mut buf).await else {
                break;
            };
            if &buf[..n] == b"VELOCITY_DISCOVER_V1" {
                let _ = socket
                    .send_to(
                        concat!("{\"name\":\"Velocity Harness\",\"port\":8787,\"version\":\"",env!("CARGO_PKG_VERSION"),"\"}").as_bytes(),
                        addr,
                    )
                    .await;
            }
        }
    });
    *host = Some(Host {
        token: token.clone(),
        http,
        udp,
    });
    Ok(json!({"token":token,"port":8787,"name":"Velocity Harness"}))
}
#[tauri::command]
async fn host_stop(state: tauri::State<'_, Shared>) -> Result<(), String> {
    if let Some(h) = state.host.lock().await.take() {
        h.http.abort();
        h.udp.abort();
    }
    Ok(())
}
#[tauri::command]
async fn discover() -> Result<Value, String> {
    let sock = tokio::net::UdpSocket::bind("0.0.0.0:0")
        .await
        .map_err(|e| e.to_string())?;
    sock.set_broadcast(true).map_err(|e| e.to_string())?;
    for _ in 0..3 {
        sock.send_to(b"VELOCITY_DISCOVER_V1", "255.255.255.255:8788")
            .await
            .map_err(|e| e.to_string())?;
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let mut found = std::collections::BTreeMap::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(3);
    let mut buf = [0u8; 1024];
    loop {
        match tokio::time::timeout_at(deadline, sock.recv_from(&mut buf)).await {
            Ok(Ok((n, addr))) => {
                if let Ok(v) = serde_json::from_slice::<Value>(&buf[..n]) {
                    if v["name"] == "Velocity Harness" {
                        found.insert(addr.ip().to_string(),json!({"name":"Velocity Harness","url":format!("http://{}:8787",addr.ip())}));
                    }
                }
            }
            _ => break,
        }
    }
    Ok(json!(found.into_values().collect::<Vec<_>>()))
}
#[tauri::command]
fn platform() -> Value {
    json!({"android":cfg!(target_os="android")})
}
fn plan_dir() -> Result<std::path::PathBuf, String> {
    let dir = std::path::PathBuf::from("/tmp/vhr");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}
fn plan_name(name: &str) -> Result<String, String> {
    if name.is_empty() || name.len() > 64 || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("Bad plan name".into());
    }
    Ok(format!("{name}.md"))
}
#[tauri::command]
fn plan_save(name: String, content: String) -> Result<Value, String> {
    if content.len() > 500_000 {
        return Err("Plan too large".into());
    }
    let file = plan_dir()?.join(plan_name(&name)?);
    std::fs::write(&file, content).map_err(|e| e.to_string())?;
    Ok(json!({"path": file.display().to_string()}))
}
#[tauri::command]
fn plan_read(name: String) -> Result<Value, String> {
    let file = plan_dir()?.join(plan_name(&name)?);
    let content = std::fs::read_to_string(&file).map_err(|e| e.to_string())?;
    Ok(json!({"path": file.display().to_string(), "content": content}))
}
#[tauri::command]
fn plan_list() -> Result<Value, String> {
    let dir = plan_dir()?;
    let mut out = vec![];
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "md") {
                out.push(p.file_stem().unwrap_or_default().to_string_lossy().to_string());
            }
        }
    }
    out.sort();
    Ok(json!(out))
}
#[tauri::command]
async fn pick_folder() -> Result<Option<String>, String> {
    #[cfg(target_os = "android")]
    {
        Err("Выберите папку на Linux-компьютере".into())
    }
    #[cfg(not(target_os = "android"))]
    {
        // rfd::FileDialog::pick_folder() runs a blocking GTK loop.
        // Awaiting it directly in an async command stalls the async
        // runtime and hangs the whole webview until the dialog closes
        // (or forever on headless/Wayland). Push it to a blocking thread.
        tokio::task::spawn_blocking(|| {
            rfd::FileDialog::new()
                .set_title("Папка проекта Velocity")
                .pick_folder()
                .map(|p| p.display().to_string())
        })
        .await
        .map_err(|e| e.to_string())
    }
}
/// Origin of the Velocity key service (redeem + model catalog + OpenAI-compatible API).
const VELOCITY_ORIGIN: &str = "https://velocity.holy-voice-dd33.workers.dev";

/// Pull a `VEL-XXXX` code out of whatever the user pasted, so a full link or a
/// sentence from the site still works. Falls back to the trimmed text as-is.
fn velocity_code(input: &str) -> Result<String, String> {
    let re = regex::Regex::new(r"VEL-[A-Za-z0-9_-]+").expect("static velocity code pattern");
    let code = re.find(input).map(|m| m.as_str()).unwrap_or(input).trim();
    if code.is_empty() {
        return Err("Вставь ключ формата VEL-XXXX".into());
    }
    Ok(code.to_string())
}

/// Redeem a key against the Velocity worker and return everything needed to
/// configure the provider: api key, balances, base url and the model catalog.
/// Done in Rust because the webview CSP blocks cross-origin fetches.
async fn redeem_velocity(client: &reqwest::Client, code: &str) -> Result<Value, String> {
    let response = client
        .post(format!("{VELOCITY_ORIGIN}/api/redeem"))
        .json(&json!({"code": code}))
        .send()
        .await
        .map_err(|e| format!("Сервер Velocity недоступен: {e}"))?;
    let status = response.status();
    let body: Value = response
        .json()
        .await
        .map_err(|e| format!("Некорректный ответ сервера Velocity: {e}"))?;
    if let Some(error) = body.get("error").and_then(|v| v.as_str()) {
        return Err(error.into());
    }
    if !status.is_success() {
        return Err(format!("Ключ не активирован (HTTP {status})"));
    }
    // The site itself falls back to the raw code when the worker returns no apiKey.
    let api_key = body
        .get("apiKey")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(code)
        .to_string();
    let models = match client
        .get(format!("{VELOCITY_ORIGIN}/api/models"))
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => response
            .json::<Value>()
            .await
            .ok()
            .and_then(|v| v.get("models").cloned())
            .unwrap_or_else(|| json!([])),
        _ => json!([]),
    };
    Ok(json!({
        "code": code,
        "api_key": api_key,
        "base_url": format!("{VELOCITY_ORIGIN}/v1"),
        "balances": body.get("balances").cloned().unwrap_or_else(|| json!({})),
        "totals": body.get("totals").cloned().unwrap_or_else(|| json!({})),
        "models": models,
    }))
}

#[tauri::command]
async fn velocity_redeem(code: String) -> Result<Value, String> {
    let code = velocity_code(&code)?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    redeem_velocity(&client, &code).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri::Manager;
    tauri::Builder::default()
        .plugin(tauri_plugin_secure_keystore::init())
        .plugin(tauri_plugin_http::init())
        .setup(|app| {
            let store = app.path().app_data_dir()?.join("history.json");
            app.manage(Shared {
                engine: Engine::with_store(Some(store)),
                gate: Arc::new(Mutex::new(())),
                credentials_loaded: Arc::new(Mutex::new(false)),
                credential_warning: Arc::new(Mutex::new(None)),
                host: Arc::new(Mutex::new(None)),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            rpc,
            host_start,
            host_stop,
            discover,
            platform,
            pick_folder,
            plan_save,
            plan_read,
            plan_list,
            velocity_redeem
        ])
        .run(tauri::generate_context!())
        .expect("Velocity Harness launch failed");
}
#[cfg(test)]
mod network_tests {
    use super::*;
    #[tokio::test]
    async fn authentication_and_no_remote_configuration() {
        let shared = Shared {
            engine: Engine::new(),
            gate: Arc::new(Mutex::new(())),
                credentials_loaded: Arc::new(Mutex::new(false)),
                credential_warning: Arc::new(Mutex::new(None)),
            host: Arc::new(Mutex::new(None)),
        };
        let token = "test-pairing-token".to_string();
        let h = tokio::spawn(async { std::future::pending::<()>().await });
        let u = tokio::spawn(async { std::future::pending::<()>().await });
        *shared.host.lock().await = Some(Host {
            token: token.clone(),
            http: h,
            udp: u,
        });
        let w = Web {
            shared: shared.clone(),
            token: token.clone(),
        };
        let router = Router::new()
            .route("/api/{method}", post(api))
            .with_state(w);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let c = reqwest::Client::new();
        assert_eq!(
            c.post(format!("http://{addr}/api/metadata"))
                .json(&json!({}))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            c.post(format!("http://{addr}/api/configure"))
                .bearer_auth(&token)
                .json(&json!({}))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            c.post(format!("http://{addr}/api/metadata"))
                .bearer_auth(&token)
                .json(&json!({}))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        if let Some(host) = shared.host.lock().await.take() {
            host.http.abort();
            host.udp.abort();
        }
        assert_eq!(
            c.post(format!("http://{addr}/api/metadata"))
                .bearer_auth(&token)
                .json(&json!({}))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );
        task.abort();
    }
}

#[cfg(test)]
mod credential_tests {
 use super::*;
 fn provider(name:&str,url:&str)->engine::Provider {engine::Provider{name:name.into(),protocol:"openai".into(),base_url:url.into(),key:"NEVER_EXPORT_SECRET".into(),model:"model".into()}}
 #[test] fn secret_identity_is_bound_to_destination_and_profile() {
  let p=provider("test","https://api.example.com/v1");let mut q=p.clone();
  q.model="another-model".into();assert_eq!(credential_id(&p),credential_id(&q));
  q.base_url="https://other.example.com/v1".into();assert_ne!(credential_id(&p),credential_id(&q));
  q=p.clone();q.name="other".into();assert_ne!(credential_id(&p),credential_id(&q));
  q=p.clone();q.protocol="anthropic".into();assert_ne!(credential_id(&p),credential_id(&q));
  assert!(!credential_id(&p).contains("NEVER_EXPORT_SECRET"));
 }
}

#[cfg(test)]
mod velocity_tests {
    use super::*;

    #[test]
    fn extracts_code_from_pasted_text() {
        assert_eq!(velocity_code("VEL-AB12").unwrap(), "VEL-AB12");
        assert_eq!(
            velocity_code("https://velocity.holy-voice-dd33.workers.dev/#/key?k=VEL-XY99 ")
                .unwrap(),
            "VEL-XY99"
        );
        assert_eq!(velocity_code("ключ VEL-ZZ01 из админки").unwrap(), "VEL-ZZ01");
        assert!(velocity_code("   ").is_err());
        assert!(velocity_code("").is_err());
    }

    #[tokio::test]
    #[ignore = "needs network access to the Velocity worker"]
    async fn redeems_and_surfaces_worker_error() {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .unwrap();
        let error = redeem_velocity(&client, "VEL-NOPE").await.unwrap_err();
        assert!(
            error.to_lowercase().contains("invalid") || error.contains("Неверн"),
            "unexpected error: {error}"
        );
    }
}
