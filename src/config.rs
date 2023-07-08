use schemars::JsonSchema;
use serde::Deserialize;

#[allow(dead_code)]
#[derive(Deserialize, JsonSchema)]
pub(crate) struct Config {
    pub(crate) oauth_provider: OauthConfig,
    pub(crate) domain_name: String,
    pub(crate) userinfo_endpoint: String,
    pub(crate) redirect_after_auth: String,
    pub(crate) listen: Vec<ListenSpec>,
    pub(crate) db_path: String,
}

#[allow(dead_code)]
#[derive(Deserialize, JsonSchema)]
pub(crate) struct OauthConfig {
    pub(crate) client_id: String,
    pub(crate) client_secret: String,
    pub(crate) auth_url: String,
    pub(crate) token_url: String,
    pub(crate) scopes: Vec<String>,
}

#[allow(dead_code)]
#[derive(Deserialize, Clone, JsonSchema)]
pub(crate) struct ListenSpec {
    pub(crate) addr: String,
    pub(crate) port: u16,
}
