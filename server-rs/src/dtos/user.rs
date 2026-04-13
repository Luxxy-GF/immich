use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserAdminResponseDto {
    pub id: String,
    pub email: String,
    pub name: String,
    pub first_name: String,
    pub last_name: String,
    pub profile_image_path: String,
    pub is_admin: bool,
    pub should_change_password: bool,
    pub storage_label: Option<String>,
    pub status: String,
    pub quota_size_in_bytes: Option<i64>,
    pub quota_usage_in_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingDto {
    pub is_onboarded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingResponseDto {
    pub is_onboarded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct UserPreferencesResponseDto {
    pub albums: AlbumsResponseDto,
    pub folders: FoldersResponseDto,
    pub memories: MemoriesResponseDto,
    pub people: PeopleResponseDto,
    pub ratings: RatingsResponseDto,
    pub shared_links: SharedLinksResponseDto,
    pub tags: TagsResponseDto,
    pub email_notifications: EmailNotificationsResponseDto,
    pub download: DownloadResponseDto,
    pub purchase: PurchaseResponseDto,
    pub cast: CastResponseDto,
}

impl Default for UserPreferencesResponseDto {
    fn default() -> Self {
        Self {
            albums: AlbumsResponseDto::default(),
            folders: FoldersResponseDto::default(),
            memories: MemoriesResponseDto::default(),
            people: PeopleResponseDto::default(),
            ratings: RatingsResponseDto::default(),
            shared_links: SharedLinksResponseDto::default(),
            tags: TagsResponseDto::default(),
            email_notifications: EmailNotificationsResponseDto::default(),
            download: DownloadResponseDto::default(),
            purchase: PurchaseResponseDto::default(),
            cast: CastResponseDto::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AlbumsResponseDto {
    pub default_asset_order: String,
}

impl Default for AlbumsResponseDto {
    fn default() -> Self {
        Self {
            default_asset_order: "desc".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct FoldersResponseDto {
    pub enabled: bool,
    pub sidebar_web: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct MemoriesResponseDto {
    pub enabled: bool,
    pub duration: i32,
}

impl Default for MemoriesResponseDto {
    fn default() -> Self {
        Self {
            enabled: true,
            duration: 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct PeopleResponseDto {
    pub enabled: bool,
    pub sidebar_web: bool,
}

impl Default for PeopleResponseDto {
    fn default() -> Self {
        Self {
            enabled: true,
            sidebar_web: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct RatingsResponseDto {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct SharedLinksResponseDto {
    pub enabled: bool,
    pub sidebar_web: bool,
}

impl Default for SharedLinksResponseDto {
    fn default() -> Self {
        Self {
            enabled: true,
            sidebar_web: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct TagsResponseDto {
    pub enabled: bool,
    pub sidebar_web: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct EmailNotificationsResponseDto {
    pub enabled: bool,
    pub album_invite: bool,
    pub album_update: bool,
}

impl Default for EmailNotificationsResponseDto {
    fn default() -> Self {
        Self {
            enabled: true,
            album_invite: true,
            album_update: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct DownloadResponseDto {
    pub archive_size: i64,
    pub include_embedded_videos: bool,
}

impl Default for DownloadResponseDto {
    fn default() -> Self {
        Self {
            archive_size: 4 * 1024 * 1024 * 1024,
            include_embedded_videos: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct PurchaseResponseDto {
    pub show_support_badge: bool,
    pub hide_buy_button_until: String,
}

impl Default for PurchaseResponseDto {
    fn default() -> Self {
        Self {
            show_support_badge: true,
            hide_buy_button_until: "2022-02-12T00:00:00.000Z".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CastResponseDto {
    pub g_cast_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct UserPreferencesUpdateDto {
    pub albums: Option<AlbumsUpdateDto>,
    pub avatar: Option<AvatarUpdateDto>,
    pub cast: Option<CastUpdateDto>,
    pub download: Option<DownloadUpdateDto>,
    pub email_notifications: Option<EmailNotificationsUpdateDto>,
    pub folders: Option<FoldersUpdateDto>,
    pub memories: Option<MemoriesUpdateDto>,
    pub people: Option<PeopleUpdateDto>,
    pub purchase: Option<PurchaseUpdateDto>,
    pub ratings: Option<RatingsUpdateDto>,
    pub shared_links: Option<SharedLinksUpdateDto>,
    pub tags: Option<TagsUpdateDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct AlbumsUpdateDto {
    pub default_asset_order: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct AvatarUpdateDto {
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CastUpdateDto {
    pub g_cast_enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct DownloadUpdateDto {
    pub archive_size: Option<i64>,
    pub include_embedded_videos: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct EmailNotificationsUpdateDto {
    pub enabled: Option<bool>,
    pub album_invite: Option<bool>,
    pub album_update: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct FoldersUpdateDto {
    pub enabled: Option<bool>,
    pub sidebar_web: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct MemoriesUpdateDto {
    pub enabled: Option<bool>,
    pub duration: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PeopleUpdateDto {
    pub enabled: Option<bool>,
    pub sidebar_web: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PurchaseUpdateDto {
    pub show_support_badge: Option<bool>,
    pub hide_buy_button_until: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct RatingsUpdateDto {
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SharedLinksUpdateDto {
    pub enabled: Option<bool>,
    pub sidebar_web: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct TagsUpdateDto {
    pub enabled: Option<bool>,
    pub sidebar_web: Option<bool>,
}
