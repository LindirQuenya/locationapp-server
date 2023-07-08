use ::reqwest as realreqwest;
use actix_web::{
    cookie::{time::Duration, Cookie},
    get,
    http::header::{self, ContentType},
    web, HttpResponse, Responder,
};
use oauth2::{
    basic::BasicClient, reqwest::async_http_client, AuthUrl, AuthorizationCode, ClientId,
    ClientSecret, CsrfToken, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope,
    TokenResponse, TokenUrl,
};
use primitive_types::U512;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, time::Instant};

use crate::{config::Config, misc::forbidden, AppState, LONG_EXPIRY_SECS_I};

#[derive(Deserialize)]
pub(crate) struct RedirectQuery {
    code: String,
    state: String,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct SessionToken {
    pub(crate) session_key: U512,
    pub(crate) name: String,
}

pub(crate) struct OAuth {
    oauth_client: BasicClient,
    auth_url: String,
    csrf_state: CsrfToken,
    // Don't worry, this is fine. Nobody can intercept our requests to Google, so this
    // can never be compromised. I hope.
    pkce_verif: String,
}

pub(crate) fn generate_oauth(config: &Config) -> OAuth {
    let google_client_id = ClientId::new(config.oauth_provider.client_id.to_string());
    let google_client_secret = ClientSecret::new(config.oauth_provider.client_secret.to_string());
    let endpoint_auth_url = AuthUrl::new(config.oauth_provider.auth_url.to_string())
        .expect("Invalid authorization endpoint URL");
    let token_url = TokenUrl::new(config.oauth_provider.token_url.to_string())
        .expect("Invalid token endpoint URL");
    let client = BasicClient::new(
        google_client_id,
        Some(google_client_secret),
        endpoint_auth_url,
        Some(token_url),
    )
    // Set the URL the user will be redirected to after the authorization process.
    .set_redirect_uri(
        RedirectUrl::new(format!("https://{}/api/auth/redirect", config.domain_name))
            .expect("Invalid redirect URL - bad domain name?"),
    );
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        // Set the desired scopes.
        .add_scopes(
            config
                .oauth_provider
                .scopes
                .iter()
                .map(|s| Scope::new(s.to_string())),
        )
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

#[derive(Serialize)]
struct URLOut {
    url: String,
}

#[get("/api/auth/url")]
pub(crate) async fn get_auth_url(data: web::Data<AppState>) -> impl Responder {
    HttpResponse::Ok().insert_header(ContentType::json()).body(
        serde_json::to_string(&URLOut {
            url: data.auth.auth_url.clone(),
        })
        .unwrap(),
    )
}

#[get("/api/auth/redirect")]
pub(crate) async fn get_auth_redirect(
    data: web::Data<AppState>,
    query: web::Query<RedirectQuery>,
) -> impl Responder {
    if query.state.ne(data.auth.csrf_state.secret()) {
        log::debug!("/api/auth/redirect: Invalid secret.");
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
            log::debug!("/api/auth/redirect: Failed to get token.");
            return forbidden();
        }
    };
    let apiresponse: HashMap<String, Value> = match realreqwest::Client::new()
        .get(data.config.userinfo_endpoint.to_string())
        .bearer_auth(token.access_token().secret())
        .send()
        .await
    {
        Ok(resp) => match serde_json::from_str(&match resp.text().await {
            Ok(t) => {
                log::trace!("/api/auth/redirect: userinfo returned: {}", t);
                t
            }
            _ => {
                log::debug!("/api/auth/redirect: userinfo returned no body?");
                return forbidden();
            }
        }) {
            Ok(h) => h,
            _ => {
                log::debug!("/api/auth/redirect: Failed to parse JSON map.");
                return forbidden();
            }
        },
        _ => {
            log::debug!("/api/auth/redirect: userinfo request failed.");
            return forbidden();
        }
    };
    let email = match apiresponse.get("email") {
        Some(e) => match e.as_str() {
            Some(estr) => estr.to_owned(),
            None => {
                log::debug!("/api/auth/redirect: email wasn't a string?");
                return forbidden();
            }
        },
        None => {
            log::debug!("/api/auth/redirect: userinfo didn't give an email.");
            return forbidden();
        }
    };
    let name = match crate::db::verify_email(&data.pool, email.clone()).await {
        Ok(Some(name)) => name,
        Ok(None) => {
            log::debug!("/api/auth/redirect: email not in db: '{}'.", email);
            return forbidden();
        }
        Err(_) => {
            log::debug!("/api/auth/redirect: something went wrong in the db.");
            return forbidden();
        }
    };
    // WHEEEE, we made it!
    // Generate a session key.
    let response = SessionToken {
        session_key: U512(rand::random()),
        name,
    };
    let now = Instant::now();
    {
        data.session_tokens.insert(
            response.session_key,
            crate::TokenExpiry {
                last_used: now,
                issued: now,
            },
        );
    }
    let cookie = Cookie::build("session", serde_json::to_string(&response).unwrap())
        .domain(data.config.domain_name.to_string())
        .max_age(Duration::seconds(LONG_EXPIRY_SECS_I))
        .same_site(actix_web::cookie::SameSite::Strict)
        .http_only(true)
        .secure(true)
        .path("/")
        .finish();
    let response_body = RedirectResponse {
        message: "Auth successful. Return home.".to_string(),
        href: data.config.redirect_after_auth.to_string(),
    };
    HttpResponse::SeeOther()
        .cookie(cookie)
        .append_header((header::LOCATION, response_body.href.clone()))
        .insert_header(ContentType::json())
        .body(serde_json::to_string(&response_body).unwrap())
}

#[derive(Serialize)]
struct RedirectResponse {
    message: String,
    href: String,
}
