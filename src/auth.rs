use ::reqwest as realreqwest;
use actix_web::{
    cookie::{time::Duration, Cookie},
    get,
    http::header::{self, ContentType},
    web, HttpRequest, HttpResponse, Responder,
};
use dashmap::DashMap;
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

/// In minutes.
const MAX_AUTH_DURATION_MINUTES: i64 = 5;

#[derive(Serialize, Deserialize)]
pub(crate) struct SessionToken {
    pub(crate) session_key: U512,
    pub(crate) name: String,
}

pub(crate) struct OAuth {
    oauth_client: BasicClient,
    /// Associates PKCE verification objects (and creation time, for expiry) with the random state parameters.
    pkce_verifs: DashMap<String, (Instant, PkceCodeVerifier)>,
}

#[derive(Deserialize)]
pub(crate) struct RedirectQuery {
    code: String,
    state: CsrfToken,
}

#[derive(Serialize)]
struct RedirectOut {
    message: String,
    href: String,
}

#[derive(Serialize)]
struct URLOut {
    url: String,
}

pub(crate) fn generate_oauth(config: &Config) -> OAuth {
    // Read the config properties and process them.
    let google_client_id = ClientId::new(config.oauth_provider.client_id.to_string());
    let google_client_secret = ClientSecret::new(config.oauth_provider.client_secret.to_string());
    let endpoint_auth_url = AuthUrl::new(config.oauth_provider.auth_url.to_string())
        .expect("Invalid authorization endpoint URL");
    let token_url = TokenUrl::new(config.oauth_provider.token_url.to_string())
        .expect("Invalid token endpoint URL");

    // Construct a client from our config properties.
    let client = BasicClient::new(
        google_client_id,
        Some(google_client_secret),
        endpoint_auth_url,
        Some(token_url),
    )
    .set_redirect_uri(
        // The redirect URL is "https://your-site.com/api/auth/redirect";
        RedirectUrl::new(format!("https://{}/api/auth/redirect", config.domain_name))
            .expect("Invalid redirect URL - bad domain name?"),
    );
    OAuth {
        oauth_client: client,
        pkce_verifs: DashMap::with_capacity(4),
    }
}

#[get("/api/auth/url")]
pub(crate) async fn get_auth_url(data: web::Data<AppState>) -> impl Responder {
    // Construct this statically, to prevent extra cost. This is the longest
    // an authentication should be allowed to take.
    static MAX_AUTH_DURATION: Duration = Duration::minutes(MAX_AUTH_DURATION_MINUTES);

    // Generate a new PKCE challenge for this client.
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    // Generate an auth URL and CSRF token.
    let (auth_url, csrf_token) = data
        .auth
        .oauth_client
        .authorize_url(CsrfToken::new_random)
        // Set the desired scopes.
        .add_scopes(
            data.config
                .oauth_provider
                .scopes
                .iter()
                .map(|s| Scope::new(s.to_string())),
        )
        // Set the PKCE code challenge.
        .set_pkce_challenge(pkce_challenge)
        .url();

    // Make a cookie to hold the CSRF token. This should be impossible for a non-XSS attacker
    // to fake on a victim's machine - setting cookies on another site isn't allowed.
    // It also shouldn't be readable by anyone ever - only the service needs to see it.
    let cookie = Cookie::build("csrf_state", serde_json::to_string(&csrf_token).unwrap())
        .domain(data.config.domain_name.to_string())
        .max_age(MAX_AUTH_DURATION)
        .same_site(actix_web::cookie::SameSite::Strict)
        .http_only(true)
        .secure(true)
        .path("/api/auth/")
        .finish();
    // Associate the PKCE challenge with the CSRF token.
    data.auth.pkce_verifs.insert(
        csrf_token.secret().to_string(),
        (Instant::now(), pkce_verifier),
    );
    // Remove any pkce verifs that have expired. This prevents
    // a resource-exhaustion attack.
    data.auth
        .pkce_verifs
        .retain(|_, v| v.0.elapsed() <= MAX_AUTH_DURATION);
    HttpResponse::Ok()
        .cookie(cookie)
        .insert_header(ContentType::json())
        .body(
            serde_json::to_string(&URLOut {
                url: auth_url.to_string(),
            })
            .unwrap(),
        )
}

/// Validates CSRF and PKCE properties and then gets a bearer token from the oauth API.
async fn request_bearer_token(
    auth: &OAuth,
    query: web::Query<RedirectQuery>,
    req: HttpRequest,
) -> Option<String> {
    // Read the CSRF token from the cookie.
    let cookie_csrf_token = match read_csrf_token(req) {
        Some(t) => t,
        None => {
            log::debug!("/api/auth/redirect: No CSRF token cookie.");
            return None;
        }
    };
    // Confirm that the cookie matches what's in the request parameters.
    if query.state.secret().ne(cookie_csrf_token.secret()) {
        log::debug!("/api/auth/redirect: CSRF token didn't match.");
        return None;
    }
    // Try to remove the corresponding PKCE verifier from the hashmap.
    let pkce_verif = match auth.pkce_verifs.remove(cookie_csrf_token.secret()) {
        Some(k) => k.1 .1,
        None => {
            log::debug!("/api/auth/redirect: CSRF token has no PKCE.");
            return None;
        }
    };
    // Get a bearer token from the code.
    let token_result = auth
        .oauth_client
        .exchange_code(AuthorizationCode::new(query.code.clone()))
        // Set the PKCE code verifier.
        .set_pkce_verifier(pkce_verif)
        .request_async(async_http_client)
        .await;
    let token = match token_result {
        Ok(tok) => tok,
        _ => {
            log::debug!("/api/auth/redirect: Failed to get token.");
            return None;
        }
    };
    Some(token.access_token().secret().to_string())
}

// Converts a bearer token into an email address using Google's userinfo API.
async fn request_userinfo(token: String, config: &Config) -> Option<String> {
    // Make the request to the API.
    let response = match realreqwest::Client::new()
        .get(config.userinfo_endpoint.to_string())
        .bearer_auth(token)
        .send()
        .await
    {
        Ok(resp) => resp,
        _ => {
            log::debug!("/api/auth/redirect: userinfo request failed.");
            return None;
        }
    };

    // Get the body of the response.
    let response_body = match response.text().await {
        Ok(t) => {
            log::trace!("/api/auth/redirect: userinfo returned: {}", t);
            t
        }
        _ => {
            log::debug!("/api/auth/redirect: userinfo returned no body?");
            return None;
        }
    };

    // Parse the response body into a hashmap. The response should be a JSON object.
    let apiresponse: HashMap<String, Value> = match serde_json::from_str(&response_body) {
        Ok(h) => h,
        _ => {
            log::debug!("/api/auth/redirect: Failed to parse JSON map.");
            return None;
        }
    };

    // Try to get the "email" key out of the hashmap.
    let email = match apiresponse.get("email") {
        // It should be a JSON string. In theory it might be different.
        Some(e) => match e.as_str() {
            Some(estr) => estr.to_owned(),
            None => {
                log::debug!("/api/auth/redirect: email wasn't a string?");
                return None;
            }
        },
        None => {
            log::debug!("/api/auth/redirect: userinfo didn't give an email.");
            return None;
        }
    };

    // Yay, we made it!
    Some(email)
}

#[get("/api/auth/redirect")]
pub(crate) async fn get_auth_redirect(
    data: web::Data<AppState>,
    query: web::Query<RedirectQuery>,
    req: HttpRequest,
) -> impl Responder {
    // Try to exchange the code for a bearer token.
    let token = match request_bearer_token(&data.auth, query, req).await {
        Some(t) => t,
        None => {
            return forbidden();
        }
    };

    // Use the bearer token to get the account's associated email.
    let email = match request_userinfo(token, &data.config).await {
        Some(email) => email,
        None => {
            return forbidden();
        }
    };

    // Check if the email exists in the database, and if so get the user's name.
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

    // WHEEEE, we made it! The user is real, verified, and authorized.
    // Generate a session key.
    let response = SessionToken {
        session_key: U512(rand::random()),
        name,
    };

    // Record the current time, for session key expiration.
    let now = Instant::now();

    // Pop the session key into our hashmap of valid ones.
    data.session_tokens.insert(
        response.session_key,
        crate::TokenExpiry {
            last_used: now,
            issued: now,
        },
    );

    // Build a cookie to hold the session key.
    // We'll send this cookie along with a redirect back to our frontend.
    let cookie = Cookie::build("session", serde_json::to_string(&response).unwrap())
        .domain(data.config.domain_name.to_string())
        .max_age(Duration::seconds(LONG_EXPIRY_SECS_I))
        .same_site(actix_web::cookie::SameSite::Strict)
        .http_only(true)
        .secure(true)
        .path("/api/")
        .finish();

    // Just in case there's some ancient browser that doesn't follow redirects,
    // send a message too. JSON because I want to and all my other APIs return it.
    let response_body = RedirectOut {
        message: "Auth successful. Return home.".to_string(),
        href: data.config.redirect_after_auth.to_string(),
    };

    // Redirect to the configured location with the cookie and the message.
    HttpResponse::SeeOther()
        .cookie(cookie)
        .append_header((header::LOCATION, response_body.href.clone()))
        .insert_header(ContentType::json())
        .body(serde_json::to_string(&response_body).unwrap())
}

fn read_csrf_token(req: HttpRequest) -> Option<CsrfToken> {
    match req.cookies() {
        Ok(cookievec) => {
            log::trace!("got cookies");
            for cookie in cookievec.iter() {
                if cookie.name() == "csrf_state" {
                    log::trace!("found csrf_state cookie: {}", cookie.value());
                    let parsed_cookie = serde_json::from_str(cookie.value());
                    log::trace!("parsed_cookie: {:?}", parsed_cookie);
                    return parsed_cookie.ok();
                }
            }
            None
        }
        Err(_) => None,
    }
}
