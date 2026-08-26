use account::Account;
use axum::body::Body;
use axum::debug_handler;
use axum::extract::ConnectInfo;
use axum::extract::State;
use axum::http::header::{CACHE_CONTROL, EXPIRES, PRAGMA};
use axum::http::HeaderMap;
use axum::http::HeaderValue;
use axum::http::Request;
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::IntoResponse;
use axum::response::Json;
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use axum_extra::extract::cookie::CookieJar;
use axum_server::tls_rustls::RustlsConfig;
use bb8::Pool;
use bb8_postgres::PostgresConnectionManager;

use chrono::DateTime;
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use native_tls::TlsConnector;
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::env;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};
use tokio_postgres::error::SqlState;
use tokio_postgres::NoTls;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::connect_async_tls_with_config;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::Connector;
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

const HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(5);
const HEALTH_CHECK_HEADER_VALUE: &str = "true";

mod account;

// Struct to represent the JSON request body
#[derive(Deserialize)]
struct AuthRequest {
    account_name: String,
    password: String,
}

#[derive(Serialize)]
struct AuthResponse {
    #[serde(rename = "playerId")]
    player_id: i32,
    account_status: AccountStatus,
    has_recovery_email: bool,
    needs_hero: bool,
    account_name: Option<String>,
}

// Registration carries an optional recovery email on top of the login fields.
#[derive(Deserialize)]
struct RegisterRequest {
    account_name: String,
    password: String,
    #[serde(default)]
    email: Option<String>,
}

#[derive(Deserialize)]
struct PasswordResetRequest {
    // Account name or email — whichever the player remembers.
    identifier: String,
}

#[derive(Deserialize)]
struct ResetPasswordRequest {
    token: String,
    password: String,
}

#[derive(Serialize)]
struct MessageResponse {
    message: String,
}

// Body sent to the Cloudflare email-sending API.
#[derive(Serialize)]
struct CloudflareEmail<'a> {
    to: &'a str,
    from: &'a str,
    subject: &'a str,
    html: &'a str,
    text: &'a str,
}

#[derive(Serialize)]
struct LogoutResponse {
    success: bool,
}

#[derive(Debug, Serialize)]
struct ScoreResponse(Vec<Score>);

#[derive(Debug, Serialize)]
struct Score {
    id: i32,
    hero_name: String,
    hero_rank: String,
    total_xp: i32,
    total_score: i32,
    score_survival: i32,
    score_progression: i32,
    score_wealth: i32,
    score_defense: i32,
    score_valor: i32,
    score_legacy: i32,
    days_survived: i32,
    highest_pressure_level: i32,
    waves_survived: i32,
    legendary_kills: i32,
    hideouts_cleared: i32,
    fate: String,
    crisis_tier: i32,
}

#[derive(Deserialize)]
struct DeviceAuthRequest {
    #[serde(default)]
    create_guest: bool,
}

struct ResolvedAccount {
    player_id: i32,
    account_name: Option<String>,
    password: Option<String>,
    email: Option<String>,
    player_state: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeviceAuthResponse {
    player_id: i32,
    has_account: bool,
    new_player: bool,
    account_status: AccountStatus,
    has_recovery_email: bool,
    needs_hero: bool,
    #[serde(rename = "account_name")]
    account_name: Option<String>,
}

#[derive(Serialize)]
struct SessionResponse {
    #[serde(rename = "playerId")]
    player_id: i32,
    account_name: Option<String>,
    account_status: AccountStatus,
    has_recovery_email: bool,
    needs_hero: bool,
}

#[derive(Serialize)]
struct AuthError {
    #[serde(rename = "error")]
    msg: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum AccountStatus {
    Guest,
    Secured,
}

impl AccountStatus {
    fn from_password(password: Option<&str>) -> Self {
        if password.is_some() {
            Self::Secured
        } else {
            Self::Guest
        }
    }

    fn is_secured(self) -> bool {
        self == Self::Secured
    }
}

#[derive(Serialize)]
struct HealthResponse {
    healthy: bool,
    message: String,
}

#[derive(Clone)]
struct AppState {
    pool: Pool<PostgresConnectionManager<NoTls>>,
    rng: Arc<Mutex<ChaCha20Rng>>,
    ws_health_url: String,
    ws_health_allow_invalid_certs: bool,
    // Per-IP fixed-window counter for the auth endpoints (R7).
    rate_limiter: Arc<Mutex<HashMap<IpAddr, (Instant, u32)>>>,
}

// Auth endpoints allow at most
// AUTH_RATE_MAX attempts per AUTH_RATE_WINDOW per client IP.
const AUTH_RATE_WINDOW: Duration = Duration::from_secs(60);
const AUTH_RATE_MAX: u32 = 20;
const SESSION_IDLE_DAYS: i64 = 7;
const TRUSTED_DEVICE_TTL_DAYS: i64 = 90;
const MAX_TRUSTED_DEVICES_PER_PLAYER: i64 = 5;
const TRUSTED_DEVICE_COOKIE: &str = "__Host-trusted_device";

impl AppState {
    // Records a hit for this IP and returns true if it is still under the limit.
    async fn rate_limit_ok(&self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let mut map = self.rate_limiter.lock().await;

        // Opportunistic cleanup so the map can't grow without bound under attack.
        if map.len() > 10_000 {
            map.retain(|_, (start, _)| now.duration_since(*start) <= AUTH_RATE_WINDOW);
        }

        let entry = map.entry(ip).or_insert((now, 0));
        if now.duration_since(entry.0) > AUTH_RATE_WINDOW {
            *entry = (now, 0);
        }
        entry.1 += 1;
        entry.1 <= AUTH_RATE_MAX
    }
}

// Resolves the real client IP for rate limiting. Behind Cloudflare the peer is
// the proxy, so prefer CF-Connecting-IP (set/overwritten by Cloudflare), then a
// generic X-Forwarded-For first hop, then the direct peer. NOTE: if the server
// is NOT behind a trusted proxy these headers are client-spoofable, which only
// weakens the limit rather than locking legitimate users out.
fn client_ip(headers: &HeaderMap, peer: SocketAddr) -> IpAddr {
    if let Some(ip) = headers
        .get("cf-connecting-ip")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<IpAddr>().ok())
    {
        return ip;
    }
    if let Some(ip) = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.trim().parse::<IpAddr>().ok())
    {
        return ip;
    }
    peer.ip()
}

fn session_is_expired(created_at: DateTime<Utc>, last_login: Option<DateTime<Utc>>) -> bool {
    let last_active = last_login.unwrap_or(created_at);
    Utc::now().signed_duration_since(last_active) > chrono::Duration::days(SESSION_IDLE_DAYS)
}

fn valid_account_name(account_name: &str) -> bool {
    (3..=20).contains(&account_name.len())
        && account_name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn valid_new_password(password: &str) -> bool {
    (8..=128).contains(&password.len())
}

fn valid_email(email: &str) -> bool {
    if email.len() > 255 || email.contains(char::is_whitespace) {
        return false;
    }
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && domain.contains('.')
        && !domain.contains('@')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
}

fn append_session_cookie(headers: &mut HeaderMap, session: &str) {
    let cookie =
        format!("session={session}; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=604800");
    headers.append("Set-Cookie", HeaderValue::from_str(&cookie).unwrap());
}

fn trusted_device_hash(credential: &str) -> String {
    format!("{:x}", Sha256::digest(credential.as_bytes()))
}

fn append_trusted_device_cookie(headers: &mut HeaderMap, credential: &str) {
    let max_age = TRUSTED_DEVICE_TTL_DAYS * 24 * 60 * 60;
    let cookie = format!(
        "{TRUSTED_DEVICE_COOKIE}={credential}; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age={max_age}"
    );
    headers.append("Set-Cookie", HeaderValue::from_str(&cookie).unwrap());
}

fn append_cleared_trusted_device_cookie(headers: &mut HeaderMap) {
    headers.append(
        "Set-Cookie",
        HeaderValue::from_static(
            "__Host-trusted_device=; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=0",
        ),
    );
}

fn append_cleared_auth_cookies(headers: &mut HeaderMap) {
    headers.append(
        "Set-Cookie",
        HeaderValue::from_static("session=; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=0"),
    );
    append_cleared_trusted_device_cookie(headers);
}

fn too_many_requests() -> Response {
    (
        StatusCode::TOO_MANY_REQUESTS,
        Json(AuthError {
            msg: "Too many attempts. Please wait a minute and try again.".to_string(),
        }),
    )
        .into_response()
}

// Issues a fresh trusted-device credential and prunes expired credentials so
// the trusted_devices table cannot grow without bound.
async fn issue_trusted_device(
    conn: &tokio_postgres::Client,
    player_id: i32,
) -> Result<String, String> {
    let trusted_device = Uuid::new_v4().to_string();
    let credential_hash = trusted_device_hash(&trusted_device);
    conn.execute(
        "INSERT INTO trusted_devices (player_id, credential_hash, created_at) VALUES ($1, $2, NOW())",
        &[&player_id, &credential_hash],
    )
    .await
    .map_err(|error| format!("Error storing device token: {error}"))?;

    conn
        .execute(
            "DELETE FROM trusted_devices WHERE player_id = $1 AND created_at < NOW() - INTERVAL '90 days'",
            &[&player_id],
        )
        .await
        .map_err(|error| format!("Error pruning expired trusted devices: {error}"))?;

    conn
        .execute(
            "DELETE FROM trusted_devices WHERE id IN (SELECT id FROM trusted_devices WHERE player_id = $1 ORDER BY created_at DESC, id DESC OFFSET $2)",
            &[&player_id, &MAX_TRUSTED_DEVICES_PER_PLAYER],
        )
        .await
        .map_err(|error| format!("Error capping trusted devices: {error}"))?;

    Ok(trusted_device)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    // Initialize the ChaCha20Rng
    let rng = ChaCha20Rng::from_rng(OsRng).unwrap();
    let shared_rng = Arc::new(Mutex::new(rng));

    // set up connection pool
    let manager = PostgresConnectionManager::new_from_stringlike(
        format!(
            "host={} user={} password={} dbname={}",
            env::var("DB_HOST").unwrap(),
            env::var("DB_USER").unwrap(),
            env::var("DB_PASSWORD").unwrap(),
            env::var("DB_NAME").unwrap()
        ),
        NoTls,
    )
    .unwrap();

    let pool = Pool::builder().build(manager).await.unwrap();

    let ws_health_url = env::var("WS_HEALTH_URL").expect("WS_HEALTH_URL must be set");
    let ws_health_allow_invalid_certs = parse_env_bool("WS_HEALTH_ALLOW_INVALID_CERTS");

    let app = Router::new()
        .nest_service("/", ServeDir::new("root"))
        .route("/session", get(session_handler))
        .route("/auth", post(auth_handler))
        .route("/register", post(register_handler))
        .route(
            "/request-password-reset",
            post(request_password_reset_handler),
        )
        .route("/reset-password", post(reset_password_handler))
        .route("/device-auth", post(device_auth_handler))
        .route("/logout", post(logout_handler))
        .route("/scores", get(scores_handler))
        .route("/health", get(health_handler))
        .route("/set-display-name", post(set_display_name_handler))
        .with_state(AppState {
            pool,
            rng: shared_rng,
            ws_health_url,
            ws_health_allow_invalid_certs,
            rate_limiter: Arc::new(Mutex::new(HashMap::new())),
        })
        .layer(middleware::from_fn(cache_control_middleware))
        .layer(CompressionLayer::new().br(true).gzip(true));

    // configure certificate and private key used by https
    let config = RustlsConfig::from_pem_file(
        PathBuf::from(&env::var("CERT_PATH").unwrap()).join(&env::var("CERT_NAME").unwrap()),
        PathBuf::from(&env::var("CERT_PATH").unwrap()).join(&env::var("KEY_NAME").unwrap()),
    )
    .await
    .unwrap();

    // run https server
    let addr_str = env::var("ADDRESS").expect("ADDRESS must be set");
    let addr = addr_str
        .parse::<SocketAddr>()
        .expect("ADDRESS must be a valid IP address");
    tracing::debug!("listening on {}", addr);
    axum_server::bind_rustls(addr, config)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}

fn parse_env_bool(key: &str) -> bool {
    env::var(key)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

async fn cache_control_middleware(request: Request<Body>, next: Next) -> Response {
    let path = request.uri().path().to_string();
    let mut response = next.run(request).await;

    if is_html_path(&path) {
        let headers = response.headers_mut();
        headers.insert(
            CACHE_CONTROL,
            HeaderValue::from_static("no-store, no-cache, must-revalidate, max-age=0"),
        );
        headers.insert(PRAGMA, HeaderValue::from_static("no-cache"));
        headers.insert(EXPIRES, HeaderValue::from_static("0"));
    } else if is_revalidated_asset_path(&path) {
        let headers = response.headers_mut();
        headers.insert(
            CACHE_CONTROL,
            HeaderValue::from_static("no-cache, must-revalidate, max-age=0"),
        );
        headers.insert(PRAGMA, HeaderValue::from_static("no-cache"));
        headers.insert(EXPIRES, HeaderValue::from_static("0"));
    }

    response
}

fn is_html_path(path: &str) -> bool {
    path == "/" || path.ends_with(".html")
}

fn is_revalidated_asset_path(path: &str) -> bool {
    path.ends_with(".js") || path.ends_with(".css") || path == "/manifest.json"
}

async fn session_handler(State(state): State<AppState>, jar: CookieJar) -> Response {
    // Retrieve a specific cookie by name
    if let Some(cookie) = jar.get("session") {
        let conn = state
            .pool
            .get()
            .await
            .expect("Error getting connection from pool");

        let session_row = conn
            .query_one(
                "SELECT s.created_at, s.last_login, s.player_id, a.account_name, a.password, a.email, a.player_state FROM sessions s JOIN accounts a ON s.player_id = a.player_id WHERE s.session = $1",
                &[&cookie.value()],
            )
            .await;

        match session_row {
            Ok(session_row) => {
                let created_at: DateTime<Utc> = session_row.get::<_, DateTime<Utc>>("created_at");
                let last_login: Option<DateTime<Utc>> = session_row.get("last_login");

                if session_is_expired(created_at, last_login) {
                    println!("Session expired");
                    // Delete session from database
                    let _ = conn
                        .execute(
                            "DELETE FROM sessions WHERE session = $1",
                            &[&cookie.value()],
                        )
                        .await;
                    return (
                        StatusCode::UNAUTHORIZED,
                        Json(AuthError {
                            msg: "Session expired".to_string(),
                        }),
                    )
                        .into_response();
                } else {
                    // Slide the window forward so active players are never bounced.
                    let _ = conn
                        .execute(
                            "UPDATE sessions SET last_login = NOW() WHERE session = $1",
                            &[&cookie.value()],
                        )
                        .await;

                    let account_name: Option<String> = session_row.get("account_name");
                    let password: Option<String> = session_row.get("password");
                    let email: Option<String> = session_row.get("email");
                    let player_state: String = session_row.get("player_state");
                    let player_id: i32 = session_row.get("player_id");
                    let account_status = AccountStatus::from_password(password.as_deref());

                    return (
                        StatusCode::OK,
                        Json(SessionResponse {
                            player_id,
                            account_name,
                            account_status,
                            has_recovery_email: email.is_some(),
                            needs_hero: player_state == "CREATING_HERO",
                        }),
                    )
                        .into_response();
                }
            }
            Err(_) => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(AuthError {
                        msg: "Session not found".to_string(),
                    }),
                )
                    .into_response();
            }
        }
    } else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(AuthError {
                msg: "Session not found".to_string(),
            }),
        )
            .into_response();
    }
}

#[debug_handler]
async fn logout_handler(State(state): State<AppState>, jar: CookieJar) -> Response {
    let session = jar.get("session").map(|cookie| cookie.value().to_string());
    let trusted_device = jar
        .get(TRUSTED_DEVICE_COOKIE)
        .map(|cookie| cookie.value().to_string());

    if session.is_some() || trusted_device.is_some() {
        let conn = state
            .pool
            .get()
            .await
            .expect("Error getting connection from pool");

        if let Some(session) = session {
            let _ = conn
                .execute("DELETE FROM sessions WHERE session = $1", &[&session])
                .await;
        }
        if let Some(trusted_device) = trusted_device {
            let credential_hash = trusted_device_hash(&trusted_device);
            let _ = conn
                .execute(
                    "DELETE FROM trusted_devices WHERE credential_hash = $1",
                    &[&credential_hash],
                )
                .await;
        }
    }

    let mut headers = HeaderMap::new();
    append_cleared_auth_cookies(&mut headers);

    (
        StatusCode::OK,
        headers,
        Json(LogoutResponse { success: true }),
    )
        .into_response()
}

async fn health_handler(State(state): State<AppState>) -> Response {
    let health_check = timeout(
        HEALTH_CHECK_TIMEOUT,
        check_websocket_health(
            state.ws_health_url.clone(),
            state.ws_health_allow_invalid_certs,
        ),
    )
    .await;

    match health_check {
        Ok(Ok(())) => (
            StatusCode::OK,
            Json(HealthResponse {
                healthy: true,
                message: "websocket server responded with pong".to_string(),
            }),
        )
            .into_response(),
        Ok(Err(err)) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(HealthResponse {
                healthy: false,
                message: err,
            }),
        )
            .into_response(),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(HealthResponse {
                healthy: false,
                message: "health check timed out".to_string(),
            }),
        )
            .into_response(),
    }
}

async fn check_websocket_health(url: String, allow_invalid_certs: bool) -> Result<(), String> {
    let (mut ws_stream, _) = if allow_invalid_certs {
        let request = build_health_check_request(&url)?;
        let connector = TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .build()
            .map_err(|err| format!("failed to build insecure TLS connector: {err}"))?;

        connect_async_tls_with_config(request, None, false, Some(Connector::NativeTls(connector)))
            .await
            .map_err(|err| format!("failed to connect to websocket server: {err}"))?
    } else {
        let request = build_health_check_request(&url)?;
        connect_async(request)
            .await
            .map_err(|err| format!("failed to connect to websocket server: {err}"))?
    };

    ws_stream
        .send(Message::Text("ping".to_string()))
        .await
        .map_err(|err| format!("failed to send ping message: {err}"))?;

    while let Some(msg) = ws_stream
        .next()
        .await
        .transpose()
        .map_err(|err| format!("error receiving websocket message: {err}"))?
    {
        match msg {
            Message::Text(text) if text.trim().eq_ignore_ascii_case("pong") => return Ok(()),
            Message::Pong(_) => return Ok(()),
            Message::Close(_) => {
                return Err("websocket connection closed before receiving pong".to_string());
            }
            _ => continue,
        }
    }

    Err("no response received from websocket server".to_string())
}

fn build_health_check_request(url: &str) -> Result<Request<()>, String> {
    let mut request = url
        .into_client_request()
        .map_err(|err| format!("failed to build health check request: {err}"))?;

    request.headers_mut().insert(
        "x-health-check",
        HeaderValue::from_static(HEALTH_CHECK_HEADER_VALUE),
    );

    Ok(request)
}

#[debug_handler]
async fn auth_handler(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(payload): Json<AuthRequest>,
) -> Response {
    if !state.rate_limit_ok(client_ip(&headers, addr)).await {
        return too_many_requests();
    }
    let conn = state
        .pool
        .get()
        .await
        .expect("Error getting connection from pool");

    let found_account: bool;
    let password_match: bool;
    let mut player_id: i32 = 0;

    let account_name = payload.account_name.trim().to_string();
    let password = payload.password;

    if account_name.is_empty()
        || account_name.len() > 50
        || password.is_empty()
        || password.len() > 128
    {
        return (
            StatusCode::UNAUTHORIZED,
            Json(AuthError {
                msg: "Incorrect account or password".to_string(),
            }),
        )
            .into_response();
    }

    let row = conn
        .query_one(
            "SELECT player_id, account_name, password, email, player_state FROM accounts WHERE LOWER(account_name) = LOWER($1)",
            &[&account_name],
        )
        .await;

    match row {
        Ok(row) => {
            found_account = true;
            player_id = row.get("player_id");

            let account_password: Option<String> = row.get("password");
            let Some(account_password) = account_password else {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(AuthError {
                        msg: "Incorrect account or password".to_string(),
                    }),
                )
                    .into_response();
            };

            let verify_password = Account::verify_password(&password, &account_password);

            match verify_password {
                Ok(_) => {
                    password_match = true;
                }
                Err(_) => {
                    // Return 401 Unauthorized
                    println!("Invalid password: {}", player_id);
                    return (
                        StatusCode::UNAUTHORIZED,
                        Json(AuthError {
                            msg: "Incorrect account or password".to_string(),
                        }),
                    )
                        .into_response();
                }
            }
        }
        Err(_) => {
            // Return 401 Unauthorized
            println!("Account not found: {}", player_id);
            return (
                StatusCode::UNAUTHORIZED,
                Json(AuthError {
                    msg: "Incorrect account or password".to_string(),
                }),
            )
                .into_response();
        }
    }

    let mut session: Option<String> = None;
    let mut store_new_session = false;

    if found_account && password_match {
        let session_row = conn
            .query_opt(
                "SELECT session FROM sessions WHERE player_id = $1",
                &[&player_id],
            )
            .await;

        match session_row {
            Ok(Some(_)) => {
                // Rotate on every password authentication to prevent an older
                // browser session from surviving a fresh credential boundary.
                let new_session = {
                    let mut rng = state.rng.lock().await;
                    rng.gen::<u128>().to_string()
                };
                if let Err(error) = conn
                    .execute(
                        "UPDATE sessions SET session = $1, created_at = NOW(), last_login = NOW() WHERE player_id = $2",
                        &[&new_session, &player_id],
                    )
                    .await
                {
                    println!("Error rotating authenticated session: {error}");
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(AuthError {
                            msg: "Unable to establish a session".to_string(),
                        }),
                    )
                        .into_response();
                }
                session = Some(new_session);
            }
            Ok(None) => {
                let mut rng = state.rng.lock().await;
                let session_num = rng.gen::<u128>().to_string();
                session = Some(session_num);
                store_new_session = true;
            }
            Err(error) => {
                println!("Error loading authenticated session: {error}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(AuthError {
                        msg: "Unable to establish a session".to_string(),
                    }),
                )
                    .into_response();
            }
        }
    }

    if store_new_session {
        println!("Storing new session in database");
        // store new session in database and return error if it fails
        let result = conn.execute(
            "INSERT INTO sessions (player_id, session, created_at, last_login) VALUES ($1, $2, NOW(), NOW())",
            &[&player_id, &session],
        )
        .await;

        if result.is_err() {
            // Print error
            println!("Error: {}", result.err().unwrap());
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthError {
                    msg: "Unknown error".to_string(),
                }),
            )
                .into_response();
        }
    }

    if let Some(session) = session {
        let trusted_device = match issue_trusted_device(&conn, player_id).await {
            Ok(credential) => credential,
            Err(error) => {
                println!("{error}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(AuthError {
                        msg: "Unable to establish a trusted device".to_string(),
                    }),
                )
                    .into_response();
            }
        };

        let account_row = match conn
            .query_one(
                "SELECT account_name, email, player_state FROM accounts WHERE player_id = $1",
                &[&player_id],
            )
            .await
        {
            Ok(row) => row,
            Err(error) => {
                println!("Error loading authenticated account state: {error}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(AuthError {
                        msg: "Unable to load account state".to_string(),
                    }),
                )
                    .into_response();
            }
        };
        let canonical_account_name: Option<String> = account_row.get("account_name");
        let email: Option<String> = account_row.get("email");
        let player_state: String = account_row.get("player_state");

        let mut headers = HeaderMap::new();
        append_session_cookie(&mut headers, &session);
        append_trusted_device_cookie(&mut headers, &trusted_device);

        println!("Successfully authenticated: {}", player_id);
        (
            StatusCode::OK,
            headers,
            Json(AuthResponse {
                player_id,
                account_status: AccountStatus::Secured,
                has_recovery_email: email.is_some(),
                needs_hero: player_state == "CREATING_HERO",
                account_name: canonical_account_name,
            }),
        )
            .into_response()
    } else {
        println!("Session invalid: {}", player_id);
        (
            StatusCode::UNAUTHORIZED,
            Json(AuthError {
                msg: "Session invalid".to_string(),
            }),
        )
            .into_response()
    }
}

#[debug_handler]
async fn device_auth_handler(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(payload): Json<DeviceAuthRequest>,
) -> Response {
    if !state.rate_limit_ok(client_ip(&headers, addr)).await {
        return too_many_requests();
    }
    let mut conn = state
        .pool
        .get()
        .await
        .expect("Error getting connection from pool");
    let transaction = match conn.transaction().await {
        Ok(transaction) => transaction,
        Err(error) => {
            println!("Error starting trusted-device transaction: {error}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthError {
                    msg: "Unable to authenticate this device".to_string(),
                }),
            )
                .into_response();
        }
    };

    // The trusted-device cookie is the only passwordless credential. It is
    // server-generated, HttpOnly, rotated whenever it restores a session, and
    // never exposed to JavaScript or accepted in a request body.
    let mut resolved_player: Option<ResolvedAccount> = None;
    let mut new_player = false;
    let mut trusted_device_to_set: Option<String> = None;

    if let Some(trusted_device) = jar
        .get(TRUSTED_DEVICE_COOKIE)
        .map(|cookie| cookie.value().to_string())
    {
        let credential_hash = trusted_device_hash(&trusted_device);
        match transaction
            .query_opt(
                "SELECT a.player_id, a.account_name, a.password, a.email, a.player_state FROM trusted_devices d JOIN accounts a ON a.player_id = d.player_id WHERE d.credential_hash = $1 AND d.created_at > NOW() - INTERVAL '90 days' FOR UPDATE OF d",
                &[&credential_hash],
            )
            .await
        {
            Ok(Some(row)) => {
                let rotated_token = Uuid::new_v4().to_string();
                let rotated_hash = trusted_device_hash(&rotated_token);
                let rotated = transaction
                    .execute(
                        "UPDATE trusted_devices SET credential_hash = $1, created_at = NOW() WHERE credential_hash = $2",
                        &[&rotated_hash, &credential_hash],
                    )
                    .await;
                match rotated {
                    Ok(1) => {
                        trusted_device_to_set = Some(rotated_token);
                    }
                    Ok(_) => {
                        let mut response = (
                            StatusCode::UNAUTHORIZED,
                            Json(AuthError {
                                msg: "authentication_required".to_string(),
                            }),
                        )
                            .into_response();
                        append_cleared_trusted_device_cookie(response.headers_mut());
                        return response;
                    }
                    Err(error) => {
                        println!("Error rotating trusted-device credential: {error}");
                        return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(AuthError {
                            msg: "Unable to restore this device".to_string(),
                        }),
                    )
                        .into_response();
                    }
                }
                resolved_player = Some(ResolvedAccount {
                    player_id: row.get("player_id"),
                    account_name: row.get("account_name"),
                    password: row.get("password"),
                    email: row.get("email"),
                    player_state: row.get("player_state"),
                });
            }
            Ok(None) => {
                let _ = transaction
                    .execute(
                        "DELETE FROM trusted_devices WHERE credential_hash = $1",
                        &[&credential_hash],
                    )
                    .await;
            }
            Err(error) => {
                println!("Error looking up trusted-device credential: {error}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(AuthError {
                        msg: "Unable to restore this device".to_string(),
                    }),
                )
                    .into_response();
            }
        }
    }

    if resolved_player.is_none() {
        if !payload.create_guest {
            let mut response = (
                StatusCode::UNAUTHORIZED,
                Json(AuthError {
                    msg: "authentication_required".to_string(),
                }),
            )
                .into_response();
            append_cleared_trusted_device_cookie(response.headers_mut());
            return response;
        }

        let no_name: Option<String> = None;
        let no_password: Option<String> = None;
        let initial_trusted_device = Uuid::new_v4().to_string();
        let initial_trusted_device_hash = trusted_device_hash(&initial_trusted_device);
        let initial_session = {
            let mut rng = state.rng.lock().await;
            rng.gen::<u128>().to_string()
        };
        let result = transaction
            .query_one(
                "INSERT INTO accounts (account_name, password, email, created_at) VALUES ($1, $2, $3, current_timestamp) RETURNING player_id",
                &[&no_name, &no_password, &no_name],
            )
            .await;

        match result {
            Ok(row) => {
                let player_id: i32 = row.get("player_id");
                let account_name = format!("account{}", player_id);

                if let Err(error) = transaction
                    .execute(
                        "UPDATE accounts SET account_name = $1 WHERE player_id = $2",
                        &[&account_name, &player_id],
                    )
                    .await
                {
                    println!("Error setting guest account name: {error}");
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(AuthError {
                            msg: "Failed to create guest account".to_string(),
                        }),
                    )
                        .into_response();
                }
                if let Err(error) = transaction
                    .execute(
                        "INSERT INTO sessions (player_id, session, created_at, last_login) VALUES ($1, $2, NOW(), NOW())",
                        &[&player_id, &initial_session],
                    )
                    .await
                {
                    println!("Error creating guest session: {error}");
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(AuthError {
                            msg: "Failed to create guest account".to_string(),
                        }),
                    )
                        .into_response();
                }
                if let Err(error) = transaction
                    .execute(
                        "INSERT INTO trusted_devices (player_id, credential_hash, created_at) VALUES ($1, $2, NOW())",
                        &[&player_id, &initial_trusted_device_hash],
                    )
                    .await
                {
                    println!("Error creating trusted-device credential: {error}");
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(AuthError {
                            msg: "Failed to create guest account".to_string(),
                        }),
                    )
                        .into_response();
                }
                println!("Created guest player {}", player_id);
                new_player = true;
                trusted_device_to_set = Some(initial_trusted_device);
                resolved_player = Some(ResolvedAccount {
                    player_id,
                    account_name: Some(account_name),
                    password: None,
                    email: None,
                    player_state: "CREATING_HERO".to_string(),
                });
            }
            Err(error) => {
                println!("Error creating guest account: {error}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(AuthError {
                        msg: "Failed to create guest account".to_string(),
                    }),
                )
                    .into_response();
            }
        }
    }

    let ResolvedAccount {
        player_id,
        account_name,
        password,
        email,
        player_state,
    } = resolved_player.unwrap();
    let account_status = AccountStatus::from_password(password.as_deref());
    let has_account = account_status.is_secured();

    let Some(trusted_device) = trusted_device_to_set else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthError {
                msg: "Unable to establish a trusted device".to_string(),
            }),
        )
            .into_response();
    };

    // Restore or create the browser session associated with this device.
    let session_row = transaction
        .query_opt(
            "SELECT session FROM sessions WHERE player_id = $1",
            &[&player_id],
        )
        .await;

    let (session, store_new) = match session_row {
        Ok(Some(row)) => (row.get("session"), false),
        Ok(None) => {
            let mut rng = state.rng.lock().await;
            let session_num = rng.gen::<u128>().to_string();
            (session_num, true)
        }
        Err(error) => {
            println!("Error checking session: {error}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthError {
                    msg: "Unable to establish a session".to_string(),
                }),
            )
                .into_response();
        }
    };

    if store_new {
        let result = transaction
            .execute(
                "INSERT INTO sessions (player_id, session, created_at) VALUES ($1, $2, current_timestamp)",
                &[&player_id, &session],
            )
            .await;

        if let Err(e) = result {
            println!("Error storing session: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthError {
                    msg: "Unknown error".to_string(),
                }),
            )
                .into_response();
        }
    }
    if let Err(error) = transaction
        .execute(
            "UPDATE sessions SET last_login = NOW() WHERE player_id = $1 AND session = $2",
            &[&player_id, &session],
        )
        .await
    {
        println!("Error refreshing session activity: {error}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthError {
                msg: "Unable to refresh the session".to_string(),
            }),
        )
            .into_response();
    }

    if let Err(error) = transaction.commit().await {
        println!("Error committing trusted-device authentication: {error}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthError {
                msg: "Unable to authenticate this device".to_string(),
            }),
        )
            .into_response();
    }

    let mut headers = HeaderMap::new();
    append_session_cookie(&mut headers, &session);
    append_trusted_device_cookie(&mut headers, &trusted_device);

    println!(
        "Trusted-device auth successful: player_id={}, has_account={}, new_player={}",
        player_id, has_account, new_player
    );
    (
        StatusCode::OK,
        headers,
        Json(DeviceAuthResponse {
            player_id,
            has_account,
            new_player,
            account_status,
            has_recovery_email: email.is_some(),
            needs_hero: player_state == "CREATING_HERO",
            account_name,
        }),
    )
        .into_response()
}

#[debug_handler]
async fn register_handler(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(payload): Json<RegisterRequest>,
) -> Response {
    if !state.rate_limit_ok(client_ip(&headers, addr)).await {
        return too_many_requests();
    }
    let Some(cookie) = jar.get("session") else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(AuthError {
                msg: "Session not found".to_string(),
            }),
        )
            .into_response();
    };

    let conn = state
        .pool
        .get()
        .await
        .expect("Error getting connection from pool");

    // Look up player_id from a still-valid current session. A stale cookie is
    // not sufficient authority to claim or replace account credentials.
    let session_row = conn
        .query_one(
            "SELECT s.player_id, s.created_at, s.last_login, a.password, a.player_state FROM sessions s JOIN accounts a ON a.player_id = s.player_id WHERE s.session = $1",
            &[&cookie.value()],
        )
        .await;

    let Ok(session_row) = session_row else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(AuthError {
                msg: "Session not found".to_string(),
            }),
        )
            .into_response();
    };

    let player_id = session_row.get::<_, i32>("player_id");
    let created_at: DateTime<Utc> = session_row.get("created_at");
    let last_login: Option<DateTime<Utc>> = session_row.get("last_login");
    if session_is_expired(created_at, last_login) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(AuthError {
                msg: "Session expired".to_string(),
            }),
        )
            .into_response();
    }
    let existing_password: Option<String> = session_row.get("password");
    if existing_password.is_some() {
        return (
            StatusCode::CONFLICT,
            Json(AuthError {
                msg: "This account is already secured".to_string(),
            }),
        )
            .into_response();
    }

    let account_name = payload.account_name.trim().to_string();
    let password = payload.password;

    if !valid_account_name(&account_name) {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                msg: "Account name must be 3-20 letters, numbers, or underscores".to_string(),
            }),
        )
            .into_response();
    }
    if !valid_new_password(&password) {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                msg: "Password must be 8-128 characters".to_string(),
            }),
        )
            .into_response();
    }

    // Email is optional; normalize and drop blanks. It is the only way to
    // recover a forgotten password, so we store it when provided.
    let email = payload
        .email
        .map(|e| e.trim().to_lowercase())
        .filter(|e| !e.is_empty());

    if let Some(ref email) = email {
        if !valid_email(email) {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthError {
                    msg: "Please enter a valid email address".to_string(),
                }),
            )
                .into_response();
        }
    }

    let identity_conflict = conn
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM accounts WHERE player_id <> $1 AND (LOWER(account_name) = LOWER($2) OR ($3::TEXT IS NOT NULL AND LOWER(email) = LOWER($3))))",
            &[&player_id, &account_name, &email],
        )
        .await;
    match identity_conflict {
        Ok(row) if row.get::<_, bool>(0) => {
            return (
                StatusCode::CONFLICT,
                Json(AuthError {
                    msg: "That account name or email is already in use".to_string(),
                }),
            )
                .into_response();
        }
        Ok(_) => {}
        Err(error) => {
            println!("Error checking account identity uniqueness: {error}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthError {
                    msg: "Unable to secure the account".to_string(),
                }),
            )
                .into_response();
        }
    }

    let account = match Account::new(account_name.clone(), password) {
        Ok(account) => account,
        Err(error) => {
            println!("Error hashing registration password: {error}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthError {
                    msg: "Unable to secure the account".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Update the existing account row for this player_id (atomically including
    // the recovery email when one was provided).
    let has_recovery_email = email.is_some();
    let result = match &email {
        Some(email) => {
            conn.execute(
                "UPDATE accounts SET account_name = $1, password = $2, email = $3 WHERE player_id = $4",
                &[&account.account_name, &account.password, email, &player_id],
            )
            .await
        }
        None => {
            conn.execute(
                "UPDATE accounts SET account_name = $1, password = $2 WHERE player_id = $3",
                &[&account.account_name, &account.password, &player_id],
            )
            .await
        }
    };

    if let Err(e) = result {
        if e.code() == Some(&SqlState::UNIQUE_VIOLATION) {
            return (
                StatusCode::CONFLICT,
                Json(AuthError {
                    msg: "That account name or email is already in use".to_string(),
                }),
            )
                .into_response();
        }
        println!("Error registering player {}: {}", player_id, e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthError {
                msg: "Unknown error".to_string(),
            }),
        )
            .into_response();
    }

    let trusted_device = match issue_trusted_device(&conn, player_id).await {
        Ok(credential) => credential,
        Err(error) => {
            println!("{error}");
            String::new()
        }
    };
    let player_state: String = session_row.get("player_state");

    println!("Successfully registered: {}", player_id);
    let mut response = (
        StatusCode::OK,
        Json(AuthResponse {
            player_id,
            account_status: AccountStatus::Secured,
            has_recovery_email,
            needs_hero: player_state == "CREATING_HERO",
            account_name: Some(account_name),
        }),
    )
        .into_response();
    if !trusted_device.is_empty() {
        append_trusted_device_cookie(response.headers_mut(), &trusted_device);
    }
    response
}

// How long a password-reset link stays valid.
const PASSWORD_RESET_TTL_MINUTES: i64 = 60;

// Generic reply for the request-reset endpoint. Returned in every case so the
// endpoint never reveals whether an account or email exists (anti-enumeration).
fn reset_requested_response() -> Response {
    (
        StatusCode::OK,
        Json(MessageResponse {
            message: "If an account with a recovery email exists, a reset link has been sent."
                .to_string(),
        }),
    )
        .into_response()
}

#[debug_handler]
async fn request_password_reset_handler(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(payload): Json<PasswordResetRequest>,
) -> Response {
    if !state.rate_limit_ok(client_ip(&headers, addr)).await {
        return too_many_requests();
    }
    let identifier = payload.identifier.trim().to_lowercase();
    if identifier.is_empty() {
        return reset_requested_response();
    }

    let conn = state
        .pool
        .get()
        .await
        .expect("Error getting connection from pool");

    // Match on either account name or email, case-insensitively.
    let row = conn
        .query_opt(
            "SELECT player_id, email FROM accounts WHERE LOWER(account_name) = $1 OR LOWER(email) = $1",
            &[&identifier],
        )
        .await;

    if let Ok(Some(row)) = row {
        let player_id: i32 = row.get("player_id");
        let email: Option<String> = row.get("email");

        // Recovery is only possible when the account has an email on file.
        if let Some(email) = email {
            let token = Uuid::new_v4().simple().to_string();

            let _ = conn
                .execute(
                    "UPDATE password_resets SET used_at = NOW() WHERE player_id = $1 AND used_at IS NULL",
                    &[&player_id],
                )
                .await;
            if let Err(e) = conn
                .execute(
                    "INSERT INTO password_resets (token, player_id, created_at) VALUES ($1, $2, NOW())",
                    &[&token, &player_id],
                )
                .await
            {
                println!("Error storing password reset token: {}", e);
                return reset_requested_response();
            }

            let base = env::var("PUBLIC_BASE_URL")
                .unwrap_or_else(|_| "https://surviveperilous.com".to_string());
            let reset_url = format!("{}/?reset={}", base.trim_end_matches('/'), token);

            send_password_reset_email(&email, &reset_url).await;
        }
    }

    reset_requested_response()
}

#[debug_handler]
async fn reset_password_handler(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(payload): Json<ResetPasswordRequest>,
) -> Response {
    if !state.rate_limit_ok(client_ip(&headers, addr)).await {
        return too_many_requests();
    }
    let token = payload.token.trim().to_string();
    let password = payload.password;

    if !valid_new_password(&password) {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                msg: "Password must be 8-128 characters".to_string(),
            }),
        )
            .into_response();
    }

    let mut conn = state
        .pool
        .get()
        .await
        .expect("Error getting connection from pool");

    let transaction = match conn.transaction().await {
        Ok(transaction) => transaction,
        Err(error) => {
            println!("Error starting password reset transaction: {error}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthError {
                    msg: "Unable to reset password".to_string(),
                }),
            )
                .into_response();
        }
    };

    let row = transaction
        .query_opt(
            "SELECT player_id, created_at, used_at FROM password_resets WHERE token = $1 FOR UPDATE",
            &[&token],
        )
        .await;

    let Ok(Some(row)) = row else {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                msg: "This reset link is invalid or has expired".to_string(),
            }),
        )
            .into_response();
    };

    let used_at: Option<DateTime<Utc>> = row.get("used_at");
    if used_at.is_some() {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                msg: "This reset link has already been used".to_string(),
            }),
        )
            .into_response();
    }

    let created_at: DateTime<Utc> = row.get("created_at");
    if Utc::now().signed_duration_since(created_at).num_minutes() > PASSWORD_RESET_TTL_MINUTES {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                msg: "This reset link is invalid or has expired".to_string(),
            }),
        )
            .into_response();
    }

    let player_id: i32 = row.get("player_id");
    let password_hash = match Account::hash_password(&password) {
        Ok(password_hash) => password_hash,
        Err(error) => {
            println!("Error hashing reset password: {error}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthError {
                    msg: "Unable to reset password".to_string(),
                }),
            )
                .into_response();
        }
    };

    if let Err(e) = transaction
        .execute(
            "UPDATE accounts SET password = $1 WHERE player_id = $2",
            &[&password_hash, &player_id],
        )
        .await
    {
        println!("Error updating password for player {}: {}", player_id, e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthError {
                msg: "Unknown error".to_string(),
            }),
        )
            .into_response();
    }

    // Burn every outstanding reset token and revoke every established browser
    // credential. A password reset is an account-recovery boundary, so a
    // previously stolen session or device token must not remain authoritative.
    if let Err(error) = transaction
        .execute(
            "UPDATE password_resets SET used_at = NOW() WHERE player_id = $1 AND used_at IS NULL",
            &[&player_id],
        )
        .await
    {
        println!("Error invalidating password reset tokens: {error}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthError {
                msg: "Unable to reset password".to_string(),
            }),
        )
            .into_response();
    }
    if let Err(error) = transaction
        .execute("DELETE FROM sessions WHERE player_id = $1", &[&player_id])
        .await
    {
        println!("Error revoking sessions after password reset: {error}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthError {
                msg: "Unable to reset password".to_string(),
            }),
        )
            .into_response();
    }
    if let Err(error) = transaction
        .execute(
            "DELETE FROM trusted_devices WHERE player_id = $1",
            &[&player_id],
        )
        .await
    {
        println!("Error revoking devices after password reset: {error}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthError {
                msg: "Unable to reset password".to_string(),
            }),
        )
            .into_response();
    }
    if let Err(error) = transaction.commit().await {
        println!("Error committing password reset: {error}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthError {
                msg: "Unable to reset password".to_string(),
            }),
        )
            .into_response();
    }

    println!("Password reset for player {}", player_id);
    let mut response = (
        StatusCode::OK,
        Json(MessageResponse {
            message: "Your password has been updated. Please log in.".to_string(),
        }),
    )
        .into_response();
    append_cleared_auth_cookies(response.headers_mut());
    response
}

async fn send_password_reset_email(to: &str, reset_url: &str) {
    let subject = "Reset your Survive Perilous password";
    let html = format!(
        "<h1>Password reset</h1>\
         <p>We received a request to reset your Survive Perilous password. \
         Click the link below to choose a new one. This link expires in {} minutes.</p>\
         <p><a href=\"{}\">Reset your password</a></p>\
         <p>If you didn't request this, you can safely ignore this email.</p>",
        PASSWORD_RESET_TTL_MINUTES, reset_url
    );
    let text = format!(
        "Reset your Survive Perilous password\n\n\
         We received a request to reset your password. Open the link below to \
         choose a new one (expires in {} minutes):\n{}\n\n\
         If you didn't request this, you can safely ignore this email.",
        PASSWORD_RESET_TTL_MINUTES, reset_url
    );

    send_email(to, subject, &html, &text).await;
}

// Sends transactional email via Cloudflare's email-sending API. When
// CLOUDFLARE_EMAIL_TOKEN is unset (e.g. local dev) it logs the message instead
// of sending, so the reset flow stays fully testable without credentials.
async fn send_email(to: &str, subject: &str, html: &str, text: &str) {
    let token = match env::var("CLOUDFLARE_EMAIL_TOKEN") {
        Ok(t) if !t.is_empty() => t,
        _ => {
            println!(
                "[email] CLOUDFLARE_EMAIL_TOKEN not set; would send to {}:\n{}",
                to, text
            );
            return;
        }
    };

    let account_id = env::var("CLOUDFLARE_ACCOUNT_ID")
        .unwrap_or_else(|_| "514db0d52efacfc2b7d0c685d7b57cf6".to_string());
    let from = env::var("EMAIL_FROM_ADDRESS")
        .unwrap_or_else(|_| "welcome@surviveperilous.com".to_string());

    let url = format!(
        "https://api.cloudflare.com/client/v4/accounts/{}/email/sending/send",
        account_id
    );

    let client = reqwest::Client::new();
    let result = client
        .post(&url)
        .bearer_auth(token)
        .json(&CloudflareEmail {
            to,
            from: &from,
            subject,
            html,
            text,
        })
        .send()
        .await;

    match result {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() {
                println!("[email] Sent to {}", to);
            } else {
                let body = resp.text().await.unwrap_or_default();
                println!("[email] Cloudflare send failed ({}): {}", status, body);
            }
        }
        Err(e) => println!("[email] Error sending to {}: {}", to, e),
    }
}

#[derive(Deserialize)]
struct SetDisplayNameRequest {
    hero_name: String,
}

#[derive(Serialize)]
struct SetDisplayNameResponse {
    account_name: String,
}

#[debug_handler]
async fn set_display_name_handler(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(payload): Json<SetDisplayNameRequest>,
) -> Response {
    let Some(cookie) = jar.get("session") else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(AuthError {
                msg: "Session not found".to_string(),
            }),
        )
            .into_response();
    };

    let conn = state
        .pool
        .get()
        .await
        .expect("Error getting connection from pool");

    // Get player_id from a still-valid session.
    let session_row = conn
        .query_one(
            "SELECT player_id, created_at, last_login FROM sessions WHERE session = $1",
            &[&cookie.value()],
        )
        .await;

    let Ok(session_row) = session_row else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(AuthError {
                msg: "Session not found".to_string(),
            }),
        )
            .into_response();
    };

    let player_id: i32 = session_row.get("player_id");
    let created_at: DateTime<Utc> = session_row.get("created_at");
    let last_login: Option<DateTime<Utc>> = session_row.get("last_login");
    if session_is_expired(created_at, last_login) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(AuthError {
                msg: "Session expired".to_string(),
            }),
        )
            .into_response();
    }

    // Only update if account has no password (guest account)
    let account_row = conn
        .query_one(
            "SELECT password FROM accounts WHERE player_id = $1",
            &[&player_id],
        )
        .await;

    let Ok(account_row) = account_row else {
        return (
            StatusCode::NOT_FOUND,
            Json(AuthError {
                msg: "Account not found".to_string(),
            }),
        )
            .into_response();
    };

    let password: Option<String> = account_row.get("password");
    if password.is_some() {
        // Already has a real account, don't overwrite the name
        return StatusCode::OK.into_response();
    }

    // Sanitize hero name: keep only alphanumeric chars
    let sanitized: String = payload
        .hero_name
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect();
    let hero_name = if sanitized.is_empty() {
        "Player".to_string()
    } else {
        sanitized
    };

    // Generate random number 1-1000
    let random_num: u32 = {
        let mut rng = state.rng.lock().await;
        rng.gen_range(1..=1000)
    };

    let display_name = format!("{}{}", hero_name, random_num);

    let _ = conn
        .execute(
            "UPDATE accounts SET account_name = $1, hero_name = $2 WHERE player_id = $3",
            &[&display_name, &hero_name, &player_id],
        )
        .await;

    (
        StatusCode::OK,
        Json(SetDisplayNameResponse {
            account_name: display_name,
        }),
    )
        .into_response()
}

#[debug_handler]
async fn scores_handler(State(state): State<AppState>) -> Response {
    let conn = state
        .pool
        .get()
        .await
        .expect("Error getting connection from pool");

    let score_migrations = [
        "ALTER TABLE scores ADD COLUMN IF NOT EXISTS total_score INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE scores ADD COLUMN IF NOT EXISTS score_survival INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE scores ADD COLUMN IF NOT EXISTS score_progression INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE scores ADD COLUMN IF NOT EXISTS score_wealth INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE scores ADD COLUMN IF NOT EXISTS score_defense INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE scores ADD COLUMN IF NOT EXISTS score_valor INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE scores ADD COLUMN IF NOT EXISTS score_legacy INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE scores ADD COLUMN IF NOT EXISTS days_survived INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE scores ADD COLUMN IF NOT EXISTS highest_pressure_level INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE scores ADD COLUMN IF NOT EXISTS waves_survived INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE scores ADD COLUMN IF NOT EXISTS legendary_kills INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE scores ADD COLUMN IF NOT EXISTS hideouts_cleared INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE scores ADD COLUMN IF NOT EXISTS crisis_tier INTEGER NOT NULL DEFAULT 0",
        "UPDATE scores SET total_score = total_xp WHERE total_score = 0",
    ];

    for statement in score_migrations {
        if let Err(err) = conn.execute(statement, &[]).await {
            println!("Score schema migration failed: {:?}", err);
        }
    }

    let rows = conn
        .query(
            "SELECT id, hero_name, hero_rank, total_xp, COALESCE(total_score, total_xp) as total_score, COALESCE(score_survival, 0) as score_survival, COALESCE(score_progression, 0) as score_progression, COALESCE(score_wealth, 0) as score_wealth, COALESCE(score_defense, 0) as score_defense, COALESCE(score_valor, 0) as score_valor, COALESCE(score_legacy, 0) as score_legacy, COALESCE(days_survived, 0) as days_survived, COALESCE(highest_pressure_level, 0) as highest_pressure_level, COALESCE(waves_survived, 0) as waves_survived, COALESCE(legendary_kills, 0) as legendary_kills, COALESCE(hideouts_cleared, 0) as hideouts_cleared, fate, COALESCE(crisis_tier, 0) as crisis_tier FROM scores ORDER BY COALESCE(total_score, total_xp) DESC",
            &[],
        )
        .await;

    let Ok(rows) = rows else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthError {
                msg: "Unknown error".to_string(),
            }),
        )
            .into_response();
    };

    let scores = rows
        .iter()
        .map(|row| Score {
            id: row.get::<_, i32>("id"),
            hero_name: row.get::<_, String>("hero_name"),
            hero_rank: row.get::<_, String>("hero_rank"),
            total_xp: row.get::<_, i32>("total_xp"),
            total_score: row.get::<_, i32>("total_score"),
            score_survival: row.get::<_, i32>("score_survival"),
            score_progression: row.get::<_, i32>("score_progression"),
            score_wealth: row.get::<_, i32>("score_wealth"),
            score_defense: row.get::<_, i32>("score_defense"),
            score_valor: row.get::<_, i32>("score_valor"),
            score_legacy: row.get::<_, i32>("score_legacy"),
            days_survived: row.get::<_, i32>("days_survived"),
            highest_pressure_level: row.get::<_, i32>("highest_pressure_level"),
            waves_survived: row.get::<_, i32>("waves_survived"),
            legendary_kills: row.get::<_, i32>("legendary_kills"),
            hideouts_cleared: row.get::<_, i32>("hideouts_cleared"),
            fate: row.get::<_, String>("fate"),
            crisis_tier: row.get::<_, i32>("crisis_tier"),
        })
        .collect::<Vec<Score>>();

    println!("Successfully registered: {:?}", scores);
    (StatusCode::OK, Json(ScoreResponse(scores))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower::ServiceExt;

    #[test]
    fn account_status_depends_on_credentials_not_generated_name() {
        assert_eq!(AccountStatus::from_password(None), AccountStatus::Guest);
        assert_eq!(
            AccountStatus::from_password(Some("argon2-hash")),
            AccountStatus::Secured
        );
    }

    #[test]
    fn account_name_validation_matches_registration_contract() {
        assert!(valid_account_name("Player_1"));
        assert!(!valid_account_name("ab"));
        assert!(!valid_account_name("player-name"));
        assert!(!valid_account_name("this_account_name_is_too_long"));
    }

    #[test]
    fn password_validation_enforces_bounded_length() {
        assert!(!valid_new_password("short"));
        assert!(valid_new_password("eight888"));
        assert!(!valid_new_password(&"x".repeat(129)));
    }

    #[test]
    fn email_validation_rejects_missing_or_ambiguous_domains() {
        assert!(valid_email("hero@example.com"));
        assert!(!valid_email("hero@example"));
        assert!(!valid_email("hero@example@invalid.com"));
        assert!(!valid_email("hero @example.com"));
    }

    #[test]
    fn session_expiration_uses_last_activity_when_present() {
        let old_creation = Utc::now() - chrono::Duration::days(8);
        assert!(session_is_expired(old_creation, None));
        assert!(!session_is_expired(
            old_creation,
            Some(Utc::now() - chrono::Duration::days(1))
        ));
    }

    #[test]
    fn trusted_device_cookie_is_http_only_secure_and_strict() {
        let mut headers = HeaderMap::new();
        append_trusted_device_cookie(&mut headers, "server-generated-secret");

        let cookie = headers
            .get("Set-Cookie")
            .and_then(|value| value.to_str().ok())
            .expect("trusted-device Set-Cookie header");
        assert!(cookie.starts_with("__Host-trusted_device=server-generated-secret;"));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("Secure"));
        assert!(cookie.contains("SameSite=Strict"));
        assert!(cookie.contains("Max-Age=7776000"));
    }

    #[test]
    fn trusted_device_database_value_is_a_one_way_digest() {
        let credential = "server-generated-secret";
        let digest = trusted_device_hash(credential);

        assert_ne!(digest, credential);
        assert_eq!(digest.len(), 64);
        assert_eq!(digest, trusted_device_hash(credential));
    }

    #[test]
    fn app_shell_and_executable_assets_are_always_revalidated() {
        assert!(is_html_path("/"));
        assert!(is_html_path("/index.html"));
        assert!(is_revalidated_asset_path("/sp2.mobile.js"));
        assert!(is_revalidated_asset_path("/service-worker.js"));
        assert!(is_revalidated_asset_path("/manifest.json"));
        assert!(!is_revalidated_asset_path("/static/art/stockade.png"));
    }

    #[tokio::test]
    async fn compression_layer_negotiates_brotli_for_large_text_responses() {
        let app = Router::new()
            .route("/asset.js", get(|| async { "x".repeat(4096) }))
            .layer(CompressionLayer::new().br(true).gzip(true));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/asset.js")
                    .header("Accept-Encoding", "br, gzip")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response
                .headers()
                .get("Content-Encoding")
                .and_then(|value| value.to_str().ok()),
            Some("br")
        );
    }
}
