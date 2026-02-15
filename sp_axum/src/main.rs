use account::Account;
use axum::debug_handler;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::HeaderValue;
use axum::http::Request;
use axum::http::StatusCode;
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
use std::env;
use std::sync::Arc;
use std::{net::SocketAddr, path::PathBuf};
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};
use tokio_postgres::NoTls;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::connect_async_tls_with_config;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::Connector;
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
    player_id: i32,
    device_token: String,
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
    fate: String,
}

#[derive(Deserialize)]
struct FingerprintAuthRequest {
    fingerprint: String,
    device_token: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FingerprintAuthResponse {
    player_id: i32,
    has_account: bool,
    new_player: bool,
    #[serde(rename = "account_name")]
    account_name: Option<String>,
    #[serde(rename = "device_token")]
    device_token: String,
}

#[derive(Serialize)]
struct SessionResponse {
    account_name: Option<String>,
    device_token: String,
}

#[derive(Serialize)]
struct AuthError {
    msg: String,
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
        .route("/fingerprint-auth", post(fingerprint_auth_handler))
        .route("/logout", post(logout_handler))
        .route("/scores", get(scores_handler))
        .route("/health", get(health_handler))
        .with_state(AppState {
            pool,
            rng: shared_rng,
            ws_health_url,
            ws_health_allow_invalid_certs,
        });

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
        .serve(app.into_make_service())
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

async fn session_handler(State(state): State<AppState>, jar: CookieJar) -> Response {
    // Retrieve a specific cookie by name
    if let Some(cookie) = jar.get("session") {
        println!("Session cookie found: {}", cookie.value());

        let conn = state
            .pool
            .get()
            .await
            .expect("Error getting connection from pool");

        let session_row = conn
            .query_one(
                "SELECT s.created_at, s.player_id, a.account_name FROM sessions s JOIN accounts a ON s.player_id = a.player_id WHERE s.session = $1",
                &[&cookie.value()],
            )
            .await;

        match session_row {
            Ok(session_row) => {
                // Check if created_at is within the last 10 minutes

                let created_at: DateTime<Utc> = session_row.get::<_, DateTime<Utc>>("created_at");
                println!("created_at: {}", created_at);

                let now = Utc::now(); // Already a DateTime<Utc>
                println!("now: {}", now);

                let diff = now.signed_duration_since(created_at);
                println!("diff: {}", diff);

                if diff.num_minutes() > 10 {
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
                    let account_name: Option<String> = session_row.get("account_name");
                    let player_id: i32 = session_row.get("player_id");
                    println!(
                        "Session found: {}, account_name: {:?}",
                        session_row.get::<_, DateTime<Utc>>("created_at"),
                        account_name
                    );

                    // Generate device token
                    let device_token = Uuid::new_v4().to_string();
                    if let Err(e) = conn
                        .execute(
                            "INSERT INTO device_tokens (player_id, token, created_at) VALUES ($1, $2, NOW())",
                            &[&player_id, &device_token],
                        )
                        .await
                    {
                        println!("Error storing device token: {}", e);
                    }

                    return (StatusCode::OK, Json(SessionResponse { account_name, device_token })).into_response();
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
    if let Some(cookie) = jar.get("session") {
        let conn = state
            .pool
            .get()
            .await
            .expect("Error getting connection from pool");

        let _ = conn
            .execute(
                "DELETE FROM sessions WHERE session = $1",
                &[&cookie.value()],
            )
            .await;
    }

    let mut headers = HeaderMap::new();
    headers.insert(
        "Set-Cookie",
        HeaderValue::from_static("session=; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=0"),
    );

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
async fn auth_handler(State(state): State<AppState>, Json(payload): Json<AuthRequest>) -> Response {
    let conn = state
        .pool
        .get()
        .await
        .expect("Error getting connection from pool");

    let found_account: bool;
    let password_match: bool;
    let mut player_id: i32 = 0;

    let account_name = payload.account_name;
    let password = payload.password;

    let row = conn
        .query_one(
            "SELECT player_id, password FROM accounts WHERE account_name = $1",
            &[&account_name],
        )
        .await;

    match row {
        Ok(row) => {
            found_account = true;
            println!("found_account: {}", found_account);
            player_id = row.get("player_id");
            println!("player_id: {}", player_id);

            let account_password: &str = row.get("password");

            let verify_password =
                Account::verify_password(password.clone(), account_password.to_string());

            match verify_password {
                Ok(_) => {
                    password_match = true;
                    println!("password_match: {}", password_match);
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
            .query_one(
                "SELECT session FROM sessions WHERE player_id = $1",
                &[&player_id],
            )
            .await;

        match session_row {
            Ok(session_row) => {
                // Get session string from database
                session = Some(session_row.get::<_, String>("session"));
            }
            Err(_) => {
                let mut rng = state.rng.lock().await;

                // generate new session and convert to string
                let session_num = rng.gen::<u128>().to_string();
                session = Some(session_num);

                store_new_session = true;
            }
        }
    }

    if store_new_session {
        println!("Storing new session in database");
        // store new session in database and return error if it fails
        let result = conn.execute(
            "INSERT INTO sessions (player_id, session, created_at) VALUES ($1, $2, current_timestamp)",
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
        // Generate device token
        let device_token = Uuid::new_v4().to_string();
        if let Err(e) = conn
            .execute(
                "INSERT INTO device_tokens (player_id, token, created_at) VALUES ($1, $2, NOW())",
                &[&player_id, &device_token],
            )
            .await
        {
            println!("Error storing device token: {}", e);
        }

        let cookie_value = format!(
            "session={}; HttpOnly; Secure; SameSite=Strict; Max-Age=604800",
            session
        );
        let mut headers = HeaderMap::new();
        headers.insert("Set-Cookie", HeaderValue::from_str(&cookie_value).unwrap());

        println!("Successfully authenticated: {}", player_id);
        (
            StatusCode::OK,
            headers,
            Json(AuthResponse {
                player_id: player_id,
                device_token,
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
async fn fingerprint_auth_handler(
    State(state): State<AppState>,
    Json(payload): Json<FingerprintAuthRequest>,
) -> Response {
    let fingerprint = payload.fingerprint.trim().to_string();
    let request_device_token = payload.device_token;

    // Validate fingerprint: must be non-empty, 8-64 chars, alphanumeric
    if fingerprint.is_empty() || fingerprint.len() < 8 || fingerprint.len() > 64 {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                msg: "Invalid fingerprint: must be between 8 and 64 characters".to_string(),
            }),
        )
            .into_response();
    }

    if !fingerprint.chars().all(|c| c.is_ascii_alphanumeric()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                msg: "Invalid fingerprint: must contain only alphanumeric characters".to_string(),
            }),
        )
            .into_response();
    }

    let conn = state
        .pool
        .get()
        .await
        .expect("Error getting connection from pool");

    // Look up the fingerprint in the accounts table
    let row = conn
        .query_opt(
            "SELECT player_id, account_name, password FROM accounts WHERE fingerprint = $1",
            &[&fingerprint],
        )
        .await;

    let row = match row {
        Ok(r) => r,
        Err(e) => {
            println!("Error looking up fingerprint: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthError {
                    msg: "Unknown error".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Resolve player identity: fingerprint match -> device_token fallback -> new player
    let mut resolved_player: Option<(i32, Option<String>, bool)> = None; // (player_id, account_name, has_account)
    let mut new_player = false;

    // Step 1: Try fingerprint match
    if let Some(row) = row {
        let player_id: i32 = row.get("player_id");
        let account_name: Option<String> = row.get("account_name");
        let password: Option<String> = row.get("password");

        if password.is_some() {
            println!("Fingerprint auth denied: player {} has password set, must use account login", player_id);
            return (
                StatusCode::UNAUTHORIZED,
                Json(AuthError {
                    msg: "Account requires password authentication".to_string(),
                }),
            )
                .into_response();
        }

        let has_account = account_name.is_some();
        println!("Fingerprint auth: found existing player {}", player_id);
        resolved_player = Some((player_id, account_name, has_account));
    }

    // Step 2: If fingerprint didn't match, try device_token fallback
    if resolved_player.is_none() {
        if let Some(ref token) = request_device_token {
            let token_row = conn
                .query_opt(
                    "SELECT player_id FROM device_tokens WHERE token = $1",
                    &[token],
                )
                .await;

            if let Ok(Some(token_row)) = token_row {
                let player_id: i32 = token_row.get("player_id");

                let account_row = conn
                    .query_opt(
                        "SELECT account_name, password FROM accounts WHERE player_id = $1",
                        &[&player_id],
                    )
                    .await;

                if let Ok(Some(account_row)) = account_row {
                    let password: Option<String> = account_row.get("password");

                    if password.is_some() {
                        println!("Device token auth denied: player {} has password set", player_id);
                        return (
                            StatusCode::UNAUTHORIZED,
                            Json(AuthError {
                                msg: "Account requires password authentication".to_string(),
                            }),
                        )
                            .into_response();
                    }

                    // Update fingerprint for this player
                    if let Err(e) = conn
                        .execute(
                            "UPDATE accounts SET fingerprint = $1 WHERE player_id = $2",
                            &[&fingerprint, &player_id],
                        )
                        .await
                    {
                        println!("Error updating fingerprint for player {}: {}", player_id, e);
                    }

                    let account_name: Option<String> = account_row.get("account_name");
                    let has_account = account_name.is_some();
                    println!("Device token auth: found existing player {}", player_id);
                    resolved_player = Some((player_id, account_name, has_account));
                }
            }
        }
    }

    // Step 3: If neither matched, create new player
    if resolved_player.is_none() {
        let no_name: Option<String> = None;
        let no_password: Option<String> = None;

        let result = conn
            .query_one(
                "INSERT INTO accounts (account_name, password, email, fingerprint, created_at) VALUES ($1, $2, $3, $4, current_timestamp) RETURNING player_id",
                &[&no_name, &no_password, &no_name, &fingerprint],
            )
            .await;

        match result {
            Ok(row) => {
                let player_id: i32 = row.get("player_id");
                let account_name = format!("account{}", player_id);

                // Set the account_name to account<PlayerId>
                if let Err(e) = conn
                    .execute(
                        "UPDATE accounts SET account_name = $1 WHERE player_id = $2",
                        &[&account_name, &player_id],
                    )
                    .await
                {
                    println!("Error setting account_name for player {}: {}", player_id, e);
                }

                println!("Fingerprint auth: created new player {}", player_id);
                new_player = true;
                resolved_player = Some((player_id, Some(account_name), false));
            }
            Err(e) => {
                println!("Error creating account for fingerprint: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(AuthError {
                        msg: "Failed to create account".to_string(),
                    }),
                )
                    .into_response();
            }
        }
    }

    let (player_id, account_name, has_account) = resolved_player.unwrap();

    // Generate device token
    let device_token = Uuid::new_v4().to_string();
    if let Err(e) = conn
        .execute(
            "INSERT INTO device_tokens (player_id, token, created_at) VALUES ($1, $2, NOW())",
            &[&player_id, &device_token],
        )
        .await
    {
        println!("Error storing device token: {}", e);
    }

    // Create a session (same logic as /auth)
    // First check for existing session
    let session_row = conn
        .query_opt(
            "SELECT session FROM sessions WHERE player_id = $1",
            &[&player_id],
        )
        .await;

    let (session, store_new) = match session_row {
        Ok(Some(row)) => {
            let session: String = row.get("session");
            (session, false)
        }
        Ok(None) => {
            let mut rng = state.rng.lock().await;
            let session_num = rng.gen::<u128>().to_string();
            (session_num, true)
        }
        Err(e) => {
            println!("Error checking session: {}", e);
            let mut rng = state.rng.lock().await;
            let session_num = rng.gen::<u128>().to_string();
            (session_num, true)
        }
    };

    if store_new {
        let result = conn
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

    let cookie_value = format!(
        "session={}; HttpOnly; Secure; SameSite=Strict; Max-Age=604800",
        session
    );
    let mut headers = HeaderMap::new();
    headers.insert("Set-Cookie", HeaderValue::from_str(&cookie_value).unwrap());

    println!("Fingerprint auth successful: player_id={}, has_account={}, new_player={}", player_id, has_account, new_player);
    (
        StatusCode::OK,
        headers,
        Json(FingerprintAuthResponse {
            player_id,
            has_account,
            new_player,
            account_name,
            device_token,
        }),
    )
        .into_response()
}

#[debug_handler]
async fn register_handler(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(payload): Json<AuthRequest>,
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

    // Look up player_id from the current session
    let session_row = conn
        .query_one(
            "SELECT player_id FROM sessions WHERE session = $1",
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

    let account_name = payload.account_name;
    let password = payload.password;

    let account = Account::new(account_name, password);

    // Update the existing account row for this player_id
    let result = conn.execute(
        "UPDATE accounts SET account_name = $1, password = $2 WHERE player_id = $3",
        &[&account.account_name, &account.password, &player_id],
    )
    .await;

    let Ok(_result) = result else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthError {
                msg: "Unknown error".to_string(),
            }),
        )
            .into_response();
    };

    // Generate device token
    let device_token = Uuid::new_v4().to_string();
    if let Err(e) = conn
        .execute(
            "INSERT INTO device_tokens (player_id, token, created_at) VALUES ($1, $2, NOW())",
            &[&player_id, &device_token],
        )
        .await
    {
        println!("Error storing device token: {}", e);
    }

    println!("Successfully registered: {}", player_id);
    (
        StatusCode::OK,
        Json(AuthResponse {
            player_id: player_id,
            device_token,
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

    let rows = conn
        .query(
            "SELECT id, hero_name, hero_rank, total_xp, fate FROM scores",
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
            fate: row.get::<_, String>("fate"),
        })
        .collect::<Vec<Score>>();

    println!("Successfully registered: {:?}", scores);
    (StatusCode::OK, Json(ScoreResponse(scores))).into_response()
}
