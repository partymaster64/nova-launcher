/// Microsoft OAuth2 → Xbox Live → Minecraft Auth Flow
///
/// Verwendet den Device-Code-Flow (kein Browser-Redirect nötig).
/// Du musst in Azure AD eine App mit folgenden Einstellungen registrieren:
///   - Platform: Mobile/Desktop
///   - Redirect URI: https://login.microsoftonline.com/common/oauth2/nativeclient
///   - API Permissions: XboxLive.signin (delegiert)
/// Die Client-ID dann in CLIENT_ID eintragen.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

/// Azure App Client-ID – hier eigene eintragen!
/// Standardmäßig die offizielle Xbox-App ID (funktioniert für persönliche Konten)
pub const CLIENT_ID: &str = "00000000402b5328";
pub const SCOPE: &str = "service::user.auth.xboxlive.com::MBI_SSL";

// ─── Device Code Flow ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    #[serde(alias = "verification_url")]
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct MsTokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
    error: Option<String>,
    error_description: Option<String>,
}

/// Schritt 1: Device-Code anfordern
pub async fn request_device_code(client: &reqwest::Client) -> Result<DeviceCodeResponse> {
    let params = [
        ("client_id", CLIENT_ID),
        ("scope", SCOPE),
        ("response_type", "device_code"),
    ];
    let resp = client
        .post("https://login.live.com/oauth20_connect.srf")
        .form(&params)
        .send()
        .await?
        .error_for_status()?;

    let dc: DeviceCodeResponse = resp.json().await?;
    Ok(dc)
}

/// Schritt 2: Auf Token-Antwort warten (polling)
pub async fn poll_for_token(
    client: &reqwest::Client,
    device_code: &str,
    interval_secs: u64,
) -> Result<(String, String, u64)> {
    let interval = std::time::Duration::from_secs(interval_secs.max(5));
    loop {
        tokio::time::sleep(interval).await;

        let params = [
            ("client_id", CLIENT_ID),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("device_code", device_code),
        ];
        let resp: MsTokenResponse = client
            .post("https://login.live.com/oauth20_token.srf")
            .form(&params)
            .send()
            .await?
            .json()
            .await?;

        if let Some(err) = &resp.error {
            match err.as_str() {
                "authorization_pending" => continue,
                "slow_down" => {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
                _ => bail!(
                    "Auth-Fehler: {} – {}",
                    err,
                    resp.error_description.unwrap_or_default()
                ),
            }
        }

        let access = resp.access_token.context("Kein access_token")?;
        let refresh = resp.refresh_token.context("Kein refresh_token")?;
        let expires = resp.expires_in.unwrap_or(3600);
        return Ok((access, refresh, expires));
    }
}

/// Token mit Refresh-Token erneuern
pub async fn refresh_ms_token(
    client: &reqwest::Client,
    refresh_token: &str,
) -> Result<(String, String, u64)> {
    let params = [
        ("client_id", CLIENT_ID),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("scope", SCOPE),
    ];
    let resp: MsTokenResponse = client
        .post("https://login.microsoftonline.com/common/oauth2/v2.0/token")
        .form(&params)
        .send()
        .await?
        .json()
        .await?;

    if let Some(err) = resp.error {
        bail!("Token-Refresh fehlgeschlagen: {}", err);
    }
    let access = resp.access_token.context("Kein access_token")?;
    let refresh = resp.refresh_token.unwrap_or_else(|| refresh_token.to_string());
    let expires = resp.expires_in.unwrap_or(3600);
    Ok((access, refresh, expires))
}

// ─── Xbox Live Auth ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct XblResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: XblDisplayClaims,
}

#[derive(Debug, Deserialize)]
struct XblDisplayClaims {
    xui: Vec<XblXui>,
}

#[derive(Debug, Deserialize)]
struct XblXui {
    uhs: String,
}

#[derive(Debug, Serialize)]
struct XblAuthProperties {
    #[serde(rename = "AuthMethod")]
    auth_method: String,
    #[serde(rename = "SiteName")]
    site_name: String,
    #[serde(rename = "RpsTicket")]
    rps_ticket: String,
}

#[derive(Debug, Serialize)]
struct XblAuthRequest {
    #[serde(rename = "Properties")]
    properties: XblAuthProperties,
    #[serde(rename = "RelyingParty")]
    relying_party: String,
    #[serde(rename = "TokenType")]
    token_type: String,
}

/// Schritt 3: Microsoft Token → Xbox Live Token
pub async fn auth_xbox_live(
    client: &reqwest::Client,
    ms_token: &str,
) -> Result<(String, String)> {
    let body = XblAuthRequest {
        properties: XblAuthProperties {
            auth_method: "RPS".into(),
            site_name: "user.auth.xboxlive.com".into(),
            rps_ticket: format!("t={}", ms_token),
        },
        relying_party: "http://auth.xboxlive.com".into(),
        token_type: "JWT".into(),
    };

    let resp: XblResponse = client
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&body)
        .header("Accept", "application/json")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let uhs = resp
        .display_claims
        .xui
        .first()
        .context("Kein uhs in XBL-Antwort")?
        .uhs
        .clone();
    Ok((resp.token, uhs))
}

// ─── XSTS Token ──────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct XstsAuthProperties {
    #[serde(rename = "SandboxId")]
    sandbox_id: String,
    #[serde(rename = "UserTokens")]
    user_tokens: Vec<String>,
}

#[derive(Debug, Serialize)]
struct XstsAuthRequest {
    #[serde(rename = "Properties")]
    properties: XstsAuthProperties,
    #[serde(rename = "RelyingParty")]
    relying_party: String,
    #[serde(rename = "TokenType")]
    token_type: String,
}

/// Schritt 4: XBL Token → XSTS Token
pub async fn auth_xsts(
    client: &reqwest::Client,
    xbl_token: &str,
) -> Result<String> {
    let body = XstsAuthRequest {
        properties: XstsAuthProperties {
            sandbox_id: "RETAIL".into(),
            user_tokens: vec![xbl_token.to_string()],
        },
        relying_party: "rp://api.minecraftservices.com/".into(),
        token_type: "JWT".into(),
    };

    let resp: XblResponse = client
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&body)
        .header("Accept", "application/json")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(resp.token)
}

// ─── Minecraft Auth ───────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct McAuthRequest {
    #[serde(rename = "identityToken")]
    identity_token: String,
}

#[derive(Debug, Deserialize)]
struct McAuthResponse {
    access_token: String,
    expires_in: u64,
}

#[derive(Debug, Deserialize)]
pub struct McProfile {
    pub id: String,
    pub name: String,
}

/// Schritt 5: XSTS Token → Minecraft Access Token
pub async fn auth_minecraft(
    client: &reqwest::Client,
    xsts_token: &str,
    uhs: &str,
) -> Result<(String, u64)> {
    let identity_token = format!("XBL3.0 x={};{}", uhs, xsts_token);
    let body = McAuthRequest { identity_token };

    let resp: McAuthResponse = client
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok((resp.access_token, resp.expires_in))
}

/// Schritt 6: Minecraft-Profil laden
pub async fn get_mc_profile(
    client: &reqwest::Client,
    mc_token: &str,
) -> Result<McProfile> {
    let profile: McProfile = client
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(mc_token)
        .send()
        .await?
        .error_for_status()
        .context("Kein Minecraft-Kauf? Profil nicht gefunden")?
        .json()
        .await?;
    Ok(profile)
}

// ─── Kompletter Auth-Flow ─────────────────────────────────────────────────────

pub struct AuthResult {
    pub uuid: String,
    pub username: String,
    pub minecraft_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
}

/// Führt den kompletten Auth-Flow durch (MS → XBL → XSTS → MC → Profil)
pub async fn complete_auth(
    client: &reqwest::Client,
    ms_access: &str,
    ms_refresh: &str,
    ms_expires_in: u64,
) -> Result<AuthResult> {
    let (xbl_token, uhs) = auth_xbox_live(client, ms_access).await?;
    let xsts_token = auth_xsts(client, &xbl_token).await?;
    let (mc_token, mc_expires) = auth_minecraft(client, &xsts_token, &uhs).await?;
    let profile = get_mc_profile(client, &mc_token).await?;

    let expires_at = chrono::Utc::now().timestamp() + mc_expires as i64;

    Ok(AuthResult {
        uuid: profile.id,
        username: profile.name,
        minecraft_token: mc_token,
        refresh_token: ms_refresh.to_string(),
        expires_at,
    })
}
