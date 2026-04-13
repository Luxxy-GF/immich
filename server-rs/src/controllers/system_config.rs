use axum::{extract::State, routing::get, Json, Router};
use serde_json::{json, Value};

use crate::{error::AppError, middleware::auth::AuthDto, AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_config).put(update_config))
        .route("/defaults", get(get_defaults))
        .route("/storage-template-options", get(get_storage_template_options))
}

async fn get_config(State(state): State<AppState>, _auth: AuthDto) -> Result<Json<Value>, AppError> {
    let stored = sqlx::query_scalar::<_, Value>(r#"SELECT value FROM system_metadata WHERE key = 'system-config'"#)
        .fetch_optional(&state.db)
        .await?;
    Ok(Json(merge_json(default_system_config(), stored.unwrap_or_else(|| json!({})))))
}

async fn get_defaults(_auth: AuthDto) -> Result<Json<Value>, AppError> {
    Ok(Json(default_system_config()))
}

async fn update_config(
    State(state): State<AppState>,
    _auth: AuthDto,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, AppError> {
    let merged = merge_json(default_system_config(), payload);
    sqlx::query(r#"INSERT INTO system_metadata (key, value) VALUES ('system-config', $1) ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value"#)
        .bind(&merged)
        .execute(&state.db)
        .await?;
    Ok(Json(merged))
}

async fn get_storage_template_options(_auth: AuthDto) -> Result<Json<Value>, AppError> {
    Ok(Json(json!({
        "secondOptions": ["s", "ss", "SSS"],
        "minuteOptions": ["m", "mm"],
        "dayOptions": ["d", "dd"],
        "weekOptions": ["W", "WW"],
        "hourOptions": ["h", "hh", "H", "HH"],
        "yearOptions": ["y", "yy"],
        "monthOptions": ["M", "MM", "MMM", "MMMM"],
        "presetOptions": [
            "{{y}}/{{y}}-{{MM}}-{{dd}}/{{filename}}",
            "{{y}}/{{MM}}-{{dd}}/{{filename}}",
            "{{y}}/{{MMMM}}-{{dd}}/{{filename}}",
            "{{y}}/{{MM}}/{{filename}}",
            "{{y}}/{{MM}}/{{dd}}/{{filename}}",
            "{{y}}-{{MM}}-{{dd}}/{{filename}}",
            "{{album}}/{{filename}}"
        ]
    })))
}

fn default_system_config() -> Value {
    json!({
        "backup": {
            "database": {
                "enabled": true,
                "cronExpression": "0 0 2 * * *",
                "keepLastAmount": 14
            }
        },
        "ffmpeg": {
            "accel": "disabled",
            "accelDecode": false,
            "acceptedAudioCodecs": ["aac", "mp3", "opus"],
            "acceptedContainers": ["mov", "ogg", "webm"],
            "acceptedVideoCodecs": ["h264"],
            "bframes": -1,
            "cqMode": "auto",
            "crf": 23,
            "gopSize": 0,
            "maxBitrate": "0",
            "preferredHwDevice": "auto",
            "preset": "ultrafast",
            "refs": 0,
            "targetAudioCodec": "aac",
            "targetResolution": "720",
            "targetVideoCodec": "h264",
            "temporalAQ": false,
            "threads": 0,
            "tonemap": "hable",
            "transcode": "required",
            "twoPass": false
        },
        "image": {
            "thumbnail": {
                "format": "webp",
                "size": 250,
                "quality": 80,
                "progressive": false
            },
            "preview": {
                "format": "jpeg",
                "size": 1440,
                "quality": 80,
                "progressive": false
            },
            "colorspace": "p3",
            "extractEmbedded": false,
            "fullsize": {
                "enabled": false,
                "format": "jpeg",
                "quality": 80,
                "progressive": false
            }
        },
        "job": {
            "backgroundTask": {"concurrency": 5},
            "editor": {"concurrency": 2},
            "faceDetection": {"concurrency": 2},
            "library": {"concurrency": 5},
            "metadataExtraction": {"concurrency": 5},
            "migration": {"concurrency": 5},
            "notifications": {"concurrency": 5},
            "ocr": {"concurrency": 1},
            "search": {"concurrency": 5},
            "sidecar": {"concurrency": 5},
            "smartSearch": {"concurrency": 2},
            "thumbnailGeneration": {"concurrency": 3},
            "videoConversion": {"concurrency": 1},
            "workflow": {"concurrency": 5}
        },
        "library": {
            "scan": {"enabled": true, "cronExpression": "0 0 0 * * *"},
            "watch": {"enabled": false}
        },
        "logging": {
            "enabled": true,
            "level": "log"
        },
        "machineLearning": {
            "enabled": true,
            "urls": ["http://immich-machine-learning:3003"],
            "availabilityChecks": {"enabled": true, "interval": 30000, "timeout": 2000},
            "clip": {"enabled": true, "modelName": "ViT-B-32__openai"},
            "duplicateDetection": {"enabled": true, "maxDistance": 0.01},
            "facialRecognition": {"enabled": true, "maxDistance": 0.5, "minFaces": 3, "minScore": 0.7, "modelName": "buffalo_l"},
            "ocr": {"enabled": true, "maxResolution": 736, "minDetectionScore": 0.5, "minRecognitionScore": 0.8, "modelName": "PP-OCRv5_mobile"}
        },
        "map": {
            "enabled": true,
            "lightStyle": "https://tiles.immich.cloud/v1/style/light.json",
            "darkStyle": "https://tiles.immich.cloud/v1/style/dark.json"
        },
        "metadata": {
            "faces": {"import": false}
        },
        "newVersionCheck": {"enabled": true},
        "nightlyTasks": {
            "startTime": "00:00",
            "databaseCleanup": true,
            "generateMemories": true,
            "syncQuotaUsage": true,
            "missingThumbnails": true,
            "clusterNewFaces": true
        },
        "notifications": {
            "smtp": {
                "enabled": false,
                "from": "",
                "replyTo": "",
                "transport": {
                    "ignoreCert": false,
                    "host": "",
                    "port": 587,
                    "secure": false,
                    "username": "",
                    "password": ""
                }
            }
        },
        "oauth": {
            "autoLaunch": false,
            "autoRegister": false,
            "buttonText": "Login with OAuth",
            "clientId": "",
            "clientSecret": "",
            "defaultStorageQuota": null,
            "enabled": false,
            "issuerUrl": "",
            "mobileOverrideEnabled": false,
            "mobileRedirectUri": "",
            "profileSigningAlgorithm": "none",
            "responseMode": "query",
            "responseType": "code",
            "scope": "openid email profile",
            "signingAlgorithm": "RS256",
            "storageLabelClaim": "",
            "storageQuotaClaim": "",
            "tokenEndpointAuthMethod": "client_secret_post"
        },
        "passwordLogin": {"enabled": true},
        "reverseGeocoding": {"enabled": true},
        "server": {
            "externalDomain": "",
            "loginPageMessage": "",
            "publicUsers": true
        },
        "storageTemplate": {
            "enabled": false,
            "hashVerificationEnabled": true,
            "template": "{{y}}/{{y}}-{{MM}}-{{dd}}/{{filename}}"
        },
        "templates": {
            "email": {
                "welcomeTemplate": "",
                "albumInviteTemplate": "",
                "albumUpdateTemplate": ""
            }
        },
        "theme": {"customCss": ""},
        "trash": {"enabled": true, "days": 30},
        "user": {"deleteDelay": 7}
    })
}

fn merge_json(base: Value, overlay: Value) -> Value {
    match (base, overlay) {
        (Value::Object(mut base_map), Value::Object(overlay_map)) => {
            for (key, overlay_value) in overlay_map {
                let base_value = base_map.remove(&key).unwrap_or(Value::Null);
                base_map.insert(key, merge_json(base_value, overlay_value));
            }
            Value::Object(base_map)
        }
        (_, Value::Null) => Value::Null,
        (base_value, Value::Array(overlay_array)) if overlay_array.is_empty() => base_value,
        (_, overlay_value) => overlay_value,
    }
}
