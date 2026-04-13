use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginCredentialDto {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponseDto {
    pub access_token: String,
    pub user_id: String,
    pub user_email: String,
    pub name: String,
    pub profile_image_path: String,
    pub is_admin: bool,
    pub should_change_password: bool,
    pub is_onboarded: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatusResponseDto {
    pub pin_code: bool,
    pub password: bool,
    pub is_elevated: bool,
    pub expires_at: Option<String>,
    pub pin_expires_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateAccessTokenResponseDto {
    pub auth_status: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogoutResponseDto {
    pub successful: bool,
    pub redirect_uri: String,
}
