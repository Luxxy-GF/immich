use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminOnboardingUpdateDto {
    pub is_onboarded: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminOnboardingResponseDto {
    pub is_onboarded: bool,
}
