use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::Mutex;
use uuid::Uuid;
#[derive(Clone, Serialize, Deserialize)]
pub struct Provider {
    pub name: String,
    pub protocol: String,
    pub base_url: String,
    #[serde(default)]
    pub key: String,
    pub model: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub path: String,
    pub name: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Session {
    pub messages: Vec<Value>,
    pub pending: Vec<Value>,
    pub grant: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub project_root: Option<PathBuf>,
    #[serde(default = "ask")]
    pub permission: String,
    #[serde(default)]
    pub events: Vec<Value>,
    #[serde(default)]
    pub title: String,
}
fn ask() -> String {
    "ask".into()
}
#[derive(Clone, Serialize, Deserialize, Default)]
struct Archive {
    projects: Vec<Project>,
    sessions: HashMap<String, Session>,
    provider: Option<Value>,
    #[serde(default)]
    profiles: Vec<Value>,
}
pub struct Data {
    pub provider: Option<Provider>,
    pub profiles: Vec<Provider>,
    pub project: Option<PathBuf>,
    pub projects: Vec<Project>,
    pub sessions: HashMap<String, Session>,
}
#[derive(Clone)]
pub struct Engine {
    pub data: Arc<Mutex<Data>>,
    pub client: reqwest::Client,
    store: Option<PathBuf>,
}
impl Engine {
    pub fn new() -> Self {
        Self::with_store(None)
    }
    pub fn with_store(store: Option<PathBuf>) -> Self {
        let archive = store
            .as_ref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|s| serde_json::from_slice::<Archive>(&s).ok())
            .unwrap_or_default();
        let provider = archive
            .provider
            .and_then(|v| serde_json::from_value(v).ok());
        Self {
            data: Arc::new(Mutex::new(Data {
                provider,
                profiles: archive
                    .profiles
                    .into_iter()
                    .filter_map(|v| serde_json::from_value(v).ok())
                    .collect(),
                project: None,
                projects: archive.projects,
                sessions: archive.sessions,
            })),
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(std::time::Duration::from_secs(180))
                .build()
                .unwrap(),
            store,
        }
    }
    fn save(&self, d: &Data) -> Result<(), String> {
        let Some(path) = &self.store else {
            return Ok(());
        };
        let parent = path.parent().ok_or("Нет каталога данных")?;
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
        }
        let provider=d.provider.as_ref().map(|p|json!({"name":p.name,"protocol":p.protocol,"base_url":p.base_url,"model":p.model,"key":""}));
        let archive = Archive {
            projects: d.projects.clone(),
            sessions: d.sessions.clone(),
            provider,
            profiles: d.profiles.iter().map(public_provider).collect(),
        };
        let bytes = serde_json::to_vec(&archive).map_err(|e| e.to_string())?;
        let temp = path.with_extension("tmp");
        let mut options = std::fs::OpenOptions::new();
        options.create(true).truncate(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut f = options.open(&temp).map_err(|e| e.to_string())?;
        use std::io::Write;
        f.write_all(&bytes)
            .and_then(|_| f.sync_all())
            .map_err(|e| e.to_string())?;
        std::fs::rename(temp, path).map_err(|e| format!("Не удалось сохранить историю: {e}"))
    }
    pub async fn dispatch(&self, method: &str, body: Value) -> Result<Value, String> {
        match method {
            "configure" => {
                let p: Provider =
                    serde_json::from_value(body["provider"].clone()).map_err(|e| e.to_string())?;
                validate_url(&p.base_url)?;
                if !["openai", "anthropic"].contains(&p.protocol.as_str())
                    || p.model.trim().is_empty()
                {
                    return Err("Укажите протокол и модель".into());
                }
                let path = body["project"].as_str().unwrap_or("");
                let root = if path.is_empty() {
                    None
                } else {
                    let p = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
                    if !p.is_dir() {
                        return Err("Проект должен быть папкой".into());
                    }
                    Some(p)
                };
                let mut d = self.data.lock().await;
                d.profiles.retain(|v| v.name != p.name);
                d.profiles.push(p.clone());
                d.provider = Some(p);
                if root.is_some() {
                    d.project = root;
                }
                self.save(&d)?;
                Ok(json!({"ok":true}))
            }
            "open_project" => {
                if cfg!(target_os = "android") {
                    return Err("Папки компьютера доступны после подключения к Linux".into());
                }
                let path = std::fs::canonicalize(body["path"].as_str().ok_or("Нет пути")?)
                    .map_err(|e| format!("Папка: {e}"))?;
                if !path.is_dir() {
                    return Err("Выберите папку".into());
                }
                let value = path.display().to_string();
                let mut d = self.data.lock().await;
                let project = if let Some(p) = d.projects.iter().find(|p| p.path == value) {
                    p.clone()
                } else {
                    let p = Project {
                        id: Uuid::new_v4().to_string(),
                        path: value,
                        name: path
                            .file_name()
                            .map(|v| v.to_string_lossy().into_owned())
                            .unwrap_or("Корень".into()),
                    };
                    d.projects.push(p.clone());
                    p
                };
                d.project = Some(path);
                self.save(&d)?;
                Ok(json!(project))
            }
            "projects" => Ok(json!(self.data.lock().await.projects)),
            "create_chat" => {
                let project_id = body["project_id"].as_str().map(str::to_string);
                let mut d = self.data.lock().await;
                if project_id
                    .as_ref()
                    .is_some_and(|id| !d.projects.iter().any(|p| &p.id == id))
                {
                    return Err("Проект не найден".into());
                }
                let id = Uuid::new_v4().to_string();
                d.sessions.insert(
                    id.clone(),
                    Session {
                        messages: vec![],
                        pending: vec![],
                        grant: None,
                        project_id,
                        project_root: None,
                        permission: ask(),
                        events: vec![],
                        title: "Новый чат".into(),
                    },
                );
                self.save(&d)?;
                Ok(json!({"id":id}))
            }
            "sessions" => {
                let d = self.data.lock().await;
                Ok(Value::Array(d.sessions.iter().map(|(id,s)|json!({"id":id,"title":s.title,"project_id":s.project_id,"permission":s.permission,"events":s.events,"pending":s.grant.as_ref().map(|g|json!({"grant":g,"calls":s.pending}))})).collect()))
            }
            "metadata" => {
                let d = self.data.lock().await;
                Ok(
                    json!({"provider":d.provider.as_ref().map(public_provider),"providers":d.profiles.iter().map(public_provider).collect::<Vec<_>>(),"project":d.project.as_ref().map(|p|p.display().to_string()),"tools":!cfg!(target_os="android"),"version":env!("CARGO_PKG_VERSION")}),
                )
            }
            "select_provider" => {
                let name = body["name"].as_str().ok_or("Нет имени")?;
                let mut d = self.data.lock().await;
                let p = d
                    .profiles
                    .iter()
                    .find(|p| p.name == name)
                    .cloned()
                    .ok_or("Провайдер не найден")?;
                d.provider = Some(p);
                self.save(&d)?;
                Ok(json!({"ok":true}))
            }
            "set_permission" => {
                let mode = body["mode"].as_str().ok_or("Нет режима")?;
                if !["ask", "read", "project"].contains(&mode) {
                    return Err("Неверный режим".into());
                }
                let mut d = self.data.lock().await;
                let s = d
                    .sessions
                    .get_mut(body["session"].as_str().ok_or("Нет сессии")?)
                    .ok_or("Нет сессии")?;
                s.permission = mode.into();
                self.save(&d)?;
                Ok(json!({"ok":true}))
            }
            "models" => {
                let p = self
                    .data
                    .lock()
                    .await
                    .provider
                    .clone()
                    .ok_or("Сначала сохраните провайдер")?;
                let r = self
                    .authorize(
                        self.client
                            .get(format!("{}/models", p.base_url.trim_end_matches('/'))),
                        &p,
                    )
                    .send()
                    .await
                    .map_err(|e| e.to_string())?;
                if !r.status().is_success() {
                    return Err(format!("Модели: HTTP {}", r.status()));
                }
                let v: Value = r.json().await.map_err(|e| e.to_string())?;
                Ok(v["data"].clone())
            }
            "agent_message" => {
                let text = body["text"].as_str().ok_or("Нет текста")?.to_string();
                let display = body["display_text"].as_str().unwrap_or("").to_string();
                if text.trim().is_empty() {
                    return Err("Пустое сообщение".into());
                }
                if text.len() > 100_000 {
                    return Err("Сообщение больше 100 000 байт".into());
                }
                let mut d = self.data.lock().await;
                if d.provider.is_none() {
                    return Err("Добавьте провайдер в настройках".into());
                }
                let id = body["session"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| Uuid::new_v4().to_string());
                if d.sessions.get(&id).is_some_and(|s| s.grant.is_some()) {
                    return Err("Сначала подтвердите или отклоните действие".into());
                }
                let fallback = if d.sessions.contains_key(&id) {
                    None
                } else {
                    d.project.clone()
                };
                let (uev, permission, title, transcript) = {
                    let s = d.sessions.entry(id.clone()).or_insert(Session {
                        messages: vec![],
                        pending: vec![],
                        grant: None,
                        project_id: None,
                        project_root: fallback.clone(),
                        permission: ask(),
                        events: vec![],
                        title: "Новый чат".into(),
                    });
                    if s.events.is_empty() && !display.trim().is_empty() {
                        s.title = display.chars().take(42).collect();
                    }
                    s.messages.push(json!({"role":"user","content":text}));
                    let uev = json!({"id":format!("user-{}",Uuid::new_v4()),"type":"user","text":display});
                    s.events.push(uev.clone());
                    (
                        uev,
                        s.permission.clone(),
                        s.title.clone(),
                        s.messages.clone(),
                    )
                };
                let provider = d.provider.clone().unwrap();
                let history = to_model_messages(&transcript);
                let root = {
                    let s = d.sessions.get(&id).ok_or("Нет сессии")?;
                    root_for(&d, s)?
                };
                let enabled = root.is_some() && !cfg!(target_os = "android");
                self.save(&d)?;
                Ok(json!({"session":id,"history":history,"title":title,"user_event":uev,"provider":{"protocol":provider.protocol,"base_url":provider.base_url,"model":provider.model},"permission":permission,"enabled":enabled,"root":root.map(|p|p.display().to_string())}))
            }
            "agent_append" => {
                let id = body["session"].as_str().ok_or("Нет сессии")?;
                let msgs = body["messages"]
                    .as_array()
                    .ok_or("Нет сообщений")?
                    .clone();
                if msgs.len() > 64 {
                    return Err("Слишком много сообщений за шаг".into());
                }
                let mut d = self.data.lock().await;
                let s = d.sessions.get_mut(id).ok_or("Нет сессии")?;
                for m in msgs {
                    if m["role"].as_str().is_none() {
                        return Err("Неверное сообщение".into());
                    }
                    s.messages.push(m);
                }
                self.save(&d)?;
                Ok(json!({"ok":true}))
            }
            "agent_event" => {
                let id = body["session"].as_str().ok_or("Нет сессии")?;
                let ev = body["event"].clone();
                if ev["id"].as_str().is_none() || ev["type"].as_str().is_none() {
                    return Err("Неверное событие".into());
                }
                let mut d = self.data.lock().await;
                let s = d.sessions.get_mut(id).ok_or("Нет сессии")?;
                if let Some(i) = s.events.iter().position(|e| e["id"] == ev["id"]) {
                    s.events[i] = ev;
                } else {
                    s.events.push(ev);
                }
                self.save(&d)?;
                Ok(json!({"ok":true}))
            }
            "agent_preview" => {
                let id = body["session"].as_str().ok_or("Нет сессии")?;
                let call = body["call"].clone();
                let d = self.data.lock().await;
                let root = {
                    let s = d.sessions.get(id).ok_or("Нет сессии")?;
                    root_for(&d, s)?
                };
                let name = call["function"]["name"].as_str().unwrap_or("");
                if !["write_file", "edit_file"].contains(&name) {
                    return Ok(json!({"preview":null}));
                }
                Ok(json!({"preview":preview_change(&call,root).unwrap_or(Value::Null)}))
            }
            "agent_tool" => {
                let id = body["session"].as_str().ok_or("Нет сессии")?;
                let allow = body["allow"].as_bool().unwrap_or(false);
                let mut call = body["call"].clone();
                if call["id"].as_str().is_none()
                    || call["function"]["name"].as_str().is_none()
                    || call["function"]["arguments"].as_str().is_none()
                {
                    return Err("Неверный вызов инструмента".into());
                }
                let mut d = self.data.lock().await;
                let (root, mode, name) = {
                    let s = d.sessions.get(id).ok_or("Нет сессии")?;
                    (
                        root_for(&d, s)?,
                        s.permission.clone(),
                        call["function"]["name"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                    )
                };
                let write = ["write_file", "edit_file", "shell"].contains(&name.as_str());
                let allow = allow && !(write && mode == "read");
                if allow
                    && ["write_file", "edit_file"].contains(&name.as_str())
                    && call["preview"].is_null()
                {
                    match preview_change(&call, root.clone()) {
                        Ok(v) => call["preview"] = v,
                        Err(e) => {
                            let result =
                                json!({"error":format!("Невозможно показать правку: {e}")});
                            let ev = json!({"id":call["id"],"type":"tool","name":name,"arguments":call["function"]["arguments"],"status":"exited","result":result,"preview":Value::Null});
                            d.sessions.get_mut(id).unwrap().events.push(ev);
                            self.save(&d)?;
                            return Ok(json!({"result":result,"preview":null}));
                        }
                    }
                }
                let result = if allow {
                    match execute(&call, root).await {
                        Ok(v) => v,
                        Err(e) => json!({"error":e}),
                    }
                } else {
                    json!({"error":"Действие отклонено"})
                };
                let ev = json!({"id":call["id"],"type":"tool","name":name,"arguments":call["function"]["arguments"],"status":"exited","result":result,"preview":call["preview"]});
                d.sessions.get_mut(id).unwrap().events.push(ev);
                self.save(&d)?;
                Ok(json!({"result":result,"preview":call["preview"]}))
            }
            "agent_pending" => {
                let id = body["session"].as_str().ok_or("Нет сессии")?;
                let mut d = self.data.lock().await;
                let s = d.sessions.get_mut(id).ok_or("Нет сессии")?;
                match (body["grant"].as_str(), body["calls"].as_array()) {
                    (Some(g), Some(c)) => {
                        s.grant = Some(g.to_string());
                        s.pending = c.clone();
                    }
                    _ => {
                        s.grant = None;
                        s.pending = vec![];
                    }
                }
                self.save(&d)?;
                Ok(json!({"ok":true}))
            }
            "provider_key" => {
                let d = self.data.lock().await;
                let p = d.provider.clone().ok_or("Сначала сохраните провайдер")?;
                if p.key.trim().is_empty() {
                    return Err("Ключ провайдера не задан".into());
                }
                Ok(json!(p.key))
            }
            _ => Err("Неизвестный метод".into()),
        }
    }
    fn authorize(&self, request: reqwest::RequestBuilder, p: &Provider) -> reqwest::RequestBuilder {
        if p.protocol == "anthropic" {
            request
                .header("x-api-key", &p.key)
                .header("anthropic-version", "2023-06-01")
        } else {
            request.bearer_auth(&p.key)
        }
    }
}
fn root_for(d: &Data, s: &Session) -> Result<Option<PathBuf>, String> {
    if let Some(pid) = &s.project_id {
        let p = d
            .projects
            .iter()
            .find(|p| &p.id == pid)
            .ok_or("Папка проекта не найдена")?;
        return Ok(Some(PathBuf::from(&p.path)));
    }
    Ok(s.project_root.clone().or_else(|| d.project.clone()))
}
fn to_model_messages(msgs: &[Value]) -> Value {
    let mut out: Vec<Value> = Vec::new();
    let mut names: HashMap<String, String> = HashMap::new();
    for m in msgs {
        let role = m["role"].as_str().unwrap_or("");
        if m["content"].is_array() {
            if role == "assistant" {
                if let Some(arr) = m["content"].as_array() {
                    for part in arr {
                        if part["type"] == "tool-call" {
                            names.insert(
                                part["toolCallId"].as_str().unwrap_or_default().to_string(),
                                part["toolName"].as_str().unwrap_or("").to_string(),
                            );
                        }
                    }
                }
            }
            out.push(m.clone());
            continue;
        }
        let content = m["content"].as_str().unwrap_or("");
        match role {
            "assistant" => {
                let mut parts = Vec::new();
                if !content.is_empty() {
                    parts.push(json!({"type":"text","text":content}));
                }
                if let Some(calls) = m["tool_calls"].as_array() {
                    for c in calls {
                        let cid = c["id"].as_str().unwrap_or_default().to_string();
                        let name = c["function"]["name"].as_str().unwrap_or("").to_string();
                        let args: Value = serde_json::from_str(
                            c["function"]["arguments"].as_str().unwrap_or("{}"),
                        )
                        .unwrap_or(json!({}));
                        names.insert(cid.clone(), name.clone());
                        parts.push(
                            json!({"type":"tool-call","toolCallId":cid,"toolName":name,"input":args}),
                        );
                    }
                }
                out.push(json!({"role":"assistant","content":parts}));
            }
            "tool" => {
                let cid = m["tool_call_id"].as_str().unwrap_or_default().to_string();
                let name = names.get(&cid).cloned().unwrap_or_default();
                out.push(
                    json!({"role":"tool","content":[{"type":"tool-result","toolCallId":cid,"toolName":name,"output":{"type":"text","value":content}}]}),
                );
            }
            _ => out.push(json!({"role":role,"content":content})),
        }
    }
    let mut called: HashSet<String> = HashSet::new();
    let mut answered: HashSet<String> = HashSet::new();
    for m in &out {
        if m["role"] == "assistant" {
            if let Some(arr) = m["content"].as_array() {
                for part in arr {
                    if part["type"] == "tool-call" {
                        if let Some(id) = part["toolCallId"].as_str() {
                            called.insert(id.to_string());
                        }
                    }
                }
            }
        } else if m["role"] == "tool" {
            if let Some(arr) = m["content"].as_array() {
                for r in arr {
                    if let Some(id) = r["toolCallId"].as_str() {
                        answered.insert(id.to_string());
                    }
                }
            }
        }
    }
    let mut fixed: Vec<Value> = Vec::new();
    for mut m in out {
        if m["role"] == "assistant" {
            if let Some(arr) = m["content"].as_array().cloned() {
                let parts: Vec<Value> = arr
                    .into_iter()
                    .filter(|part| {
                        part["type"] != "tool-call"
                            || part["toolCallId"]
                                .as_str()
                                .is_some_and(|id| answered.contains(id))
                    })
                    .collect();
                if parts.is_empty() {
                    continue;
                }
                m["content"] = Value::Array(parts);
            }
            fixed.push(m);
        } else if m["role"] == "tool" {
            if let Some(arr) = m["content"].as_array().cloned() {
                let results: Vec<Value> = arr
                    .into_iter()
                    .filter(|r| {
                        r["toolCallId"]
                            .as_str()
                            .is_some_and(|id| called.contains(id))
                    })
                    .collect();
                if results.is_empty() {
                    continue;
                }
                m["content"] = Value::Array(results);
            }
            fixed.push(m);
        } else {
            fixed.push(m);
        }
    }
    Value::Array(fixed)
}
fn public_provider(p: &Provider) -> Value {
    json!({"name":p.name,"protocol":p.protocol,"base_url":p.base_url,"model":p.model,"has_key":!p.key.is_empty()})
}
fn preview_change(c: &Value, root: Option<PathBuf>) -> Result<Value, String> {
    let root = root.ok_or("Нет папки проекта")?;
    let name = c["function"]["name"].as_str().ok_or("Нет инструмента")?;
    let args: Value = serde_json::from_str(c["function"]["arguments"].as_str().unwrap_or("{}"))
        .map_err(|e| e.to_string())?;
    let path = args["path"].as_str().ok_or("Нет пути")?;
    let file = safe_path(&root, path, true)?;
    let existed = file.exists();
    let before = if existed {
        if file.metadata().map_err(|e| e.to_string())?.len() > 262144 {
            return Err("Файл больше 256 KB".into());
        }
        std::fs::read_to_string(&file).map_err(|e| e.to_string())?
    } else {
        String::new()
    };
    let after = if name == "edit_file" {
        let old = args["old"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("Пустой old")?;
        if !existed || before.matches(old).count() != 1 {
            return Err("Фрагмент должен совпадать ровно один раз".into());
        }
        before.replacen(old, args["new"].as_str().ok_or("Нет нового текста")?, 1)
    } else {
        args["content"]
            .as_str()
            .ok_or("Нет содержимого")?
            .to_string()
    };
    if after.len() > 262144 {
        return Err("Изменение больше 256 KB".into());
    }
    Ok(json!({"path":path,"before":before,"after":after,"existed":existed}))
}
fn verify_preview(c: &Value, root: Option<&Path>) -> Result<(), String> {
    if c["preview"].is_null() {
        return Ok(());
    }
    let root = root.ok_or("Нет папки проекта")?;
    let preview = &c["preview"];
    let path = safe_path(root, preview["path"].as_str().ok_or("Нет пути")?, true)?;
    if preview["existed"] == true {
        if std::fs::read_to_string(path).map_err(|e| e.to_string())?
            != preview["before"].as_str().unwrap_or("")
        {
            return Err(
                "Файл изменился после показа diff. Правка не применена; запросите новый diff."
                    .into(),
            );
        }
    } else if path.exists() {
        return Err("Файл появился после показа diff. Перезапись отменена.".into());
    }
    Ok(())
}
pub fn validate_url(s: &str) -> Result<(), String> {
    let u = reqwest::Url::parse(s).map_err(|_| "Неверный URL")?;
    if !["http", "https"].contains(&u.scheme())
        || u.host_str().is_none()
        || !u.username().is_empty()
        || u.password().is_some()
        || u.query().is_some()
        || u.fragment().is_some()
    {
        return Err("Используйте http(s) URL без логина, query и fragment".into());
    }
    Ok(())
}
pub fn safe_path(root: &Path, p: &str, write: bool) -> Result<PathBuf, String> {
    let relative = Path::new(p);
    if relative.components().any(|c| {
        let x = c.as_os_str().to_string_lossy();
        x.starts_with(".env")
            || [
                ".ssh",
                ".aws",
                ".gnupg",
                "id_rsa",
                "id_ed25519",
                "credentials",
                ".npmrc",
            ]
            .contains(&x.as_ref())
    }) {
        return Err("Чтение и запись секретных файлов запрещены файловыми инструментами".into());
    }
    if relative.is_absolute()
        || relative
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("Путь должен находиться внутри проекта".into());
    }
    let joined = root.join(relative);
    let canonical = if write && !joined.exists() {
        let parent = joined
            .parent()
            .ok_or("Нет родителя")?
            .canonicalize()
            .map_err(|e| e.to_string())?;
        parent.join(joined.file_name().ok_or("Нет имени")?)
    } else {
        joined.canonicalize().map_err(|e| e.to_string())?
    };
    if !canonical.starts_with(root) {
        return Err("Выход из проекта, включая symlink, запрещён".into());
    }
    Ok(canonical)
}
async fn execute(c: &Value, root: Option<PathBuf>) -> Result<Value, String> {
    verify_preview(c, root.as_deref())?;
    if c["function"]["name"] == "recommend_options" {
        return serde_json::from_str(c["function"]["arguments"].as_str().unwrap_or("{}"))
            .map_err(|e| e.to_string());
    }
    if cfg!(target_os = "android") {
        return Err("Файловые инструменты доступны на Linux".into());
    }
    let root = root.ok_or("Выберите проект")?;
    let name = c["function"]["name"]
        .as_str()
        .ok_or("Нет инструмента")?
        .to_string();
    let args: Value = serde_json::from_str(
        c["function"]["arguments"]
            .as_str()
            .ok_or("Нет аргументов")?,
    )
    .map_err(|e| e.to_string())?;
    if name == "shell" {
        let command = args["command"].as_str().ok_or("Нет команды")?;
        if command.len() > 8192 {
            return Err("Команда слишком длинная".into());
        }
        let mut child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(&root)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| e.to_string())?;
        use tokio::io::AsyncReadExt;
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        async fn drain<R: tokio::io::AsyncRead + Unpin>(mut r: R) -> Vec<u8> {
            let mut all = vec![];
            let mut buf = [0; 4096];
            while let Ok(n) = r.read(&mut buf).await {
                if n == 0 {
                    break;
                }
                if all.len() < 65536 {
                    all.extend_from_slice(&buf[..n.min(65536 - all.len())])
                }
            }
            all
        }
        let mut a = tokio::spawn(drain(stdout));
        let mut b = tokio::spawn(drain(stderr));
        let status = tokio::time::timeout(std::time::Duration::from_secs(30), child.wait()).await;
        let status = match status {
            Ok(Ok(s)) => s,
            _ => {
                let _ = child.kill().await;
                a.abort();
                b.abort();
                return Err("Команда превысила 30 секунд".into());
            }
        };
        let captures = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            tokio::join!(&mut a, &mut b)
        })
        .await;
        let (out, err) = match captures {
            Ok((out, err)) => (out.unwrap_or_default(), err.unwrap_or_default()),
            Err(_) => {
                a.abort();
                b.abort();
                return Err("Команда завершилась, но дочерний процесс удерживает stdout/stderr. Фоновые процессы могут продолжить работу.".into());
            }
        };
        return Ok(
            json!({"code":status.code(),"stdout":String::from_utf8_lossy(&out),"stderr":String::from_utf8_lossy(&err)}),
        );
    }
    tokio::task::spawn_blocking(move||{match name.as_str(){
 "read_file"=>{let p=safe_path(&root,args["path"].as_str().ok_or("Нет пути")?,false)?;if p.metadata().map_err(|e|e.to_string())?.len()>262144{return Err("Файл больше 256KB".into())}Ok(json!({"content":std::fs::read_to_string(p).map_err(|e|e.to_string())?}))},
 "write_file"|"edit_file"=>{let p=safe_path(&root,args["path"].as_str().ok_or("Нет пути")?,true)?;let content=if name=="edit_file"{let old=args["old"].as_str().filter(|s|!s.is_empty()).ok_or("old пуст")?;if p.metadata().map_err(|e|e.to_string())?.len()>262144{return Err("Файл больше 256KB".into())}let text=std::fs::read_to_string(&p).map_err(|e|e.to_string())?;if text.matches(old).count()!=1{return Err("old должен совпасть ровно один раз".into())}text.replacen(old,args["new"].as_str().ok_or("Нет new")?,1)}else{args["content"].as_str().ok_or("Нет content")?.to_string()};if content.len()>262144{return Err("Максимум 256KB".into())}std::fs::write(p,content).map_err(|e|e.to_string())?;Ok(json!({"ok":true}))},
 "list_files"|"search_files"|"grep"=>{let base=if name=="list_files"{safe_path(&root,args["path"].as_str().unwrap_or("."),false)?}else{root.clone()};let re=if name=="grep"{Some(regex::RegexBuilder::new(args["pattern"].as_str().ok_or("Нет pattern")?).size_limit(1_000_000).build().map_err(|e|e.to_string())?)}else{None};let mut found=vec![];for entry in walkdir::WalkDir::new(base).max_depth(16).follow_links(false).into_iter().filter_entry(|e|![".git","node_modules","target",".env"].contains(&e.file_name().to_string_lossy().as_ref())).take(20000){let e=entry.map_err(|e|e.to_string())?;if !e.file_type().is_file(){continue}let rel=e.path().strip_prefix(&root).unwrap().display().to_string();if safe_path(&root,&rel,false).is_err(){continue}if name=="search_files"&&!rel.contains(args["query"].as_str().ok_or("Нет query")?){continue}if let Some(re)=&re{if e.metadata().map_err(|e|e.to_string())?.len()>262144{continue}if let Ok(text)=std::fs::read_to_string(e.path()){for (i,line) in text.lines().enumerate(){if re.is_match(line){found.push(json!({"path":rel,"line":i+1,"text":line.chars().take(400).collect::<String>()}));if found.len()>=100{break}}}}}else{found.push(json!({"path":rel}))}if found.len()>=100{break}}Ok(json!({"matches":found,"limit":100}))}, _=>Err("Неизвестный инструмент".into())}}).await.map_err(|e|e.to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn urls() {
        assert!(validate_url("https://api.example.com/v1").is_ok());
        assert!(validate_url("file:///etc").is_err());
        assert!(validate_url("https://user:pass@example.com").is_err());
    }
    #[test]
    fn paths() {
        let root = std::env::temp_dir().join(Uuid::new_v4().to_string());
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("a"), "ok").unwrap();
        assert!(safe_path(&root, "a", false).is_ok());
        assert!(safe_path(&root, "../etc/passwd", false).is_err());
        assert!(safe_path(&root, "/etc/passwd", false).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/etc/passwd", root.join("escape")).unwrap();
            assert!(safe_path(&root, "escape", false).is_err());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn model_messages() {
        let m = vec![
            json!({"role":"user","content":"hi"}),
            json!({"role":"assistant","content":"","tool_calls":[{"id":"x","function":{"name":"read_file","arguments":"{\"path\":\"a\"}"}}]}),
            json!({"role":"tool","tool_call_id":"x","content":"ok"}),
            json!({"role":"assistant","content":"", "tool_calls":[{"id":"dangling","function":{"name":"shell","arguments":"{}"}}]}),
        ];
        let out = to_model_messages(&m);
        assert_eq!(out[0], json!({"role":"user","content":"hi"}));
        assert_eq!(out[1]["content"][0]["type"], "tool-call");
        assert_eq!(out[1]["content"][0]["input"], json!({"path":"a"}));
        assert_eq!(out[2]["content"][0]["type"], "tool-result");
        assert_eq!(out[2]["content"][0]["toolCallId"], "x");
        assert_eq!(out[2]["content"][0]["toolName"], "read_file");
        assert_eq!(out.as_array().unwrap().len(), 3);
    }
}
#[cfg(test)]
mod integration_tests {
    use super::*;
    async fn engine_with_project() -> (Engine, PathBuf) {
        let root = std::env::temp_dir().join(Uuid::new_v4().to_string());
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("hello.txt"), "original").unwrap();
        let e = Engine::new();
        e.dispatch("configure",json!({"provider":{"name":"test","protocol":"openai","base_url":"https://api.example.com/v1","key":"TEST_SECRET_DO_NOT_LEAK","model":"test-model"},"project":root.display().to_string()})).await.unwrap();
        (e, root)
    }
    #[tokio::test]
    async fn agent_flow_approve_and_deny() {
        let (e, root) = engine_with_project().await;
        let started = e.dispatch("agent_message", json!({"text":"Change the file","display_text":"Change the file"})).await.unwrap();
        let id = started["session"].as_str().unwrap().to_string();
        assert_eq!(started["history"][0], json!({"role":"user","content":"Change the file"}));
        assert_eq!(started["provider"]["model"], "test-model");
        assert_eq!(started["permission"], "ask");
        assert_eq!(started["enabled"], true);
        assert_eq!(started["user_event"]["type"], "user");
        let read = json!({"id":"c1","function":{"name":"read_file","arguments":json!({"path":"hello.txt"}).to_string()}});
        let r = e.dispatch("agent_tool", json!({"session":id,"call":read,"allow":true})).await.unwrap();
        assert!(r["result"]["content"].as_str().unwrap_or("").contains("original"));
        let write = json!({"id":"c2","function":{"name":"write_file","arguments":json!({"path":"hello.txt","content":"changed"}).to_string()}});
        let denied = e.dispatch("agent_tool", json!({"session":id,"call":write,"allow":false})).await.unwrap();
        assert_eq!(denied["result"]["error"], "Действие отклонено");
        assert_eq!(std::fs::read_to_string(root.join("hello.txt")).unwrap(), "original");
        let previewed = e.dispatch("agent_preview", json!({"session":id,"call":write})).await.unwrap();
        assert_eq!(previewed["preview"]["after"], "changed");
        e.dispatch("set_permission", json!({"session":id,"mode":"project"})).await.unwrap();
        let ran = e.dispatch("agent_tool", json!({"session":id,"call":write,"allow":true})).await.unwrap();
        assert!(ran["result"].get("error").is_none());
        assert_eq!(ran["preview"]["after"], "changed");
        assert_eq!(std::fs::read_to_string(root.join("hello.txt")).unwrap(), "changed");
        e.dispatch("agent_append", json!({"session":id,"messages":[{"role":"assistant","content":[{"type":"text","text":"done"}]}]})).await.unwrap();
        let d = e.data.lock().await;
        let sess = &d.sessions[&id];
        assert_eq!(sess.messages.len(), 2);
        assert_eq!(sess.events.iter().filter(|v| v["type"]=="tool").count(), 3);
        drop(d);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn agent_read_mode_denies_writes_and_pending_blocks() {
        let (e, root) = engine_with_project().await;
        let started = e.dispatch("agent_message", json!({"text":"hi","display_text":"hi"})).await.unwrap();
        let id = started["session"].as_str().unwrap().to_string();
        e.dispatch("set_permission", json!({"session":id,"mode":"read"})).await.unwrap();
        let write = json!({"id":"c9","function":{"name":"shell","arguments":json!({"command":"echo x"}).to_string()}});
        let r = e.dispatch("agent_tool", json!({"session":id,"call":write,"allow":true})).await.unwrap();
        assert!(r["result"].get("error").is_some());
        let key = e.dispatch("provider_key", json!({})).await.unwrap();
        assert_eq!(key, "TEST_SECRET_DO_NOT_LEAK");
        assert!(!e.dispatch("metadata", json!({})).await.unwrap().to_string().contains("TEST_SECRET"));
        e.dispatch("agent_pending", json!({"session":id,"grant":"g1","calls":[write]})).await.unwrap();
        let sessions = e.dispatch("sessions", json!({})).await.unwrap();
        assert_eq!(sessions[0]["pending"]["grant"], "g1");
        assert!(e.dispatch("agent_message", json!({"session":id,"text":"more","display_text":"more"})).await.is_err());
        e.dispatch("agent_pending", json!({"session":id})).await.unwrap();
        e.dispatch("agent_message", json!({"session":id,"text":"more","display_text":"more"})).await.unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
#[cfg(test)]
mod production_tests {
    use super::*;
    fn provider(url: String, protocol: &str) -> Value {
        json!({"name":"test","protocol":protocol,"base_url":url,"key":"PRIVATE_TEST_KEY","model":"test"})
    }
    #[tokio::test]
    async fn persistence_projects_and_no_keys() {
        let root = std::env::temp_dir().join(Uuid::new_v4().to_string());
        std::fs::create_dir(&root).unwrap();
        let path = root.join("private/history.json");
        let e = Engine::with_store(Some(path.clone()));
        let project = e
            .dispatch("open_project", json!({"path":root}))
            .await
            .unwrap();
        let duplicate = e
            .dispatch("open_project", json!({"path":root.join(".")}))
            .await
            .unwrap();
        assert_eq!(project["id"], duplicate["id"]);
        let chat = e
            .dispatch("create_chat", json!({"project_id":project["id"]}))
            .await
            .unwrap();
        e.dispatch(
            "configure",
            json!({"provider":provider("https://api.example.com/v1".into(),"openai")}),
        )
        .await
        .unwrap();
        e.dispatch(
            "set_permission",
            json!({"session":chat["id"],"mode":"read"}),
        )
        .await
        .unwrap();
        assert!(!std::fs::read_to_string(&path)
            .unwrap()
            .contains("PRIVATE_TEST_KEY"));
        let restored = Engine::with_store(Some(path));
        assert_eq!(
            restored.dispatch("projects", json!({})).await.unwrap()[0]["id"],
            project["id"]
        );
        assert_eq!(
            restored.dispatch("sessions", json!({})).await.unwrap()[0]["permission"],
            "read"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn diff_snapshot_rejects_stale_file() {
        let root = std::env::temp_dir().join(Uuid::new_v4().to_string());
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("a.txt"), "hello world\n").unwrap();
        let mut call = json!({"id":"edit","function":{"name":"edit_file","arguments":json!({"path":"a.txt","old":"world","new":"there"}).to_string()}});
        let preview = preview_change(&call, Some(root.clone())).unwrap();
        assert_eq!(preview["before"], "hello world\n");
        assert_eq!(preview["after"], "hello there\n");
        call["preview"] = preview;
        std::fs::write(root.join("a.txt"), "hello world changed\n").unwrap();
        assert!(execute(&call, Some(root.clone())).await.is_err());
        assert_eq!(
            std::fs::read_to_string(root.join("a.txt")).unwrap(),
            "hello world changed\n"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn fresh_write_preview() {
        let root = std::env::temp_dir().join(Uuid::new_v4().to_string());
        std::fs::create_dir(&root).unwrap();
        let c = json!({"function":{"name":"write_file","arguments":json!({"path":"new.txt","content":"new\n"}).to_string()}});
        let preview = preview_change(&c, Some(root.clone())).unwrap();
        assert_eq!(preview["before"], "");
        assert_eq!(preview["after"], "new\n");
        assert_eq!(preview["existed"], false);
        std::fs::remove_dir_all(root).unwrap();
    }
}
