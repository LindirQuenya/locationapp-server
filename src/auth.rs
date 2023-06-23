use ::reqwest as realreqwest;
use actix_web::{get, web, HttpResponse, Responder};
use oauth2::{
    basic::BasicClient, reqwest::async_http_client, AuthUrl, AuthorizationCode, ClientId,
    ClientSecret, CsrfToken, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope,
    TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, env, time::Instant};

use crate::{misc::forbidden, AppState};

#[derive(Deserialize)]
pub(crate) struct RedirectQuery {
    code: String,
    state: String,
}

#[derive(Serialize)]
struct SessionKeyResponse {
    session_key: u128,
    name: String,
}

pub(crate) struct OAuth {
    oauth_client: BasicClient,
    auth_url: String,
    csrf_state: CsrfToken,
    // Don't worry, this is fine. Nobody can intercept our requests to Google, so this
    // can never be compromised. I hope.
    pkce_verif: String,
}

pub(crate) fn generate_oauth() -> OAuth {
    let google_client_id = ClientId::new(
        env::var("GOOGLE_CLIENT_ID").expect("Missing the GOOGLE_CLIENT_ID environment variable."),
    );
    let google_client_secret = ClientSecret::new(
        env::var("GOOGLE_CLIENT_SECRET")
            .expect("Missing the GOOGLE_CLIENT_SECRET environment variable."),
    );
    let endpoint_auth_url = AuthUrl::new("https://accounts.google.com/o/oauth2/auth".to_string())
        .expect("Invalid authorization endpoint URL");
    let token_url = TokenUrl::new("https://oauth2.googleapis.com/token".to_string())
        .expect("Invalid token endpoint URL");
    let client = BasicClient::new(
        google_client_id,
        Some(google_client_secret),
        endpoint_auth_url,
        Some(token_url),
    )
    // Set the URL the user will be redirected to after the authorization process.
    .set_redirect_uri(
        RedirectUrl::new("https://eldamar.duckdns.org/auth/redirect".to_string()).unwrap(),
    );
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        // Set the desired scopes.
        .add_scope(Scope::new(
            "https://www.googleapis.com/auth/userinfo.email".to_string(),
        ))
        // Set the PKCE code challenge.
        .set_pkce_challenge(pkce_challenge)
        .url();
    OAuth {
        oauth_client: client,
        auth_url: auth_url.to_string(),
        csrf_state: csrf_token,
        pkce_verif: pkce_verifier.secret().clone(),
    }
}

#[get("/auth/url")]
pub(crate) async fn get_auth_url(data: web::Data<AppState>) -> impl Responder {
    HttpResponse::Ok().body(data.auth.auth_url.clone())
}

#[get("/auth/redirect")]
pub(crate) async fn get_auth_redirect(
    data: web::Data<AppState>,
    query: web::Query<RedirectQuery>,
) -> impl Responder {
    if query.state.ne(data.auth.csrf_state.secret()) {
        log::debug!("/auth/redirect: Invalid secret.");
        return forbidden();
    }
    let token_result = data
        .auth
        .oauth_client
        .exchange_code(AuthorizationCode::new(query.code.clone()))
        // Set the PKCE code verifier.
        .set_pkce_verifier(PkceCodeVerifier::new(data.auth.pkce_verif.clone()))
        .request_async(async_http_client)
        .await;
    let token = match token_result {
        Ok(tok) => tok,
        _ => {
            log::debug!("/auth/redirect: Failed to get token.");
            return forbidden();
        }
    };
    token.access_token().secret();
    let apiresponse: HashMap<String, Value> = match realreqwest::Client::new()
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .bearer_auth(token.access_token().secret())
        .send()
        .await
    {
        Ok(resp) => match serde_json::from_str(&match resp.text().await {
            Ok(t) => t,
            _ => {
                log::debug!("/auth/redirect: userinfo returned no body?");
                return forbidden();
            }
        }) {
            Ok(h) => h,
            _ => {
                log::debug!("/auth/redirect: Failed to parse JSON map.");
                return forbidden();
            }
        },
        _ => {
            log::debug!("/auth/redirect: userinfo request failed.");
            return forbidden();
        }
    };
    let email = match apiresponse.get("email") {
        Some(e) => e,
        None => {
            log::debug!("/auth/redirect: userinfo didn't give an email.");
            return forbidden();
        }
    };
    let name = match crate::db::verify_email(&data.pool, email.to_string()).await {
        Ok(Some(name)) => name,
        Ok(None) => {
            log::debug!("/auth/redirect: email not in db.");
            return forbidden();
        }
        Err(_) => {
            log::debug!("/auth/redirect: something went wrong in the db.");
            return forbidden();
        }
    };
    // WHEEEE, we made it!
    // Generate a session key.
    let sessionkey = SessionKeyResponse {
        session_key: rand::random(),
        name,
    };
    let now = Instant::now();
    data.session_tokens.insert(
        sessionkey.session_key,
        crate::TokenExpiry {
            last_used: now,
            issued: now,
        },
    );
    HttpResponse::Ok().body(serde_json::to_string(&sessionkey).unwrap())
}
