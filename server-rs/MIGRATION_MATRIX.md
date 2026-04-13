# Immich `server/src` -> `server-rs` Migration Matrix

This file is the working inventory for replacing the NestJS backend in
`/root/immich/server/src` with Rust in `/root/immich/server-rs`.

Current snapshot:
- Controllers in `server/src/controllers`: 64 files, 40 non-test controller surfaces
- Services in `server/src/services`: 95 files
- Repositories in `server/src/repositories`: 59 files
- DTO files in `server/src/dtos`: 50 files

Current Rust coverage:
- Implemented controllers:
  - `app`
  - `auth`
  - `server`
  - `system_metadata`
  - `user`
  - `album`
  - `asset`
  - `memory`
  - `notification`
  - `timeline`
  - `socket`
- Implemented DTO groups:
  - `auth`
  - `album`
  - `asset`
  - `server`
  - `system_metadata`
  - `user`
  - `memory`
  - `notification`
  - `timeline`

What “done” means for each module:
- Rust route exists with matching path/methods
- Request/response JSON shape matches Immich OpenAPI
- SQL respects migration rules:
  - UUID `SELECT`s cast to text
  - UUID `WHERE`/`INSERT` string binds cast with `::uuid`
  - singular table names only
- Frontend path is exercised against Rust without falling back to NestJS
- `cargo check` passes after the slice

## Priority Order

### Phase 1: Boot and Core UX
- `app.controller.ts`
- `auth.controller.ts`
- `server.controller.ts`
- `system-metadata.controller.ts`
- `user.controller.ts`
- `album.controller.ts`
- `asset.controller.ts`
- `asset-media.controller.ts`
- `memory.controller.ts`
- `notification.controller.ts`
- `timeline.controller.ts`

Status:
- Route-complete in Rust
- Remaining behavior gaps:
  - several low-traffic endpoints are compatibility stubs/no-ops, not full NestJS behavior yet
  - `timeline/bucket` still returns an empty compatibility payload
  - `asset-media` flows beyond upload/check are still shallow
  - album/memory/notification mutation endpoints exist but are not fully feature-complete
  - auth advanced flows (pin/session lock/admin sign-up/change-password) are present but not fully implemented

### Phase 2: Sessions, Sharing, Search, Library
- `session.controller.ts`
- `shared-link.controller.ts`
- `stack.controller.ts`
- `search.controller.ts`
- `library.controller.ts`
- `download.controller.ts`
- `trash.controller.ts`
- `map.controller.ts`
- `partner.controller.ts`
- `tag.controller.ts`
- `view.controller.ts`

Status:
- Route-complete in Rust
- Remaining behavior gaps:
  - most endpoints are compatibility stubs/no-ops
  - search/shared-link/session/library/download flows are not feature-complete yet
  - trash/map/partner/tag/view controllers exist but need real backing logic

### Phase 3: Admin and Operations
- `user-admin.controller.ts`
- `auth-admin.controller.ts`
- `notification-admin.controller.ts`
- `system-config.controller.ts`
- `maintenance.controller.ts`
- `queue.controller.ts`
- `job.controller.ts`
- `database-backup.controller.ts`
- `duplicate.controller.ts`
- `api-key.controller.ts`

Status:
- Route-complete in Rust
- Remaining behavior gaps:
  - most admin/ops endpoints are compatibility stubs/no-ops
  - system-config has basic persistence via `system_metadata`, but defaults/options/admin workflows are still shallow
  - queue/job/database-backup/duplicate/api-key/maintenance behavior is not yet feature-complete

### Phase 4: People, Faces, Activity, Sync, Workflow, Plugins
- `activity.controller.ts`
- `face.controller.ts`
- `person.controller.ts`
- `sync.controller.ts`
- `workflow.controller.ts`
- `plugin.controller.ts`
- `oauth.controller.ts`

Status:
- Route-complete in Rust
- Remaining behavior gaps:
  - most endpoints are compatibility stubs/no-ops
  - people/faces/activity/sync/workflow/plugin/oauth behavior is not feature-complete yet
  - media/file and stateful mutations in these modules still need real service/repository logic

## Source-to-Rust Controller Mapping

Implemented or partially implemented:
- `app.controller.ts` -> `src/controllers/app.rs`
- `auth.controller.ts` -> `src/controllers/auth.rs`
- `server.controller.ts` -> `src/controllers/server.rs`
- `system-metadata.controller.ts` -> `src/controllers/system_metadata.rs`
- `user.controller.ts` -> `src/controllers/user.rs`
- `album.controller.ts` -> `src/controllers/album.rs`
- `asset.controller.ts` + parts of `asset-media.controller.ts` -> `src/controllers/asset.rs`
- `memory.controller.ts` -> `src/controllers/memory.rs`
- `notification.controller.ts` -> `src/controllers/notification.rs`
- `timeline.controller.ts` -> `src/controllers/timeline.rs`
- `session.controller.ts` -> `src/controllers/session.rs`
- `shared-link.controller.ts` -> `src/controllers/shared_link.rs`
- `stack.controller.ts` -> `src/controllers/stack.rs`
- `search.controller.ts` -> `src/controllers/search.rs`
- `library.controller.ts` -> `src/controllers/library.rs`
- `download.controller.ts` -> `src/controllers/download.rs`
- `trash.controller.ts` -> `src/controllers/trash.rs`
- `map.controller.ts` -> `src/controllers/map.rs`
- `partner.controller.ts` -> `src/controllers/partner.rs`
- `tag.controller.ts` -> `src/controllers/tag.rs`
- `view.controller.ts` -> `src/controllers/view.rs`
- `user-admin.controller.ts` -> `src/controllers/user_admin.rs`
- `auth-admin.controller.ts` -> `src/controllers/auth_admin.rs`
- `notification-admin.controller.ts` -> `src/controllers/notification_admin.rs`
- `system-config.controller.ts` -> `src/controllers/system_config.rs`
- `maintenance.controller.ts` -> `src/controllers/maintenance.rs`
- `queue.controller.ts` -> `src/controllers/queue.rs`
- `job.controller.ts` -> `src/controllers/job.rs`
- `database-backup.controller.ts` -> `src/controllers/database_backup.rs`
- `duplicate.controller.ts` -> `src/controllers/duplicate.rs`
- `api-key.controller.ts` -> `src/controllers/api_key.rs`
- `activity.controller.ts` -> `src/controllers/activity.rs`
- `face.controller.ts` -> `src/controllers/face.rs`
- `person.controller.ts` -> `src/controllers/person.rs`
- `sync.controller.ts` -> `src/controllers/sync.rs`
- `workflow.controller.ts` -> `src/controllers/workflow.rs`
- `plugin.controller.ts` -> `src/controllers/plugin.rs`
- `oauth.controller.ts` -> `src/controllers/oauth.rs`

Missing:
- None at controller route layer

## Service / Repository Backlog

These NestJS services define the real behavior still to port:

Highest-value services next:
- `asset-media.service.ts`
- `session.service.ts`
- `shared-link.service.ts`
- `search.service.ts`
- `library.service.ts`
- `system-config.service.ts`
- `tag.service.ts`
- `trash.service.ts`
- `timeline.service.ts`
- `version.service.ts`

Key repositories to port or replace with direct Rust query modules:
- `asset.repository.ts`
- `album.repository.ts`
- `user.repository.ts`
- `session.repository.ts`
- `shared-link.repository.ts`
- `notification.repository.ts`
- `memory.repository.ts`
- `search.repository.ts`
- `system-metadata.repository.ts`
- `version-history.repository.ts`
- `storage.repository.ts`
- `server-info.repository.ts`

## DTO Coverage Backlog

Already represented in Rust:
- auth
- album
- asset
- memory
- notification
- server
- system-metadata
- timeline
- user

Still missing or partial:
- activity
- api-key
- database-backup
- download
- duplicate
- job
- library
- license
- maintenance
- map
- partner
- person
- plugin
- queue
- search
- session
- shared-link
- stack
- sync
- system-config
- tag
- trash
- workflow

## Execution Strategy

1. Finish all currently exposed frontend routes so the web app can fully run on Rust.
2. Port session/shared-link/search/library/download next because they unlock real user workflows.
3. Port admin/system-config/maintenance flows so `/custom.css`, server settings, and operations are fully Rust-owned.
4. Port people/activity/sync/workflow/plugin surfaces last because they have heavier background and integration behavior.

## Immediate Next Slice

The next concrete implementation wave should be:
- `session.controller.ts`
- `shared-link.controller.ts`
- `search.controller.ts`
- `download.controller.ts`
- `system-config.controller.ts`
- complete `asset-media.controller.ts`

That set removes the biggest remaining holes for real-world use and keeps the migration on the critical path to replacing NestJS entirely.

Next is behavior hardening, not more route creation.

Best order from here:

asset-media parity
real thumbnail/preview generation for images and videos
encoded video generation
asset_file lifecycle
original/preview/fullsize selection exactly like NestJS
remove remaining placeholder/fallback logic
system-config parity
real system-config defaults
full read/update behavior
storage-template options
make /custom.css, media settings, ffmpeg/image settings come from real config
session parity
create/update/delete/lock sessions correctly
current session detection
child sessions/casting behavior
pending sync reset handling
shared-link parity
create/read/update/delete shared links
password login flow
cookie/token handling
add/remove assets
shared album/individual link behavior
search parity
metadata search
smart search
random/large-assets
places/cities/suggestions
explore data
library + storage parity
external libraries
validation
scanning
statistics
proper “cannot use upload folder” checks
download + trash
archive/download info
restore/delete/empty trash
make asset lifecycle feel real
people / faces / activity
real DB-backed mutations
thumbnails
merge/reassign flows
comments/likes/statistics
sync
stream/ack checkpoints
delta sync/full sync behavior
mobile sync correctness
Admin/ops modules
queue
job
duplicate
api-key
maintenance
database-backup
If the goal is “daily usable Immich,” I’d do this exact sprint next:

finish asset-media
finish system-config
finish session
finish shared-link
finish search
That gets you from “UI mostly loads” to “real product behavior.” If you want, I’ll start with asset-media parity and work straight through that stack.