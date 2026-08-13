use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use pixiv_client_auth::ClientRequestSignature;
use reqwest::blocking::Client;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fmt;
use std::io::Read;
use url::Url;

pub const API_HOST: &str = "app-api.pixiv.net";
pub const MEDIA_REFERER: &str = "https://app-api.pixiv.net/";
pub const MEDIA_USER_AGENT: &str = "PixivIOSApp/5.8.0";

const RECOMMENDED_PATH: &str = "/v1/illust/recommended";
const ILLUSTRATION_DETAIL_PATH: &str = "/v1/illust/detail";
const ILLUSTRATION_SERIES_PATH: &str = "/v1/illust/series";
const RELATED_ILLUSTRATIONS_PATH: &str = "/v2/illust/related";
const USER_DETAIL_PATH: &str = "/v1/user/detail";
const USER_ILLUSTRATIONS_PATH: &str = "/v1/user/illusts";
const RANKING_PATH: &str = "/v1/illust/ranking";
const TRENDING_TAGS_PATH: &str = "/v1/trending-tags/illust";
const SEARCH_ILLUSTRATIONS_PATH: &str = "/v1/search/illust";
const SEARCH_USERS_PATH: &str = "/v1/search/user";
const USER_FOLLOWING_PATH: &str = "/v1/user/following";
const FOLLOWED_ILLUSTRATIONS_PATH: &str = "/v2/illust/follow";
const BOOKMARKED_ILLUSTRATIONS_PATH: &str = "/v1/user/bookmarks/illust";
const ILLUSTRATION_BOOKMARK_DETAIL_PATH: &str = "/v2/illust/bookmark/detail";
const ILLUSTRATION_BOOKMARK_TAGS_PATH: &str = "/v1/user/bookmark-tags/illust";
const BOOKMARK_ADD_PATH: &str = "/v2/illust/bookmark/add";
const BOOKMARK_DELETE_PATH: &str = "/v1/illust/bookmark/delete";
const FOLLOW_ADD_PATH: &str = "/v1/user/follow/add";
const FOLLOW_DELETE_PATH: &str = "/v1/user/follow/delete";
const ILLUSTRATION_COMMENTS_PATH: &str = "/v3/illust/comments";
const COMMENT_REPLIES_PATH: &str = "/v2/illust/comment/replies";
const COMMENT_ADD_PATH: &str = "/v1/illust/comment/add";
const COMMENT_DELETE_PATH: &str = "/v1/illust/comment/delete";
const COMMENT_STAMPS_PATH: &str = "/v1/stamps";
const NOTIFICATION_LIST_PATH: &str = "/v1/notification/list";
const NOTIFICATION_VIEW_MORE_PATH: &str = "/v1/notification/view-more";
const NOTIFICATION_PAGE_LIMIT: &str = "30";
const ACCESS_BLOCK_USERS_PATH: &str = "/v1/access-block/users";
const ACCESS_BLOCK_ADD_PATH: &str = "/v1/access-block/user/add";
const ACCESS_BLOCK_DELETE_PATH: &str = "/v1/access-block/user/delete";
const MUTE_LIST_PATH: &str = "/v1/mute/list";
const MUTE_EDIT_PATH: &str = "/v1/mute/edit";
const MANGA_RECOMMENDED_PATH: &str = "/v1/illust/recommended";
const NOVEL_RECOMMENDED_PATH: &str = "/v1/novel/recommended";
const NOVEL_DETAIL_PATH: &str = "/v2/novel/detail";
const NOVEL_SERIES_PATH: &str = "/v2/novel/series";
const NOVEL_WEBVIEW_PATH: &str = "/webview/v2/novel";
const SEARCH_NOVELS_PATH: &str = "/v1/search/novel";
const USER_NOVELS_PATH: &str = "/v1/user/novels";
const FOLLOWED_NOVELS_PATH: &str = "/v1/novel/follow";
const BOOKMARKED_NOVELS_PATH: &str = "/v1/user/bookmarks/novel";
const NOVEL_BOOKMARK_DETAIL_PATH: &str = "/v2/novel/bookmark/detail";
const NOVEL_BOOKMARK_TAGS_PATH: &str = "/v1/user/bookmark-tags/novel";
const NOVEL_RANKING_PATH: &str = "/v1/novel/ranking";
const NOVEL_BOOKMARK_ADD_PATH: &str = "/v2/novel/bookmark/add";
const NOVEL_BOOKMARK_DELETE_PATH: &str = "/v1/novel/bookmark/delete";
const NOVEL_COMMENTS_PATH: &str = "/v3/novel/comments";
const NOVEL_COMMENT_REPLIES_PATH: &str = "/v2/novel/comment/replies";
const NOVEL_COMMENT_ADD_PATH: &str = "/v1/novel/comment/add";
const NOVEL_COMMENT_DELETE_PATH: &str = "/v1/novel/comment/delete";
const UGOIRA_METADATA_PATH: &str = "/v1/ugoira/metadata";
const APP_VERSION: &str = "5.0.166";
const USER_AGENT: &str = "PixivAndroidApp/5.0.166 (Android 13; PixivClient)";
const MAX_CURSOR_BYTES: usize = 4096;
const MAX_ERROR_BODY_BYTES: usize = 64 * 1024;
const MAX_NOVEL_HTML_BYTES: usize = 32 * 1024 * 1024;
const MAX_BOOKMARK_TAGS: usize = 10;
const MAX_BOOKMARK_TAG_BYTES: usize = 300;

pub struct PixivApiClient {
    http: Client,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BookmarkContentKind {
    Illustration,
    Novel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkTagStatus {
    pub name: String,
    pub is_registered: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkDetail {
    pub restrict: String,
    pub tags: Vec<BookmarkTagStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkTag {
    pub name: String,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkTagPage {
    pub tags: Vec<BookmarkTag>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkUpdate {
    pub kind: BookmarkContentKind,
    pub resource_id: String,
    pub bookmarked: bool,
    pub restrict: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BookmarkUpdateFailure {
    AuthenticationRequired,
    InvalidInput,
    RequestFailed,
    Rejected,
    InvalidResponse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkUpdateResult {
    pub kind: BookmarkContentKind,
    pub resource_id: String,
    pub succeeded: bool,
    pub failure: Option<BookmarkUpdateFailure>,
}

impl PixivApiClient {
    pub fn with_http(http: Client) -> Self {
        Self { http }
    }

    pub fn recommended_illustrations(
        &self,
        access_token: &str,
        cursor: Option<&str>,
        signature: &ClientRequestSignature,
    ) -> Result<IllustrationPage, ApiError> {
        let url = recommended_url(cursor)?;
        let envelope: IllustrationListEnvelope = self.get_json(access_token, url, signature)?;
        page_from_envelope(envelope, RECOMMENDED_PATH, &[])
    }

    pub fn recommended_manga(
        &self,
        access_token: &str,
        cursor: Option<&str>,
        signature: &ClientRequestSignature,
    ) -> Result<IllustrationPage, ApiError> {
        let bindings = [("content_type", "manga")];
        let url = match cursor {
            Some(cursor) => decode_cursor(cursor, MANGA_RECOMMENDED_PATH, &bindings)?,
            None => endpoint_url(
                MANGA_RECOMMENDED_PATH,
                &[
                    ("content_type", "manga"),
                    ("filter", "for_ios"),
                    ("include_ranking_label", "true"),
                ],
            )?,
        };
        let envelope: IllustrationListEnvelope = self.get_json(access_token, url, signature)?;
        page_from_envelope(envelope, MANGA_RECOMMENDED_PATH, &bindings)
    }

    pub fn notifications(
        &self,
        access_token: &str,
        cursor: Option<&str>,
        signature: &ClientRequestSignature,
    ) -> Result<NotificationPage, ApiError> {
        let bindings = [("limit", NOTIFICATION_PAGE_LIMIT)];
        let url = match cursor {
            Some(cursor) => decode_cursor(cursor, NOTIFICATION_LIST_PATH, &bindings)?,
            None => endpoint_url(NOTIFICATION_LIST_PATH, &bindings)?,
        };
        let envelope: NotificationsEnvelope = self.get_json(access_token, url, signature)?;
        notification_page_from_envelope_for(envelope, NOTIFICATION_LIST_PATH, &bindings)
    }

    pub fn notification_view_more(
        &self,
        access_token: &str,
        notification_id: &str,
        cursor: Option<&str>,
        signature: &ClientRequestSignature,
    ) -> Result<NotificationPage, ApiError> {
        let notification_id = normalized_resource_id(notification_id)?;
        let bindings = [
            ("notification_id", notification_id.as_str()),
            ("limit", NOTIFICATION_PAGE_LIMIT),
        ];
        let url = match cursor {
            Some(cursor) => decode_cursor(cursor, NOTIFICATION_VIEW_MORE_PATH, &bindings)?,
            None => endpoint_url(NOTIFICATION_VIEW_MORE_PATH, &bindings)?,
        };
        let envelope: NotificationsEnvelope = self.get_json(access_token, url, signature)?;
        notification_page_from_envelope_for(envelope, NOTIFICATION_VIEW_MORE_PATH, &bindings)
    }

    pub fn access_blocked_users(
        &self,
        access_token: &str,
        cursor: Option<&str>,
        signature: &ClientRequestSignature,
    ) -> Result<AccessBlockPage, ApiError> {
        let url = match cursor {
            Some(cursor) => decode_cursor(cursor, ACCESS_BLOCK_USERS_PATH, &[])?,
            None => endpoint_url(ACCESS_BLOCK_USERS_PATH, &[])?,
        };
        let envelope: AccessBlockEnvelope = self.get_json(access_token, url, signature)?;
        access_block_page_from_envelope(envelope)
    }

    pub fn add_access_block(
        &self,
        access_token: &str,
        user_id: &str,
        signature: &ClientRequestSignature,
    ) -> Result<(), ApiError> {
        let user_id = normalized_resource_id(user_id)?;
        self.post_form_unit(
            access_token,
            endpoint_url(ACCESS_BLOCK_ADD_PATH, &[])?,
            &[("user_id", user_id.as_str())],
            signature,
        )
    }

    pub fn delete_access_block(
        &self,
        access_token: &str,
        user_id: &str,
        signature: &ClientRequestSignature,
    ) -> Result<(), ApiError> {
        let user_id = normalized_resource_id(user_id)?;
        self.post_form_unit(
            access_token,
            endpoint_url(ACCESS_BLOCK_DELETE_PATH, &[])?,
            &[("user_id", user_id.as_str())],
            signature,
        )
    }

    pub fn mute_settings(
        &self,
        access_token: &str,
        signature: &ClientRequestSignature,
    ) -> Result<MuteSettings, ApiError> {
        let envelope: MuteSettingsEnvelope =
            self.get_json(access_token, endpoint_url(MUTE_LIST_PATH, &[])?, signature)?;
        mute_settings_from_envelope(envelope)
    }

    pub fn edit_user_mute(
        &self,
        access_token: &str,
        user_id: &str,
        muted: bool,
        signature: &ClientRequestSignature,
    ) -> Result<(), ApiError> {
        let user_id = normalized_resource_id(user_id)?;
        let field = if muted {
            "add_user_ids[]"
        } else {
            "delete_user_ids[]"
        };
        self.post_form_unit(
            access_token,
            endpoint_url(MUTE_EDIT_PATH, &[])?,
            &[(field, user_id.as_str())],
            signature,
        )
    }

    pub fn edit_tag_mute(
        &self,
        access_token: &str,
        tag: &str,
        muted: bool,
        signature: &ClientRequestSignature,
    ) -> Result<(), ApiError> {
        let tag = normalized_mute_tag(tag)?;
        let field = if muted { "add_tags[]" } else { "delete_tags[]" };
        self.post_form_unit(
            access_token,
            endpoint_url(MUTE_EDIT_PATH, &[])?,
            &[(field, tag.as_str())],
            signature,
        )
    }

    pub fn recommended_novels(
        &self,
        access_token: &str,
        cursor: Option<&str>,
        signature: &ClientRequestSignature,
    ) -> Result<NovelPage, ApiError> {
        let url = match cursor {
            Some(cursor) => decode_cursor(cursor, NOVEL_RECOMMENDED_PATH, &[])?,
            None => endpoint_url(
                NOVEL_RECOMMENDED_PATH,
                &[("filter", "for_ios"), ("include_ranking_label", "true")],
            )?,
        };
        let envelope: NovelListEnvelope = self.get_json(access_token, url, signature)?;
        novel_page_from_envelope(envelope, NOVEL_RECOMMENDED_PATH, &[])
    }

    pub fn novel_detail(
        &self,
        access_token: &str,
        novel_id: &str,
        signature: &ClientRequestSignature,
    ) -> Result<NovelDetail, ApiError> {
        let novel_id = normalized_resource_id(novel_id)?;
        let url = endpoint_url(NOVEL_DETAIL_PATH, &[("novel_id", novel_id.as_str())])?;
        let envelope: NovelDetailEnvelope = self.get_json(access_token, url, signature)?;
        NovelDetail::from_payload(envelope.novel)
    }

    pub fn novel_series(
        &self,
        access_token: &str,
        series_id: &str,
        cursor: Option<&str>,
        signature: &ClientRequestSignature,
    ) -> Result<NovelSeriesPage, ApiError> {
        let series_id = normalized_resource_id(series_id)?;
        let bindings = [("series_id", series_id.as_str())];
        let url = match cursor {
            Some(cursor) => decode_cursor(cursor, NOVEL_SERIES_PATH, &bindings)?,
            None => endpoint_url(
                NOVEL_SERIES_PATH,
                &[("series_id", series_id.as_str()), ("filter", "for_ios")],
            )?,
        };
        let envelope: NovelSeriesEnvelope = self.get_json(access_token, url, signature)?;
        NovelSeriesPage::from_envelope(envelope, &series_id)
    }

    pub fn novel_content(
        &self,
        access_token: &str,
        novel_id: &str,
        signature: &ClientRequestSignature,
    ) -> Result<NovelContent, ApiError> {
        let novel_id = normalized_resource_id(novel_id)?;
        let url = endpoint_url(
            NOVEL_WEBVIEW_PATH,
            &[("id", novel_id.as_str()), ("viewer_version", "20221031_ai")],
        )?;
        let html = self.get_text(access_token, url, signature, MAX_NOVEL_HTML_BYTES)?;
        let json = extract_embedded_novel_json(&html).ok_or(ApiError::InvalidResponse)?;
        let payload: NovelContentPayload =
            serde_json::from_str(json).map_err(|_| ApiError::InvalidResponse)?;
        NovelContent::from_payload(payload, &novel_id)
    }

    pub fn search_novels(
        &self,
        access_token: &str,
        word: &str,
        search_target: &str,
        cursor: Option<&str>,
        signature: &ClientRequestSignature,
    ) -> Result<NovelPage, ApiError> {
        let word = normalized_search_word(word)?;
        let search_target = normalized_search_target(search_target)?;
        let bindings = [("word", word.as_str()), ("search_target", search_target)];
        let url = match cursor {
            Some(cursor) => decode_cursor(cursor, SEARCH_NOVELS_PATH, &bindings)?,
            None => endpoint_url(
                SEARCH_NOVELS_PATH,
                &[
                    ("word", word.as_str()),
                    ("search_target", search_target),
                    ("sort", "date_desc"),
                    ("filter", "for_ios"),
                    ("merge_plain_keyword_results", "true"),
                    ("include_translated_tag_results", "true"),
                ],
            )?,
        };
        let envelope: NovelListEnvelope = self.get_json(access_token, url, signature)?;
        novel_page_from_envelope(envelope, SEARCH_NOVELS_PATH, &bindings)
    }

    pub fn user_novels(
        &self,
        access_token: &str,
        user_id: &str,
        cursor: Option<&str>,
        signature: &ClientRequestSignature,
    ) -> Result<NovelPage, ApiError> {
        let user_id = normalized_resource_id(user_id)?;
        let bindings = [("user_id", user_id.as_str())];
        let url = match cursor {
            Some(cursor) => decode_cursor(cursor, USER_NOVELS_PATH, &bindings)?,
            None => endpoint_url(
                USER_NOVELS_PATH,
                &[("user_id", user_id.as_str()), ("filter", "for_ios")],
            )?,
        };
        let envelope: NovelListEnvelope = self.get_json(access_token, url, signature)?;
        novel_page_from_envelope(envelope, USER_NOVELS_PATH, &bindings)
    }

    pub fn followed_novels(
        &self,
        access_token: &str,
        cursor: Option<&str>,
        signature: &ClientRequestSignature,
    ) -> Result<NovelPage, ApiError> {
        let bindings = [("restrict", "public")];
        let url = match cursor {
            Some(cursor) => decode_cursor(cursor, FOLLOWED_NOVELS_PATH, &bindings)?,
            None => endpoint_url(FOLLOWED_NOVELS_PATH, &bindings)?,
        };
        let envelope: NovelListEnvelope = self.get_json(access_token, url, signature)?;
        novel_page_from_envelope(envelope, FOLLOWED_NOVELS_PATH, &bindings)
    }

    pub fn bookmarked_novels(
        &self,
        access_token: &str,
        user_id: &str,
        restrict: &str,
        tag: Option<&str>,
        cursor: Option<&str>,
        signature: &ClientRequestSignature,
    ) -> Result<NovelPage, ApiError> {
        let user_id = normalized_resource_id(user_id)?;
        let restrict = normalized_bookmark_restrict(restrict)?;
        let tag = tag
            .map(|tag| normalized_bookmark_tags(&[tag.to_owned()]))
            .transpose()?
            .and_then(|tags| tags.into_iter().next());
        let mut bindings = vec![("user_id", user_id.as_str()), ("restrict", restrict)];
        if let Some(tag) = tag.as_deref() {
            bindings.push(("tag", tag));
        }
        let url = match cursor {
            Some(cursor) => decode_cursor(cursor, BOOKMARKED_NOVELS_PATH, &bindings)?,
            None => endpoint_url(BOOKMARKED_NOVELS_PATH, &{
                let mut query = bindings.clone();
                query.push(("filter", "for_ios"));
                query
            })?,
        };
        let envelope: NovelListEnvelope = self.get_json(access_token, url, signature)?;
        novel_page_from_envelope(envelope, BOOKMARKED_NOVELS_PATH, &bindings)
    }

    pub fn bookmark_detail(
        &self,
        access_token: &str,
        kind: BookmarkContentKind,
        resource_id: &str,
        signature: &ClientRequestSignature,
    ) -> Result<BookmarkDetail, ApiError> {
        let resource_id = normalized_resource_id(resource_id)?;
        let (path, parameter) = match kind {
            BookmarkContentKind::Illustration => (ILLUSTRATION_BOOKMARK_DETAIL_PATH, "illust_id"),
            BookmarkContentKind::Novel => (NOVEL_BOOKMARK_DETAIL_PATH, "novel_id"),
        };
        let url = endpoint_url(path, &[(parameter, resource_id.as_str())])?;
        let envelope: BookmarkDetailEnvelope = self.get_json(access_token, url, signature)?;
        bookmark_detail_from_envelope(envelope)
    }

    pub fn bookmark_tags(
        &self,
        access_token: &str,
        user_id: &str,
        kind: BookmarkContentKind,
        restrict: &str,
        cursor: Option<&str>,
        signature: &ClientRequestSignature,
    ) -> Result<BookmarkTagPage, ApiError> {
        let user_id = normalized_resource_id(user_id)?;
        let restrict = normalized_bookmark_restrict(restrict)?;
        let path = match kind {
            BookmarkContentKind::Illustration => ILLUSTRATION_BOOKMARK_TAGS_PATH,
            BookmarkContentKind::Novel => NOVEL_BOOKMARK_TAGS_PATH,
        };
        let bindings = [("user_id", user_id.as_str()), ("restrict", restrict)];
        let url = match cursor {
            Some(cursor) => decode_cursor(cursor, path, &bindings)?,
            None => endpoint_url(path, &bindings)?,
        };
        let envelope: BookmarkTagsEnvelope = self.get_json(access_token, url, signature)?;
        bookmark_tag_page_from_envelope(envelope, path, &bindings)
    }

    pub fn update_bookmark(
        &self,
        access_token: &str,
        update: &BookmarkUpdate,
        signature: &ClientRequestSignature,
    ) -> Result<(), ApiError> {
        if !update.bookmarked {
            return match update.kind {
                BookmarkContentKind::Illustration => {
                    self.delete_illustration_bookmark(access_token, &update.resource_id, signature)
                }
                BookmarkContentKind::Novel => {
                    self.delete_novel_bookmark(access_token, &update.resource_id, signature)
                }
            };
        }
        let (path, parameter, resource_id, restrict, tags) = bookmark_update_parts(update)?;
        let url = endpoint_url(path, &[])?;
        let mut form = vec![(parameter, resource_id.as_str()), ("restrict", restrict)];
        form.extend(tags.iter().map(|tag| ("tags[]", tag.as_str())));
        self.post_form_unit(access_token, url, &form, signature)
    }

    pub fn batch_update_bookmarks(
        &self,
        access_token: &str,
        updates: &[BookmarkUpdate],
        signature: &ClientRequestSignature,
    ) -> Result<Vec<BookmarkUpdateResult>, ApiError> {
        if updates.is_empty() || updates.len() > 100 {
            return Err(ApiError::InvalidInput);
        }
        let mut results = Vec::with_capacity(updates.len());
        for update in updates {
            let result = self.update_bookmark(access_token, update, signature);
            results.push(BookmarkUpdateResult {
                kind: update.kind,
                resource_id: update.resource_id.clone(),
                succeeded: result.is_ok(),
                failure: result.err().map(bookmark_update_failure),
            });
        }
        Ok(results)
    }

    pub fn ranking_novels(
        &self,
        access_token: &str,
        ranking_mode: &str,
        cursor: Option<&str>,
        signature: &ClientRequestSignature,
    ) -> Result<NovelPage, ApiError> {
        let ranking_mode = normalized_ranking_mode(ranking_mode)?;
        let bindings = [("mode", ranking_mode)];
        let url = match cursor {
            Some(cursor) => decode_cursor(cursor, NOVEL_RANKING_PATH, &bindings)?,
            None => endpoint_url(
                NOVEL_RANKING_PATH,
                &[("mode", ranking_mode), ("filter", "for_ios")],
            )?,
        };
        let envelope: NovelListEnvelope = self.get_json(access_token, url, signature)?;
        novel_page_from_envelope(envelope, NOVEL_RANKING_PATH, &bindings)
    }

    pub fn add_novel_bookmark(
        &self,
        access_token: &str,
        novel_id: &str,
        restrict: &str,
        signature: &ClientRequestSignature,
    ) -> Result<(), ApiError> {
        self.update_bookmark(
            access_token,
            &BookmarkUpdate {
                kind: BookmarkContentKind::Novel,
                resource_id: novel_id.to_owned(),
                bookmarked: true,
                restrict: restrict.to_owned(),
                tags: Vec::new(),
            },
            signature,
        )
    }

    pub fn delete_novel_bookmark(
        &self,
        access_token: &str,
        novel_id: &str,
        signature: &ClientRequestSignature,
    ) -> Result<(), ApiError> {
        let novel_id = normalized_resource_id(novel_id)?;
        let url = endpoint_url(NOVEL_BOOKMARK_DELETE_PATH, &[])?;
        self.post_form_unit(
            access_token,
            url,
            &[("novel_id", novel_id.as_str())],
            signature,
        )
    }

    pub fn novel_comments(
        &self,
        access_token: &str,
        novel_id: &str,
        cursor: Option<&str>,
        signature: &ClientRequestSignature,
    ) -> Result<CommentPage, ApiError> {
        let novel_id = normalized_resource_id(novel_id)?;
        let bindings = [("novel_id", novel_id.as_str())];
        let url = match cursor {
            Some(cursor) => decode_cursor(cursor, NOVEL_COMMENTS_PATH, &bindings)?,
            None => endpoint_url(NOVEL_COMMENTS_PATH, &bindings)?,
        };
        let envelope: CommentsEnvelope = self.get_json(access_token, url, signature)?;
        comment_page_from_envelope(envelope, NOVEL_COMMENTS_PATH, &bindings)
    }

    pub fn novel_comment_replies(
        &self,
        access_token: &str,
        comment_id: &str,
        cursor: Option<&str>,
        signature: &ClientRequestSignature,
    ) -> Result<CommentPage, ApiError> {
        let comment_id = normalized_resource_id(comment_id)?;
        let bindings = [("comment_id", comment_id.as_str())];
        let url = match cursor {
            Some(cursor) => decode_cursor(cursor, NOVEL_COMMENT_REPLIES_PATH, &bindings)?,
            None => endpoint_url(NOVEL_COMMENT_REPLIES_PATH, &bindings)?,
        };
        let envelope: CommentsEnvelope = self.get_json(access_token, url, signature)?;
        comment_page_from_envelope(envelope, NOVEL_COMMENT_REPLIES_PATH, &bindings)
    }

    pub fn add_novel_comment(
        &self,
        access_token: &str,
        novel_id: &str,
        comment: &str,
        parent_comment_id: Option<&str>,
        stamp_id: Option<&str>,
        signature: &ClientRequestSignature,
    ) -> Result<Comment, ApiError> {
        let novel_id = normalized_resource_id(novel_id)?;
        let (comment, stamp_id) = normalized_comment_submission(comment, stamp_id)?;
        let parent_comment_id = parent_comment_id.map(normalized_resource_id).transpose()?;
        let mut form = vec![
            ("novel_id", novel_id.as_str()),
            ("comment", comment.as_str()),
        ];
        if let Some(parent_comment_id) = parent_comment_id.as_deref() {
            form.push(("parent_comment_id", parent_comment_id));
        }
        if let Some(stamp_id) = stamp_id.as_deref() {
            form.push(("stamp_id", stamp_id));
        }
        let url = endpoint_url(NOVEL_COMMENT_ADD_PATH, &[])?;
        let envelope: CommentAddEnvelope =
            self.post_form_json(access_token, url, &form, signature)?;
        Comment::from_payload(&envelope.comment).ok_or(ApiError::InvalidResponse)
    }

    pub fn delete_novel_comment(
        &self,
        access_token: &str,
        comment_id: &str,
        signature: &ClientRequestSignature,
    ) -> Result<(), ApiError> {
        let comment_id = normalized_resource_id(comment_id)?;
        let url = endpoint_url(NOVEL_COMMENT_DELETE_PATH, &[])?;
        self.post_form_unit(
            access_token,
            url,
            &[("comment_id", comment_id.as_str())],
            signature,
        )
    }

    pub fn ugoira_metadata(
        &self,
        access_token: &str,
        illustration_id: &str,
        signature: &ClientRequestSignature,
    ) -> Result<UgoiraMetadata, ApiError> {
        let illustration_id = normalized_resource_id(illustration_id)?;
        let url = endpoint_url(
            UGOIRA_METADATA_PATH,
            &[("illust_id", illustration_id.as_str())],
        )?;
        let envelope: UgoiraEnvelope = self.get_json(access_token, url, signature)?;
        UgoiraMetadata::from_payload(envelope.ugoira_metadata)
    }

    pub fn illustration_detail(
        &self,
        access_token: &str,
        illustration_id: &str,
        signature: &ClientRequestSignature,
    ) -> Result<IllustrationDetail, ApiError> {
        let illustration_id = normalized_resource_id(illustration_id)?;
        let url = endpoint_url(
            ILLUSTRATION_DETAIL_PATH,
            &[("illust_id", illustration_id.as_str())],
        )?;
        let envelope: IllustrationDetailEnvelope = self.get_json(access_token, url, signature)?;
        IllustrationDetail::from_payload(envelope.illust)
    }

    pub fn illustration_series(
        &self,
        access_token: &str,
        series_id: &str,
        cursor: Option<&str>,
        signature: &ClientRequestSignature,
    ) -> Result<IllustrationSeriesPage, ApiError> {
        let series_id = normalized_resource_id(series_id)?;
        let bindings = [("illust_series_id", series_id.as_str())];
        let url = match cursor {
            Some(cursor) => decode_cursor(cursor, ILLUSTRATION_SERIES_PATH, &bindings)?,
            None => endpoint_url(
                ILLUSTRATION_SERIES_PATH,
                &[
                    ("illust_series_id", series_id.as_str()),
                    ("filter", "for_ios"),
                ],
            )?,
        };
        let envelope: IllustrationSeriesEnvelope = self.get_json(access_token, url, signature)?;
        IllustrationSeriesPage::from_envelope(envelope, &series_id)
    }

    pub fn related_illustrations(
        &self,
        access_token: &str,
        illustration_id: &str,
        cursor: Option<&str>,
        signature: &ClientRequestSignature,
    ) -> Result<IllustrationPage, ApiError> {
        let illustration_id = normalized_resource_id(illustration_id)?;
        let bindings = [("illust_id", illustration_id.as_str())];
        let url = match cursor {
            Some(cursor) => decode_cursor(cursor, RELATED_ILLUSTRATIONS_PATH, &bindings)?,
            None => endpoint_url(
                RELATED_ILLUSTRATIONS_PATH,
                &[
                    ("illust_id", illustration_id.as_str()),
                    ("filter", "for_ios"),
                ],
            )?,
        };
        let envelope: IllustrationListEnvelope = self.get_json(access_token, url, signature)?;
        page_from_envelope(envelope, RELATED_ILLUSTRATIONS_PATH, &bindings)
    }

    pub fn user_detail(
        &self,
        access_token: &str,
        user_id: &str,
        signature: &ClientRequestSignature,
    ) -> Result<UserDetail, ApiError> {
        let user_id = normalized_resource_id(user_id)?;
        let url = endpoint_url(
            USER_DETAIL_PATH,
            &[("user_id", user_id.as_str()), ("filter", "for_ios")],
        )?;
        let envelope: UserDetailEnvelope = self.get_json(access_token, url, signature)?;
        UserDetail::from_envelope(envelope)
    }

    pub fn user_illustrations(
        &self,
        access_token: &str,
        user_id: &str,
        work_kind: &str,
        cursor: Option<&str>,
        signature: &ClientRequestSignature,
    ) -> Result<IllustrationPage, ApiError> {
        let user_id = normalized_resource_id(user_id)?;
        let work_kind = normalized_work_kind(work_kind)?;
        let bindings = [("user_id", user_id.as_str()), ("type", work_kind)];
        let url = match cursor {
            Some(cursor) => decode_cursor(cursor, USER_ILLUSTRATIONS_PATH, &bindings)?,
            None => endpoint_url(
                USER_ILLUSTRATIONS_PATH,
                &[
                    ("user_id", user_id.as_str()),
                    ("type", work_kind),
                    ("filter", "for_ios"),
                ],
            )?,
        };
        let envelope: IllustrationListEnvelope = self.get_json(access_token, url, signature)?;
        page_from_envelope(envelope, USER_ILLUSTRATIONS_PATH, &bindings)
    }

    pub fn ranking_illustrations(
        &self,
        access_token: &str,
        ranking_mode: &str,
        cursor: Option<&str>,
        signature: &ClientRequestSignature,
    ) -> Result<IllustrationPage, ApiError> {
        let ranking_mode = normalized_ranking_mode(ranking_mode)?;
        let bindings = [("mode", ranking_mode)];
        let url = match cursor {
            Some(cursor) => decode_cursor(cursor, RANKING_PATH, &bindings)?,
            None => endpoint_url(
                RANKING_PATH,
                &[("mode", ranking_mode), ("filter", "for_ios")],
            )?,
        };
        let envelope: IllustrationListEnvelope = self.get_json(access_token, url, signature)?;
        page_from_envelope(envelope, RANKING_PATH, &bindings)
    }

    pub fn trending_tags(
        &self,
        access_token: &str,
        signature: &ClientRequestSignature,
    ) -> Result<Vec<TrendingTag>, ApiError> {
        let url = endpoint_url(TRENDING_TAGS_PATH, &[("filter", "for_ios")])?;
        let envelope: TrendingTagsEnvelope = self.get_json(access_token, url, signature)?;
        Ok(envelope
            .trend_tags
            .iter()
            .filter_map(TrendingTag::from_payload)
            .collect())
    }

    pub fn search_illustrations(
        &self,
        access_token: &str,
        word: &str,
        search_target: &str,
        cursor: Option<&str>,
        signature: &ClientRequestSignature,
    ) -> Result<IllustrationPage, ApiError> {
        let word = normalized_search_word(word)?;
        let search_target = normalized_search_target(search_target)?;
        let bindings = [("word", word.as_str()), ("search_target", search_target)];
        let url = match cursor {
            Some(cursor) => decode_cursor(cursor, SEARCH_ILLUSTRATIONS_PATH, &bindings)?,
            None => endpoint_url(
                SEARCH_ILLUSTRATIONS_PATH,
                &[
                    ("word", word.as_str()),
                    ("search_target", search_target),
                    ("sort", "date_desc"),
                    ("filter", "for_ios"),
                    ("merge_plain_keyword_results", "true"),
                ],
            )?,
        };
        let envelope: IllustrationListEnvelope = self.get_json(access_token, url, signature)?;
        page_from_envelope(envelope, SEARCH_ILLUSTRATIONS_PATH, &bindings)
    }

    pub fn search_users(
        &self,
        access_token: &str,
        word: &str,
        cursor: Option<&str>,
        signature: &ClientRequestSignature,
    ) -> Result<UserPreviewPage, ApiError> {
        let word = normalized_search_word(word)?;
        let bindings = [("word", word.as_str())];
        let url = match cursor {
            Some(cursor) => decode_cursor(cursor, SEARCH_USERS_PATH, &bindings)?,
            None => endpoint_url(
                SEARCH_USERS_PATH,
                &[("word", word.as_str()), ("filter", "for_ios")],
            )?,
        };
        let envelope: UserPreviewEnvelope = self.get_json(access_token, url, signature)?;
        user_preview_page_from_envelope(envelope, SEARCH_USERS_PATH, &bindings)
    }

    pub fn followed_users(
        &self,
        access_token: &str,
        user_id: &str,
        restrict: &str,
        cursor: Option<&str>,
        signature: &ClientRequestSignature,
    ) -> Result<UserPreviewPage, ApiError> {
        let user_id = normalized_resource_id(user_id)?;
        let restrict = normalized_bookmark_restrict(restrict)?;
        let bindings = [("user_id", user_id.as_str()), ("restrict", restrict)];
        let url = match cursor {
            Some(cursor) => decode_cursor(cursor, USER_FOLLOWING_PATH, &bindings)?,
            None => endpoint_url(
                USER_FOLLOWING_PATH,
                &[
                    ("user_id", user_id.as_str()),
                    ("restrict", restrict),
                    ("filter", "for_ios"),
                ],
            )?,
        };
        let envelope: UserPreviewEnvelope = self.get_json(access_token, url, signature)?;
        user_preview_page_from_envelope(envelope, USER_FOLLOWING_PATH, &bindings)
    }

    pub fn followed_illustrations(
        &self,
        access_token: &str,
        cursor: Option<&str>,
        signature: &ClientRequestSignature,
    ) -> Result<IllustrationPage, ApiError> {
        let bindings = [("restrict", "all")];
        let url = match cursor {
            Some(cursor) => decode_cursor(cursor, FOLLOWED_ILLUSTRATIONS_PATH, &bindings)?,
            None => endpoint_url(FOLLOWED_ILLUSTRATIONS_PATH, &bindings)?,
        };
        let envelope: IllustrationListEnvelope = self.get_json(access_token, url, signature)?;
        page_from_envelope(envelope, FOLLOWED_ILLUSTRATIONS_PATH, &bindings)
    }

    pub fn bookmarked_illustrations(
        &self,
        access_token: &str,
        user_id: &str,
        restrict: &str,
        tag: Option<&str>,
        cursor: Option<&str>,
        signature: &ClientRequestSignature,
    ) -> Result<IllustrationPage, ApiError> {
        let user_id = normalized_resource_id(user_id)?;
        let restrict = normalized_bookmark_restrict(restrict)?;
        let tag = tag
            .map(|tag| normalized_bookmark_tags(&[tag.to_owned()]))
            .transpose()?
            .and_then(|tags| tags.into_iter().next());
        let mut bindings = vec![("user_id", user_id.as_str()), ("restrict", restrict)];
        if let Some(tag) = tag.as_deref() {
            bindings.push(("tag", tag));
        }
        let url = match cursor {
            Some(cursor) => decode_cursor(cursor, BOOKMARKED_ILLUSTRATIONS_PATH, &bindings)?,
            None => endpoint_url(BOOKMARKED_ILLUSTRATIONS_PATH, &{
                let mut query = bindings.clone();
                query.push(("filter", "for_ios"));
                query
            })?,
        };
        let envelope: IllustrationListEnvelope = self.get_json(access_token, url, signature)?;
        page_from_envelope(envelope, BOOKMARKED_ILLUSTRATIONS_PATH, &bindings)
    }

    pub fn add_illustration_bookmark(
        &self,
        access_token: &str,
        illustration_id: &str,
        restrict: &str,
        signature: &ClientRequestSignature,
    ) -> Result<(), ApiError> {
        self.update_bookmark(
            access_token,
            &BookmarkUpdate {
                kind: BookmarkContentKind::Illustration,
                resource_id: illustration_id.to_owned(),
                bookmarked: true,
                restrict: restrict.to_owned(),
                tags: Vec::new(),
            },
            signature,
        )
    }

    pub fn delete_illustration_bookmark(
        &self,
        access_token: &str,
        illustration_id: &str,
        signature: &ClientRequestSignature,
    ) -> Result<(), ApiError> {
        let illustration_id = normalized_resource_id(illustration_id)?;
        let url = endpoint_url(BOOKMARK_DELETE_PATH, &[])?;
        self.post_form_unit(
            access_token,
            url,
            &[("illust_id", illustration_id.as_str())],
            signature,
        )
    }

    pub fn follow_user(
        &self,
        access_token: &str,
        user_id: &str,
        restrict: &str,
        signature: &ClientRequestSignature,
    ) -> Result<(), ApiError> {
        let user_id = normalized_resource_id(user_id)?;
        let restrict = normalized_bookmark_restrict(restrict)?;
        let url = endpoint_url(FOLLOW_ADD_PATH, &[])?;
        self.post_form_unit(
            access_token,
            url,
            &[("user_id", user_id.as_str()), ("restrict", restrict)],
            signature,
        )
    }

    pub fn unfollow_user(
        &self,
        access_token: &str,
        user_id: &str,
        signature: &ClientRequestSignature,
    ) -> Result<(), ApiError> {
        let user_id = normalized_resource_id(user_id)?;
        let url = endpoint_url(FOLLOW_DELETE_PATH, &[])?;
        self.post_form_unit(
            access_token,
            url,
            &[("user_id", user_id.as_str())],
            signature,
        )
    }

    pub fn illustration_comments(
        &self,
        access_token: &str,
        illustration_id: &str,
        cursor: Option<&str>,
        signature: &ClientRequestSignature,
    ) -> Result<CommentPage, ApiError> {
        let illustration_id = normalized_resource_id(illustration_id)?;
        let bindings = [("illust_id", illustration_id.as_str())];
        let url = match cursor {
            Some(cursor) => decode_cursor(cursor, ILLUSTRATION_COMMENTS_PATH, &bindings)?,
            None => endpoint_url(ILLUSTRATION_COMMENTS_PATH, &bindings)?,
        };
        let envelope: CommentsEnvelope = self.get_json(access_token, url, signature)?;
        comment_page_from_envelope(envelope, ILLUSTRATION_COMMENTS_PATH, &bindings)
    }

    pub fn comment_replies(
        &self,
        access_token: &str,
        comment_id: &str,
        cursor: Option<&str>,
        signature: &ClientRequestSignature,
    ) -> Result<CommentPage, ApiError> {
        let comment_id = normalized_resource_id(comment_id)?;
        let bindings = [("comment_id", comment_id.as_str())];
        let url = match cursor {
            Some(cursor) => decode_cursor(cursor, COMMENT_REPLIES_PATH, &bindings)?,
            None => endpoint_url(COMMENT_REPLIES_PATH, &bindings)?,
        };
        let envelope: CommentsEnvelope = self.get_json(access_token, url, signature)?;
        comment_page_from_envelope(envelope, COMMENT_REPLIES_PATH, &bindings)
    }

    pub fn add_illustration_comment(
        &self,
        access_token: &str,
        illustration_id: &str,
        comment: &str,
        parent_comment_id: Option<&str>,
        stamp_id: Option<&str>,
        signature: &ClientRequestSignature,
    ) -> Result<Comment, ApiError> {
        let illustration_id = normalized_resource_id(illustration_id)?;
        let (comment, stamp_id) = normalized_comment_submission(comment, stamp_id)?;
        let parent_comment_id = parent_comment_id.map(normalized_resource_id).transpose()?;
        let mut form = vec![
            ("illust_id", illustration_id.as_str()),
            ("comment", comment.as_str()),
        ];
        if let Some(parent_comment_id) = parent_comment_id.as_deref() {
            form.push(("parent_comment_id", parent_comment_id));
        }
        if let Some(stamp_id) = stamp_id.as_deref() {
            form.push(("stamp_id", stamp_id));
        }
        let url = endpoint_url(COMMENT_ADD_PATH, &[])?;
        let envelope: CommentAddEnvelope =
            self.post_form_json(access_token, url, &form, signature)?;
        Comment::from_payload(&envelope.comment).ok_or(ApiError::InvalidResponse)
    }

    pub fn delete_illustration_comment(
        &self,
        access_token: &str,
        comment_id: &str,
        signature: &ClientRequestSignature,
    ) -> Result<(), ApiError> {
        let comment_id = normalized_resource_id(comment_id)?;
        let url = endpoint_url(COMMENT_DELETE_PATH, &[])?;
        self.post_form_unit(
            access_token,
            url,
            &[("comment_id", comment_id.as_str())],
            signature,
        )
    }

    pub fn comment_stamps(
        &self,
        access_token: &str,
        signature: &ClientRequestSignature,
    ) -> Result<Vec<CommentStamp>, ApiError> {
        let url = endpoint_url(COMMENT_STAMPS_PATH, &[])?;
        let envelope: StampListEnvelope = self.get_json(access_token, url, signature)?;
        Ok(comment_stamps_from_envelope(envelope))
    }

    fn get_json<T: DeserializeOwned>(
        &self,
        access_token: &str,
        url: Url,
        signature: &ClientRequestSignature,
    ) -> Result<T, ApiError> {
        if access_token.is_empty() {
            return Err(ApiError::AuthenticationRequired);
        }

        let response = self
            .http
            .get(url)
            .bearer_auth(access_token)
            .header("User-Agent", USER_AGENT)
            .header("Accept-Language", "zh-CN")
            .header("App-OS", "Android")
            .header("App-OS-Version", "Android 13")
            .header("App-Version", APP_VERSION)
            .header("X-Client-Time", signature.client_time())
            .header("X-Client-Hash", signature.client_hash())
            .send()
            .map_err(|_| ApiError::RequestFailed)?;
        let status = response.status();
        if !status.is_success() {
            let status = status.as_u16();
            let mut body = String::new();
            let _ = response
                .take((MAX_ERROR_BODY_BYTES + 1) as u64)
                .read_to_string(&mut body);
            return Err(classify_rejection(status, &body));
        }

        response.json().map_err(|_| ApiError::InvalidResponse)
    }

    fn get_text(
        &self,
        access_token: &str,
        url: Url,
        signature: &ClientRequestSignature,
        max_bytes: usize,
    ) -> Result<String, ApiError> {
        if access_token.is_empty() {
            return Err(ApiError::AuthenticationRequired);
        }
        let response = self
            .http
            .get(url)
            .bearer_auth(access_token)
            .header("User-Agent", USER_AGENT)
            .header("Accept-Language", "zh-CN")
            .header("App-OS", "Android")
            .header("App-OS-Version", "Android 13")
            .header("App-Version", APP_VERSION)
            .header("X-Client-Time", signature.client_time())
            .header("X-Client-Hash", signature.client_hash())
            .send()
            .map_err(|_| ApiError::RequestFailed)?;
        let mut response = ensure_success(response)?;
        if response
            .content_length()
            .is_some_and(|length| length > max_bytes as u64)
        {
            return Err(ApiError::InvalidResponse);
        }
        let mut body = Vec::new();
        response
            .by_ref()
            .take((max_bytes + 1) as u64)
            .read_to_end(&mut body)
            .map_err(|_| ApiError::RequestFailed)?;
        if body.len() > max_bytes {
            return Err(ApiError::InvalidResponse);
        }
        String::from_utf8(body).map_err(|_| ApiError::InvalidResponse)
    }

    fn post_form_unit(
        &self,
        access_token: &str,
        url: Url,
        form: &[(&str, &str)],
        signature: &ClientRequestSignature,
    ) -> Result<(), ApiError> {
        let response = self.post_form(access_token, url, form, signature)?;
        ensure_success(response).map(|_| ())
    }

    fn post_form_json<T: DeserializeOwned>(
        &self,
        access_token: &str,
        url: Url,
        form: &[(&str, &str)],
        signature: &ClientRequestSignature,
    ) -> Result<T, ApiError> {
        let response = self.post_form(access_token, url, form, signature)?;
        ensure_success(response)?
            .json()
            .map_err(|_| ApiError::InvalidResponse)
    }

    fn post_form(
        &self,
        access_token: &str,
        url: Url,
        form: &[(&str, &str)],
        signature: &ClientRequestSignature,
    ) -> Result<reqwest::blocking::Response, ApiError> {
        if access_token.is_empty() {
            return Err(ApiError::AuthenticationRequired);
        }
        self.http
            .post(url)
            .bearer_auth(access_token)
            .header("User-Agent", USER_AGENT)
            .header("Accept-Language", "zh-CN")
            .header("App-OS", "Android")
            .header("App-OS-Version", "Android 13")
            .header("App-Version", APP_VERSION)
            .header("X-Client-Time", signature.client_time())
            .header("X-Client-Hash", signature.client_hash())
            .form(form)
            .send()
            .map_err(|_| ApiError::RequestFailed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IllustrationPage {
    pub illustrations: Vec<IllustrationSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NovelPage {
    pub novels: Vec<NovelSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NovelSeriesPage {
    pub series: NovelSeriesDetail,
    pub first_novel: NovelSummary,
    pub latest_novel: NovelSummary,
    pub novels: Vec<NovelSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NovelSeriesDetail {
    pub id: String,
    pub title: String,
    pub caption: String,
    pub is_original: bool,
    pub is_concluded: bool,
    pub content_count: u32,
    pub total_character_count: u64,
    pub author: IllustrationAuthor,
    pub display_text: String,
    pub ai_type: u8,
    pub watchlist_added: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NovelSummary {
    pub id: String,
    pub title: String,
    pub caption: String,
    pub cover_url: Option<String>,
    pub author: IllustrationAuthor,
    pub create_date: String,
    pub page_count: u32,
    pub text_length: u64,
    pub is_bookmarked: bool,
    pub x_restrict: u8,
    pub ai_type: u8,
    pub tags: Vec<String>,
    pub series: Option<IllustrationSeries>,
    pub total_views: u64,
    pub total_bookmarks: u64,
    pub total_comments: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NovelDetail {
    pub novel: NovelSummary,
    pub visible: bool,
    pub is_muted: bool,
    pub is_original: bool,
    pub is_mypixiv_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NovelContent {
    pub novel_id: String,
    pub title: String,
    pub text: String,
    pub cover_url: Option<String>,
    pub series_id: Option<String>,
    pub series_title: Option<String>,
    pub series_navigation: NovelSeriesNavigation,
    pub illustration_ids: Vec<String>,
    pub image_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NovelSeriesNavigation {
    pub previous: Option<NovelSeriesNavigationItem>,
    pub next: Option<NovelSeriesNavigationItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NovelSeriesNavigationItem {
    pub id: String,
    pub title: String,
    pub cover_url: Option<String>,
    pub content_order: String,
    pub viewable: bool,
    pub viewable_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UgoiraMetadata {
    pub zip_url: String,
    pub frames: Vec<UgoiraFrame>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UgoiraFrame {
    pub file_name: String,
    pub delay_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IllustrationSummary {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub thumbnail_url: Option<String>,
    pub author: IllustrationAuthor,
    pub page_count: u32,
    pub width: u32,
    pub height: u32,
    pub is_bookmarked: bool,
    pub x_restrict: u8,
    pub sanity_level: u8,
    pub ai_type: u8,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IllustrationAuthor {
    pub id: String,
    pub name: String,
    pub account: String,
    pub avatar_url: Option<String>,
    pub is_followed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IllustrationDetail {
    pub illustration: IllustrationSummary,
    pub caption: String,
    pub create_date: String,
    pub pages: Vec<IllustrationImage>,
    pub total_views: u64,
    pub total_bookmarks: u64,
    pub total_comments: u64,
    pub tools: Vec<String>,
    pub visible: bool,
    pub is_muted: bool,
    pub series: Option<IllustrationSeries>,
    pub tags: Vec<IllustrationTag>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IllustrationImage {
    pub page_index: u32,
    pub display_url: Option<String>,
    pub original_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IllustrationSeries {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IllustrationSeriesPage {
    pub series: IllustrationSeriesDetail,
    pub first_illustration: IllustrationSummary,
    pub illustrations: Vec<IllustrationSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IllustrationSeriesDetail {
    pub id: String,
    pub title: String,
    pub caption: String,
    pub cover_url: Option<String>,
    pub work_count: u32,
    pub create_date: String,
    pub width: u32,
    pub height: u32,
    pub author: IllustrationAuthor,
    pub watchlist_added: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IllustrationTag {
    pub name: String,
    pub translated_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDetail {
    pub user: IllustrationAuthor,
    pub comment: String,
    pub profile: UserProfile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserProfile {
    pub webpage: Option<String>,
    pub gender: String,
    pub birth: String,
    pub region: String,
    pub job: String,
    pub total_follow_users: u64,
    pub total_mypixiv_users: u64,
    pub total_illustrations: u64,
    pub total_manga: u64,
    pub total_novels: u64,
    pub total_illustration_bookmarks: u64,
    pub background_image_url: Option<String>,
    pub twitter_account: String,
    pub twitter_url: Option<String>,
    pub pawoo_url: Option<String>,
    pub is_premium: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrendingTag {
    pub name: String,
    pub translated_name: Option<String>,
    pub illustration: IllustrationSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPreviewPage {
    pub users: Vec<UserPreview>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPreview {
    pub user: IllustrationAuthor,
    pub illustrations: Vec<IllustrationSummary>,
    pub is_muted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentPage {
    pub comments: Vec<Comment>,
    pub next_cursor: Option<String>,
    pub total_comments: u64,
    pub comment_access_control: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: String,
    pub text: String,
    pub date: String,
    pub user: Option<IllustrationAuthor>,
    pub has_replies: bool,
    pub parent: Option<CommentParent>,
    pub stamp: Option<CommentStamp>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentParent {
    pub id: String,
    pub text: String,
    pub user_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentStamp {
    pub id: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationPage {
    pub notifications: Vec<NotificationItem>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessBlockPage {
    pub users: Vec<IllustrationAuthor>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MuteSettings {
    pub tags: Vec<MutedTag>,
    pub users: Vec<MutedUser>,
    pub limit_count: u32,
    pub text_limits: MuteTextLimits,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MutedTag {
    pub name: String,
    pub translated_name: Option<String>,
    pub is_premium_slot: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MutedUser {
    pub user: IllustrationAuthor,
    pub is_premium_slot: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MuteTextLimits {
    pub without_premium: u32,
    pub with_premium: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationItem {
    pub id: String,
    pub type_id: u32,
    pub is_read: bool,
    pub created_datetime: String,
    pub target_url: Option<String>,
    pub content: NotificationContent,
    pub view_more: Option<NotificationViewMore>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationContent {
    pub text: String,
    pub left_icon: Option<String>,
    pub left_image: Option<String>,
    pub right_icon: Option<String>,
    pub right_image: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationViewMore {
    pub title: String,
    pub unread_exists: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    AuthenticationRequired,
    InvalidCursor,
    InvalidIdentifier,
    InvalidInput,
    InvalidMediaUrl,
    RequestFailed,
    Rejected { http_status: u16 },
    InvalidResponse,
}

impl fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AuthenticationRequired => formatter.write_str("authentication is required"),
            Self::InvalidCursor => formatter.write_str("invalid page cursor"),
            Self::InvalidIdentifier => formatter.write_str("invalid Pixiv resource identifier"),
            Self::InvalidInput => formatter.write_str("invalid Pixiv input"),
            Self::InvalidMediaUrl => formatter.write_str("invalid Pixiv media URL"),
            Self::RequestFailed => formatter.write_str("Pixiv API request failed"),
            Self::Rejected { http_status } => {
                write!(formatter, "Pixiv API rejected the request ({http_status})")
            }
            Self::InvalidResponse => formatter.write_str("invalid Pixiv API response"),
        }
    }
}

impl std::error::Error for ApiError {}

pub fn validated_media_url(candidate: &str) -> Result<Url, ApiError> {
    if candidate.len() > MAX_CURSOR_BYTES {
        return Err(ApiError::InvalidMediaUrl);
    }
    let url = Url::parse(candidate).map_err(|_| ApiError::InvalidMediaUrl)?;
    if url.scheme() != "https"
        || !matches!(url.host_str(), Some("i.pximg.net" | "s.pximg.net"))
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.path().is_empty()
        || url.path() == "/"
    {
        return Err(ApiError::InvalidMediaUrl);
    }
    Ok(url)
}

fn recommended_url(cursor: Option<&str>) -> Result<Url, ApiError> {
    match cursor {
        Some(cursor) => decode_cursor(cursor, RECOMMENDED_PATH, &[]),
        None => endpoint_url(
            RECOMMENDED_PATH,
            &[("filter", "for_ios"), ("include_ranking_label", "true")],
        ),
    }
}

fn endpoint_url(path: &str, query: &[(&str, &str)]) -> Result<Url, ApiError> {
    let mut url = Url::parse(&format!("https://{API_HOST}{path}"))
        .map_err(|_| ApiError::InvalidIdentifier)?;
    url.query_pairs_mut().extend_pairs(query.iter().copied());
    Ok(url)
}

fn normalized_resource_id(candidate: &str) -> Result<String, ApiError> {
    let value = candidate
        .parse::<u64>()
        .map_err(|_| ApiError::InvalidIdentifier)?;
    if value == 0 {
        return Err(ApiError::InvalidIdentifier);
    }
    Ok(value.to_string())
}

fn normalized_mute_tag(candidate: &str) -> Result<String, ApiError> {
    let value = candidate.trim();
    if value.is_empty() || value.chars().count() > 100 || value.chars().any(char::is_control) {
        return Err(ApiError::InvalidInput);
    }
    Ok(value.to_owned())
}

fn normalized_work_kind(candidate: &str) -> Result<&'static str, ApiError> {
    match candidate {
        "illust" => Ok("illust"),
        "manga" => Ok("manga"),
        _ => Err(ApiError::InvalidIdentifier),
    }
}

fn normalized_ranking_mode(candidate: &str) -> Result<&'static str, ApiError> {
    match candidate {
        "day" => Ok("day"),
        "week" => Ok("week"),
        "month" => Ok("month"),
        _ => Err(ApiError::InvalidIdentifier),
    }
}

fn normalized_search_target(candidate: &str) -> Result<&'static str, ApiError> {
    match candidate {
        "partial_match_for_tags" => Ok("partial_match_for_tags"),
        "exact_match_for_tags" => Ok("exact_match_for_tags"),
        "title_and_caption" => Ok("title_and_caption"),
        _ => Err(ApiError::InvalidIdentifier),
    }
}

fn normalized_search_word(candidate: &str) -> Result<String, ApiError> {
    let word = candidate.trim();
    if word.is_empty()
        || word.len() > 512
        || word.chars().count() > 100
        || word.chars().any(char::is_control)
    {
        return Err(ApiError::InvalidIdentifier);
    }
    Ok(word.to_owned())
}

fn normalized_comment(candidate: &str) -> Result<String, ApiError> {
    let comment = candidate.trim();
    if comment.is_empty()
        || comment.len() > 2048
        || comment.chars().count() > 140
        || comment
            .chars()
            .any(|character| character.is_control() && character != '\n' && character != '\r')
    {
        return Err(ApiError::InvalidInput);
    }
    Ok(comment.to_owned())
}

fn normalized_comment_submission(
    candidate: &str,
    stamp_id: Option<&str>,
) -> Result<(String, Option<String>), ApiError> {
    let stamp_id = stamp_id.map(normalized_resource_id).transpose()?;
    let comment = if candidate.trim().is_empty() {
        if stamp_id.is_none() {
            return Err(ApiError::InvalidInput);
        }
        String::new()
    } else {
        normalized_comment(candidate)?
    };
    Ok((comment, stamp_id))
}

fn normalized_bookmark_restrict(candidate: &str) -> Result<&'static str, ApiError> {
    match candidate {
        "public" => Ok("public"),
        "private" => Ok("private"),
        _ => Err(ApiError::InvalidIdentifier),
    }
}

fn normalized_bookmark_tags(candidates: &[String]) -> Result<Vec<String>, ApiError> {
    if candidates.len() > MAX_BOOKMARK_TAGS {
        return Err(ApiError::InvalidInput);
    }
    let mut tags = Vec::with_capacity(candidates.len());
    let mut seen = std::collections::HashSet::new();
    for candidate in candidates {
        let tag = candidate.trim();
        if tag.is_empty()
            || tag.len() > MAX_BOOKMARK_TAG_BYTES
            || tag.chars().count() > 100
            || tag.chars().any(char::is_control)
        {
            return Err(ApiError::InvalidInput);
        }
        let key = tag.to_lowercase();
        if seen.insert(key) {
            tags.push(tag.to_owned());
        }
    }
    Ok(tags)
}

fn bookmark_update_parts(
    update: &BookmarkUpdate,
) -> Result<
    (
        &'static str,
        &'static str,
        String,
        &'static str,
        Vec<String>,
    ),
    ApiError,
> {
    let resource_id = normalized_resource_id(&update.resource_id)?;
    let restrict = normalized_bookmark_restrict(&update.restrict)?;
    let tags = normalized_bookmark_tags(&update.tags)?;
    let (path, parameter) = match update.kind {
        BookmarkContentKind::Illustration => (BOOKMARK_ADD_PATH, "illust_id"),
        BookmarkContentKind::Novel => (NOVEL_BOOKMARK_ADD_PATH, "novel_id"),
    };
    Ok((path, parameter, resource_id, restrict, tags))
}

fn bookmark_update_failure(error: ApiError) -> BookmarkUpdateFailure {
    match error {
        ApiError::AuthenticationRequired => BookmarkUpdateFailure::AuthenticationRequired,
        ApiError::InvalidCursor
        | ApiError::InvalidIdentifier
        | ApiError::InvalidInput
        | ApiError::InvalidMediaUrl => BookmarkUpdateFailure::InvalidInput,
        ApiError::RequestFailed => BookmarkUpdateFailure::RequestFailed,
        ApiError::Rejected { .. } => BookmarkUpdateFailure::Rejected,
        ApiError::InvalidResponse => BookmarkUpdateFailure::InvalidResponse,
    }
}

fn bookmark_detail_from_envelope(
    envelope: BookmarkDetailEnvelope,
) -> Result<BookmarkDetail, ApiError> {
    let restrict = normalized_bookmark_restrict(&envelope.bookmark_detail.restrict)?.to_owned();
    let mut tags = Vec::with_capacity(envelope.bookmark_detail.tags.len());
    if envelope.bookmark_detail.tags.len() > MAX_BOOKMARK_TAGS {
        return Err(ApiError::InvalidResponse);
    }
    for tag in envelope.bookmark_detail.tags {
        let name = normalized_bookmark_tags(&[tag.name])?
            .into_iter()
            .next()
            .ok_or(ApiError::InvalidResponse)?;
        tags.push(BookmarkTagStatus {
            name,
            is_registered: tag.is_registered,
        });
    }
    Ok(BookmarkDetail { restrict, tags })
}

fn bookmark_tag_page_from_envelope(
    envelope: BookmarkTagsEnvelope,
    path: &str,
    bindings: &[(&str, &str)],
) -> Result<BookmarkTagPage, ApiError> {
    if envelope.bookmark_tags.len() > 10_000 {
        return Err(ApiError::InvalidResponse);
    }
    let mut tags = Vec::with_capacity(envelope.bookmark_tags.len());
    for tag in envelope.bookmark_tags {
        let name = normalized_bookmark_tags(&[tag.name])?
            .into_iter()
            .next()
            .ok_or(ApiError::InvalidResponse)?;
        tags.push(BookmarkTag {
            name,
            count: tag.count,
        });
    }
    let next_cursor = envelope
        .next_url
        .filter(|value| !value.is_empty())
        .map(|next| validate_api_url(&next, path, bindings).map(|url| encode_cursor(&url)))
        .transpose()
        .map_err(|_| ApiError::InvalidResponse)?;
    Ok(BookmarkTagPage { tags, next_cursor })
}

fn classify_rejection(http_status: u16, response_body: &str) -> ApiError {
    let body = response_body.to_ascii_lowercase();
    if http_status == 401
        || (http_status == 400
            && (body.contains("invalid_grant")
                || body.contains("oauth process")
                || body.contains("invalid access token")))
    {
        ApiError::AuthenticationRequired
    } else {
        ApiError::Rejected { http_status }
    }
}

fn ensure_success(
    response: reqwest::blocking::Response,
) -> Result<reqwest::blocking::Response, ApiError> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status().as_u16();
    let mut body = String::new();
    let _ = response
        .take((MAX_ERROR_BODY_BYTES + 1) as u64)
        .read_to_string(&mut body);
    Err(classify_rejection(status, &body))
}

fn encode_cursor(url: &Url) -> String {
    URL_SAFE_NO_PAD.encode(url.as_str())
}

fn decode_cursor(
    cursor: &str,
    expected_path: &str,
    expected_bindings: &[(&str, &str)],
) -> Result<Url, ApiError> {
    if cursor.is_empty() || cursor.len() > MAX_CURSOR_BYTES * 2 {
        return Err(ApiError::InvalidCursor);
    }
    let decoded = URL_SAFE_NO_PAD
        .decode(cursor)
        .map_err(|_| ApiError::InvalidCursor)?;
    if decoded.len() > MAX_CURSOR_BYTES {
        return Err(ApiError::InvalidCursor);
    }
    let decoded = String::from_utf8(decoded).map_err(|_| ApiError::InvalidCursor)?;
    validate_api_url(&decoded, expected_path, expected_bindings)
}

fn validate_api_url(
    candidate: &str,
    expected_path: &str,
    expected_bindings: &[(&str, &str)],
) -> Result<Url, ApiError> {
    let url = Url::parse(candidate).map_err(|_| ApiError::InvalidCursor)?;
    let valid_origin = url.scheme() == "https"
        && url.host_str() == Some(API_HOST)
        && url.path() == expected_path
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
        && url.as_str().len() <= MAX_CURSOR_BYTES;
    let bindings_match = expected_bindings
        .iter()
        .all(|(expected_key, expected_value)| {
            url.query_pairs()
                .any(|(key, value)| key == *expected_key && value == *expected_value)
        });
    if !valid_origin || !bindings_match {
        return Err(ApiError::InvalidCursor);
    }
    Ok(url)
}

fn page_from_envelope(
    envelope: IllustrationListEnvelope,
    expected_path: &str,
    expected_bindings: &[(&str, &str)],
) -> Result<IllustrationPage, ApiError> {
    let illustrations = envelope
        .illusts
        .iter()
        .filter_map(IllustrationSummary::from_payload)
        .collect();
    let next_cursor = envelope
        .next_url
        .filter(|value| !value.is_empty())
        .map(|value| {
            validate_api_url(&value, expected_path, expected_bindings)
                .map(|url| encode_cursor(&url))
        })
        .transpose()
        .map_err(|_| ApiError::InvalidResponse)?;
    Ok(IllustrationPage {
        illustrations,
        next_cursor,
    })
}

fn access_block_page_from_envelope(
    envelope: AccessBlockEnvelope,
) -> Result<AccessBlockPage, ApiError> {
    let users = envelope
        .blocked_users
        .iter()
        .filter_map(IllustrationAuthor::from_payload)
        .collect();
    let next_cursor = envelope
        .next_url
        .filter(|value| !value.is_empty())
        .map(|value| {
            validate_api_url(&value, ACCESS_BLOCK_USERS_PATH, &[]).map(|url| encode_cursor(&url))
        })
        .transpose()
        .map_err(|_| ApiError::InvalidResponse)?;
    Ok(AccessBlockPage { users, next_cursor })
}

fn mute_settings_from_envelope(envelope: MuteSettingsEnvelope) -> Result<MuteSettings, ApiError> {
    let tags = envelope
        .muted_tags
        .into_iter()
        .filter_map(|item| {
            let name = item.tag.name.trim().to_owned();
            (!name.is_empty()).then(|| MutedTag {
                name,
                translated_name: item
                    .tag
                    .translated_name
                    .filter(|value| !value.trim().is_empty()),
                is_premium_slot: item.is_premium_slot,
            })
        })
        .collect();
    let users = envelope
        .muted_users
        .into_iter()
        .filter_map(|item| {
            IllustrationAuthor::from_payload(&item.user).map(|user| MutedUser {
                user,
                is_premium_slot: item.is_premium_slot,
            })
        })
        .collect();
    Ok(MuteSettings {
        tags,
        users,
        limit_count: envelope.mute_limit_count,
        text_limits: MuteTextLimits {
            without_premium: envelope.for_text.mute_limit_count_if_no_premium,
            with_premium: envelope.for_text.mute_limit_count_if_premium,
        },
    })
}

fn novel_page_from_envelope(
    envelope: NovelListEnvelope,
    expected_path: &str,
    expected_bindings: &[(&str, &str)],
) -> Result<NovelPage, ApiError> {
    let novels = envelope
        .novels
        .iter()
        .filter_map(NovelSummary::from_payload)
        .collect();
    let next_cursor = envelope
        .next_url
        .filter(|value| !value.is_empty())
        .map(|value| {
            validate_api_url(&value, expected_path, expected_bindings)
                .map(|url| encode_cursor(&url))
        })
        .transpose()
        .map_err(|_| ApiError::InvalidResponse)?;
    Ok(NovelPage {
        novels,
        next_cursor,
    })
}

fn user_preview_page_from_envelope(
    envelope: UserPreviewEnvelope,
    expected_path: &str,
    expected_bindings: &[(&str, &str)],
) -> Result<UserPreviewPage, ApiError> {
    let users = envelope
        .user_previews
        .iter()
        .filter_map(UserPreview::from_payload)
        .collect();
    let next_cursor = envelope
        .next_url
        .filter(|value| !value.is_empty())
        .map(|value| {
            validate_api_url(&value, expected_path, expected_bindings)
                .map(|url| encode_cursor(&url))
        })
        .transpose()
        .map_err(|_| ApiError::InvalidResponse)?;
    Ok(UserPreviewPage { users, next_cursor })
}

fn comment_page_from_envelope(
    envelope: CommentsEnvelope,
    expected_path: &str,
    expected_bindings: &[(&str, &str)],
) -> Result<CommentPage, ApiError> {
    let comments = envelope
        .comments
        .iter()
        .filter_map(Comment::from_payload)
        .collect();
    let next_cursor = envelope
        .next_url
        .filter(|value| !value.is_empty())
        .map(|value| {
            validate_api_url(&value, expected_path, expected_bindings)
                .map(|url| encode_cursor(&url))
        })
        .transpose()
        .map_err(|_| ApiError::InvalidResponse)?;
    Ok(CommentPage {
        comments,
        next_cursor,
        total_comments: envelope.total_comments,
        comment_access_control: envelope.comment_access_control,
    })
}

fn comment_stamps_from_envelope(envelope: StampListEnvelope) -> Vec<CommentStamp> {
    envelope
        .stamps
        .into_iter()
        .filter_map(|stamp| {
            (stamp.stamp_id != 0)
                .then(|| validated_media_url(&stamp.stamp_url).ok())
                .flatten()
                .map(|url| CommentStamp {
                    id: stamp.stamp_id.to_string(),
                    url: url.to_string(),
                })
        })
        .collect()
}

#[cfg(test)]
fn notification_page_from_envelope(
    envelope: NotificationsEnvelope,
) -> Result<NotificationPage, ApiError> {
    notification_page_from_envelope_for(
        envelope,
        NOTIFICATION_LIST_PATH,
        &[("limit", NOTIFICATION_PAGE_LIMIT)],
    )
}

fn notification_page_from_envelope_for(
    envelope: NotificationsEnvelope,
    expected_path: &str,
    expected_bindings: &[(&str, &str)],
) -> Result<NotificationPage, ApiError> {
    let notifications = envelope
        .notifications
        .iter()
        .filter_map(NotificationItem::from_payload)
        .collect();
    let next_cursor = envelope
        .next_url
        .filter(|value| !value.is_empty())
        .map(|value| {
            validate_api_url(&value, expected_path, expected_bindings)
                .map(|url| encode_cursor(&url))
        })
        .transpose()
        .map_err(|_| ApiError::InvalidResponse)?;
    Ok(NotificationPage {
        notifications,
        next_cursor,
    })
}

impl IllustrationSummary {
    fn from_payload(payload: &IllustrationPayload) -> Option<Self> {
        if payload.id == 0 || payload.user.id == 0 {
            return None;
        }
        let thumbnail_url = first_media_url([
            payload.image_urls.square_medium.as_deref(),
            payload.image_urls.medium.as_deref(),
            payload.image_urls.large.as_deref(),
        ]);

        Some(Self {
            id: payload.id.to_string(),
            title: payload.title.clone(),
            kind: payload.kind.clone(),
            thumbnail_url,
            author: IllustrationAuthor::from_payload(&payload.user)?,
            page_count: payload.page_count.max(1),
            width: payload.width,
            height: payload.height,
            is_bookmarked: payload.is_bookmarked.unwrap_or(false),
            x_restrict: payload.x_restrict,
            sanity_level: payload.sanity_level,
            ai_type: payload.ai_type,
            tags: payload
                .tags
                .iter()
                .map(|tag| tag.name.clone())
                .filter(|name| !name.is_empty())
                .take(8)
                .collect(),
        })
    }
}

impl NovelSummary {
    fn from_payload(payload: &NovelPayload) -> Option<Self> {
        if payload.id == 0 || payload.user.id == 0 {
            return None;
        }
        Some(Self {
            id: payload.id.to_string(),
            title: payload.title.clone(),
            caption: payload.caption.clone(),
            cover_url: first_media_url([
                payload.image_urls.large.as_deref(),
                payload.image_urls.medium.as_deref(),
                payload.image_urls.square_medium.as_deref(),
            ]),
            author: IllustrationAuthor::from_payload(&payload.user)?,
            create_date: payload.create_date.clone(),
            page_count: payload.page_count.max(1),
            text_length: payload.text_length,
            is_bookmarked: payload.is_bookmarked.unwrap_or(false),
            x_restrict: payload.x_restrict,
            ai_type: payload.ai_type,
            tags: payload
                .tags
                .iter()
                .map(|tag| tag.name.clone())
                .filter(|name| !name.is_empty())
                .take(8)
                .collect(),
            series: payload.series.as_ref().and_then(|series| {
                (series.id != 0).then(|| IllustrationSeries {
                    id: series.id.to_string(),
                    title: series.title.clone(),
                })
            }),
            total_views: payload.total_view,
            total_bookmarks: payload.total_bookmarks,
            total_comments: payload.total_comments,
        })
    }
}

impl NovelSeriesPage {
    fn from_envelope(
        envelope: NovelSeriesEnvelope,
        expected_series_id: &str,
    ) -> Result<Self, ApiError> {
        if envelope.novel_series_detail.id.to_string() != expected_series_id {
            return Err(ApiError::InvalidResponse);
        }
        let series = NovelSeriesDetail::from_payload(envelope.novel_series_detail)
            .ok_or(ApiError::InvalidResponse)?;
        let first_novel = NovelSummary::from_payload(&envelope.novel_series_first_novel)
            .ok_or(ApiError::InvalidResponse)?;
        let latest_novel = NovelSummary::from_payload(&envelope.novel_series_latest_novel)
            .ok_or(ApiError::InvalidResponse)?;
        let novels = envelope
            .novels
            .iter()
            .filter_map(NovelSummary::from_payload)
            .collect();
        let bindings = [("series_id", expected_series_id)];
        let next_cursor = envelope
            .next_url
            .filter(|value| !value.is_empty())
            .map(|value| {
                validate_api_url(&value, NOVEL_SERIES_PATH, &bindings)
                    .map(|url| encode_cursor(&url))
            })
            .transpose()
            .map_err(|_| ApiError::InvalidResponse)?;
        Ok(Self {
            series,
            first_novel,
            latest_novel,
            novels,
            next_cursor,
        })
    }
}

impl NovelSeriesDetail {
    fn from_payload(payload: NovelSeriesDetailPayload) -> Option<Self> {
        (payload.id != 0).then_some(Self {
            id: payload.id.to_string(),
            title: payload.title,
            caption: payload.caption,
            is_original: payload.is_original,
            is_concluded: payload.is_concluded,
            content_count: payload.content_count,
            total_character_count: payload.total_character_count,
            author: IllustrationAuthor::from_payload(&payload.user)?,
            display_text: payload.display_text,
            ai_type: payload.ai_type,
            watchlist_added: payload.watchlist_added,
        })
    }
}

impl NovelDetail {
    fn from_payload(payload: NovelPayload) -> Result<Self, ApiError> {
        let novel = NovelSummary::from_payload(&payload).ok_or(ApiError::InvalidResponse)?;
        Ok(Self {
            novel,
            visible: payload.visible,
            is_muted: payload.is_muted,
            is_original: payload.is_original,
            is_mypixiv_only: payload.is_mypixiv_only,
        })
    }
}

impl NovelContent {
    fn from_payload(payload: NovelContentPayload, expected_id: &str) -> Result<Self, ApiError> {
        if payload.id != expected_id {
            return Err(ApiError::InvalidResponse);
        }
        Ok(Self {
            novel_id: payload.id,
            title: payload.title,
            text: payload.text,
            cover_url: payload
                .cover_url
                .as_deref()
                .and_then(|url| validated_media_url(url).ok())
                .map(|url| url.to_string()),
            series_id: non_empty(payload.series_id),
            series_title: non_empty(payload.series_title),
            series_navigation: NovelSeriesNavigation::from_payload(payload.series_navigation),
            illustration_ids: normalized_embedded_ids(payload.illusts),
            image_ids: normalized_embedded_ids(payload.images),
        })
    }
}

impl NovelSeriesNavigation {
    fn from_payload(payload: Option<NovelSeriesNavigationPayload>) -> Self {
        let Some(payload) = payload else {
            return Self::default();
        };
        Self {
            previous: payload
                .prev
                .and_then(NovelSeriesNavigationItem::from_payload),
            next: payload
                .next
                .and_then(NovelSeriesNavigationItem::from_payload),
        }
    }
}

impl NovelSeriesNavigationItem {
    fn from_payload(payload: NovelSeriesNavigationItemPayload) -> Option<Self> {
        (payload.id != 0).then_some(Self {
            id: payload.id.to_string(),
            title: payload.title,
            cover_url: payload
                .cover_url
                .as_deref()
                .and_then(|url| validated_media_url(url).ok())
                .map(|url| url.to_string()),
            content_order: payload.content_order,
            viewable: payload.viewable,
            viewable_message: non_empty(payload.viewable_message),
        })
    }
}

impl UgoiraMetadata {
    fn from_payload(payload: UgoiraPayload) -> Result<Self, ApiError> {
        let zip_url = payload
            .zip_urls
            .medium
            .as_deref()
            .ok_or(ApiError::InvalidResponse)
            .and_then(validated_media_url)?
            .to_string();
        if payload.frames.is_empty() || payload.frames.len() > 20_000 {
            return Err(ApiError::InvalidResponse);
        }
        let mut frames = Vec::with_capacity(payload.frames.len());
        for frame in payload.frames {
            let file_name = frame.file.trim();
            let safe_name = !file_name.is_empty()
                && file_name.len() <= 255
                && !file_name.contains(['/', '\\'])
                && file_name != "."
                && file_name != "..";
            if !safe_name || frame.delay == 0 || frame.delay > 60_000 {
                return Err(ApiError::InvalidResponse);
            }
            frames.push(UgoiraFrame {
                file_name: file_name.to_owned(),
                delay_ms: frame.delay,
            });
        }
        Ok(Self { zip_url, frames })
    }
}

impl IllustrationAuthor {
    fn from_payload(payload: &UserPayload) -> Option<Self> {
        (payload.id != 0).then(|| Self {
            id: payload.id.to_string(),
            name: payload.name.clone(),
            account: payload.account.clone(),
            avatar_url: first_media_url([payload.profile_image_urls.medium.as_deref()]),
            is_followed: payload.is_followed,
        })
    }
}

impl IllustrationSeriesPage {
    fn from_envelope(
        envelope: IllustrationSeriesEnvelope,
        expected_series_id: &str,
    ) -> Result<Self, ApiError> {
        if envelope.illust_series_detail.id.to_string() != expected_series_id {
            return Err(ApiError::InvalidResponse);
        }
        let series = IllustrationSeriesDetail::from_payload(envelope.illust_series_detail)
            .ok_or(ApiError::InvalidResponse)?;
        let first_illustration =
            IllustrationSummary::from_payload(&envelope.illust_series_first_illust)
                .ok_or(ApiError::InvalidResponse)?;
        let illustrations = envelope
            .illusts
            .iter()
            .filter_map(IllustrationSummary::from_payload)
            .collect();
        let bindings = [("illust_series_id", expected_series_id)];
        let next_cursor = envelope
            .next_url
            .filter(|value| !value.is_empty())
            .map(|value| {
                validate_api_url(&value, ILLUSTRATION_SERIES_PATH, &bindings)
                    .map(|url| encode_cursor(&url))
            })
            .transpose()
            .map_err(|_| ApiError::InvalidResponse)?;
        Ok(Self {
            series,
            first_illustration,
            illustrations,
            next_cursor,
        })
    }
}

impl IllustrationSeriesDetail {
    fn from_payload(payload: IllustrationSeriesDetailPayload) -> Option<Self> {
        (payload.id != 0).then_some(Self {
            id: payload.id.to_string(),
            title: payload.title,
            caption: payload.caption,
            cover_url: first_media_url([
                payload.cover_image_urls.medium.as_deref(),
                payload.cover_image_urls.large.as_deref(),
                payload.cover_image_urls.square_medium.as_deref(),
            ]),
            work_count: payload.series_work_count,
            create_date: payload.create_date,
            width: payload.width,
            height: payload.height,
            author: IllustrationAuthor::from_payload(&payload.user)?,
            watchlist_added: payload.watchlist_added,
        })
    }
}

impl IllustrationDetail {
    fn from_payload(payload: IllustrationPayload) -> Result<Self, ApiError> {
        let illustration =
            IllustrationSummary::from_payload(&payload).ok_or(ApiError::InvalidResponse)?;
        let pages = illustration_pages(&payload);
        let series = payload.series.as_ref().and_then(|series| {
            (series.id != 0).then(|| IllustrationSeries {
                id: series.id.to_string(),
                title: series.title.clone(),
            })
        });
        let tags = payload
            .tags
            .iter()
            .filter(|tag| !tag.name.is_empty())
            .map(|tag| IllustrationTag {
                name: tag.name.clone(),
                translated_name: tag.translated_name.clone().filter(|name| !name.is_empty()),
            })
            .collect();
        Ok(Self {
            illustration,
            caption: payload.caption,
            create_date: payload.create_date,
            pages,
            total_views: payload.total_view,
            total_bookmarks: payload.total_bookmarks,
            total_comments: payload.total_comments,
            tools: payload.tools,
            visible: payload.visible,
            is_muted: payload.is_muted,
            series,
            tags,
        })
    }
}

impl UserDetail {
    fn from_envelope(envelope: UserDetailEnvelope) -> Result<Self, ApiError> {
        let user =
            IllustrationAuthor::from_payload(&envelope.user).ok_or(ApiError::InvalidResponse)?;
        Ok(Self {
            user,
            comment: envelope.user.comment,
            profile: UserProfile {
                webpage: non_empty(envelope.profile.webpage),
                gender: envelope.profile.gender,
                birth: envelope.profile.birth,
                region: envelope.profile.region,
                job: envelope.profile.job,
                total_follow_users: envelope.profile.total_follow_users,
                total_mypixiv_users: envelope.profile.total_mypixiv_users,
                total_illustrations: envelope.profile.total_illusts,
                total_manga: envelope.profile.total_manga,
                total_novels: envelope.profile.total_novels,
                total_illustration_bookmarks: envelope.profile.total_illust_bookmarks,
                background_image_url: envelope
                    .profile
                    .background_image_url
                    .as_deref()
                    .and_then(|url| validated_media_url(url).ok())
                    .map(|url| url.to_string()),
                twitter_account: envelope.profile.twitter_account,
                twitter_url: non_empty(envelope.profile.twitter_url),
                pawoo_url: non_empty(envelope.profile.pawoo_url),
                is_premium: envelope.profile.is_premium,
            },
        })
    }
}

impl TrendingTag {
    fn from_payload(payload: &TrendingTagPayload) -> Option<Self> {
        let name = payload.tag.trim();
        if name.is_empty() {
            return None;
        }
        Some(Self {
            name: name.to_owned(),
            translated_name: payload
                .translated_name
                .clone()
                .filter(|name| !name.is_empty()),
            illustration: IllustrationSummary::from_payload(&payload.illust)?,
        })
    }
}

impl UserPreview {
    fn from_payload(payload: &UserPreviewPayload) -> Option<Self> {
        Some(Self {
            user: IllustrationAuthor::from_payload(&payload.user)?,
            illustrations: payload
                .illusts
                .iter()
                .filter_map(IllustrationSummary::from_payload)
                .take(3)
                .collect(),
            is_muted: payload.is_muted,
        })
    }
}

impl Comment {
    fn from_payload(payload: &CommentPayload) -> Option<Self> {
        (payload.id != 0).then(|| Self {
            id: payload.id.to_string(),
            text: payload.comment.clone(),
            date: payload.date.clone(),
            user: payload
                .user
                .as_ref()
                .and_then(IllustrationAuthor::from_payload),
            has_replies: payload.has_replies,
            parent: payload.parent_comment.as_deref().and_then(|parent| {
                (parent.id != 0).then(|| CommentParent {
                    id: parent.id.to_string(),
                    text: parent.comment.clone(),
                    user_name: parent
                        .user
                        .as_ref()
                        .map(|user| user.name.clone())
                        .unwrap_or_default(),
                })
            }),
            stamp: payload.stamp.as_ref().and_then(|stamp| {
                (stamp.stamp_id != 0)
                    .then(|| validated_media_url(&stamp.stamp_url).ok())
                    .flatten()
                    .map(|url| CommentStamp {
                        id: stamp.stamp_id.to_string(),
                        url: url.to_string(),
                    })
            }),
        })
    }
}

impl NotificationItem {
    fn from_payload(payload: &NotificationPayload) -> Option<Self> {
        (payload.id != 0).then(|| Self {
            id: payload.id.to_string(),
            type_id: payload.type_id,
            is_read: payload.is_read,
            created_datetime: payload.created_datetime.clone(),
            target_url: payload
                .target_url
                .as_ref()
                .map(|url| url.trim())
                .filter(|url| !url.is_empty() && url.len() <= MAX_CURSOR_BYTES)
                .map(ToOwned::to_owned),
            content: NotificationContent {
                text: payload.content.text.clone(),
                left_icon: non_empty_string(payload.content.left_icon.as_deref()),
                left_image: payload
                    .content
                    .left_image
                    .as_deref()
                    .and_then(|url| validated_media_url(url).ok())
                    .map(|url| url.to_string()),
                right_icon: non_empty_string(payload.content.right_icon.as_deref()),
                right_image: payload
                    .content
                    .right_image
                    .as_deref()
                    .and_then(|url| validated_media_url(url).ok())
                    .map(|url| url.to_string()),
            },
            view_more: payload
                .view_more
                .as_ref()
                .map(|view_more| NotificationViewMore {
                    title: view_more.title.clone(),
                    unread_exists: view_more.unread_exists,
                }),
        })
    }
}

fn non_empty_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn illustration_pages(payload: &IllustrationPayload) -> Vec<IllustrationImage> {
    if !payload.meta_pages.is_empty() {
        return payload
            .meta_pages
            .iter()
            .enumerate()
            .map(|(index, page)| IllustrationImage {
                page_index: u32::try_from(index).unwrap_or(u32::MAX),
                display_url: first_media_url([
                    page.image_urls.large.as_deref(),
                    page.image_urls.medium.as_deref(),
                    page.image_urls.original.as_deref(),
                    page.image_urls.square_medium.as_deref(),
                ]),
                original_url: first_media_url([page.image_urls.original.as_deref()]),
            })
            .collect();
    }

    let display_url = first_media_url([
        payload.image_urls.large.as_deref(),
        payload.image_urls.medium.as_deref(),
        payload.meta_single_page.original_image_url.as_deref(),
        payload.image_urls.square_medium.as_deref(),
    ]);
    let original_url = first_media_url([payload.meta_single_page.original_image_url.as_deref()]);
    if display_url.is_none() && original_url.is_none() {
        Vec::new()
    } else {
        vec![IllustrationImage {
            page_index: 0,
            display_url,
            original_url,
        }]
    }
}

fn first_media_url<'a>(candidates: impl IntoIterator<Item = Option<&'a str>>) -> Option<String> {
    candidates.into_iter().flatten().find_map(|candidate| {
        validated_media_url(candidate)
            .ok()
            .map(|url| url.to_string())
    })
}

fn normalized_embedded_ids(candidates: Vec<String>) -> Vec<String> {
    candidates
        .into_iter()
        .filter_map(|candidate| normalized_resource_id(candidate.trim()).ok())
        .take(256)
        .collect()
}

fn extract_embedded_novel_json(html: &str) -> Option<&str> {
    for (marker_index, _) in html.match_indices("novel:") {
        let rest = &html[marker_index + "novel:".len()..];
        let Some(start_offset) = rest.find('{') else {
            continue;
        };
        let candidate = &rest[start_offset..];
        let mut depth = 0_u32;
        let mut in_string = false;
        let mut escaped = false;
        for (index, character) in candidate.char_indices() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if character == '\\' {
                    escaped = true;
                } else if character == '"' {
                    in_string = false;
                }
                continue;
            }
            match character {
                '"' => in_string = true,
                '{' => depth = depth.saturating_add(1),
                '}' => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        return candidate.get(..=index);
                    }
                }
                _ => {}
            }
        }
    }
    None
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

#[derive(Deserialize)]
struct IllustrationListEnvelope {
    #[serde(default)]
    illusts: Vec<IllustrationPayload>,
    #[serde(default)]
    next_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BookmarkDetailEnvelope {
    bookmark_detail: BookmarkDetailPayload,
}

#[derive(Debug, Deserialize)]
struct BookmarkDetailPayload {
    restrict: String,
    tags: Vec<BookmarkTagStatusPayload>,
}

#[derive(Debug, Deserialize)]
struct BookmarkTagStatusPayload {
    name: String,
    is_registered: bool,
}

#[derive(Debug, Deserialize)]
struct BookmarkTagsEnvelope {
    bookmark_tags: Vec<BookmarkTagPayload>,
    #[serde(default)]
    next_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BookmarkTagPayload {
    name: String,
    count: u32,
}

#[derive(Deserialize)]
struct IllustrationDetailEnvelope {
    illust: IllustrationPayload,
}

#[derive(Deserialize)]
struct IllustrationSeriesEnvelope {
    illust_series_detail: IllustrationSeriesDetailPayload,
    illust_series_first_illust: IllustrationPayload,
    #[serde(default)]
    illusts: Vec<IllustrationPayload>,
    #[serde(default)]
    next_url: Option<String>,
}

#[derive(Deserialize)]
struct IllustrationSeriesDetailPayload {
    id: u64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    caption: String,
    #[serde(default)]
    cover_image_urls: ImageUrlsPayload,
    #[serde(default)]
    series_work_count: u32,
    #[serde(default)]
    create_date: String,
    #[serde(default)]
    width: u32,
    #[serde(default)]
    height: u32,
    user: UserPayload,
    #[serde(default)]
    watchlist_added: bool,
}

#[derive(Deserialize)]
struct NovelListEnvelope {
    #[serde(default)]
    novels: Vec<NovelPayload>,
    #[serde(default)]
    next_url: Option<String>,
}

#[derive(Deserialize)]
struct NovelDetailEnvelope {
    novel: NovelPayload,
}

#[derive(Deserialize)]
struct NovelSeriesEnvelope {
    novel_series_detail: NovelSeriesDetailPayload,
    novel_series_first_novel: NovelPayload,
    novel_series_latest_novel: NovelPayload,
    #[serde(default)]
    novels: Vec<NovelPayload>,
    #[serde(default)]
    next_url: Option<String>,
}

#[derive(Deserialize)]
struct NovelSeriesDetailPayload {
    id: u64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    caption: String,
    #[serde(default)]
    is_original: bool,
    #[serde(default)]
    is_concluded: bool,
    #[serde(default)]
    content_count: u32,
    #[serde(default)]
    total_character_count: u64,
    user: UserPayload,
    #[serde(default)]
    display_text: String,
    #[serde(default, rename = "novel_ai_type")]
    ai_type: u8,
    #[serde(default)]
    watchlist_added: bool,
}

#[derive(Deserialize)]
struct NovelPayload {
    id: u64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    caption: String,
    #[serde(default)]
    image_urls: ImageUrlsPayload,
    user: UserPayload,
    #[serde(default)]
    create_date: String,
    #[serde(default = "one")]
    page_count: u32,
    #[serde(default)]
    text_length: u64,
    #[serde(default)]
    is_bookmarked: Option<bool>,
    #[serde(default)]
    x_restrict: u8,
    #[serde(default, rename = "novel_ai_type")]
    ai_type: u8,
    #[serde(default)]
    tags: Vec<TagPayload>,
    #[serde(default)]
    series: Option<SeriesPayload>,
    #[serde(default)]
    total_view: u64,
    #[serde(default)]
    total_bookmarks: u64,
    #[serde(default)]
    total_comments: u64,
    #[serde(default = "yes")]
    visible: bool,
    #[serde(default)]
    is_muted: bool,
    #[serde(default)]
    is_original: bool,
    #[serde(default)]
    is_mypixiv_only: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NovelContentPayload {
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    cover_url: Option<String>,
    #[serde(default)]
    series_id: Option<String>,
    #[serde(default)]
    series_title: Option<String>,
    #[serde(default)]
    series_navigation: Option<NovelSeriesNavigationPayload>,
    #[serde(default)]
    illusts: Vec<String>,
    #[serde(default)]
    images: Vec<String>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NovelSeriesNavigationPayload {
    #[serde(default)]
    prev: Option<NovelSeriesNavigationItemPayload>,
    #[serde(default)]
    next: Option<NovelSeriesNavigationItemPayload>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NovelSeriesNavigationItemPayload {
    id: u64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    cover_url: Option<String>,
    #[serde(default)]
    content_order: String,
    #[serde(default = "yes")]
    viewable: bool,
    #[serde(default)]
    viewable_message: Option<String>,
}

#[derive(Deserialize)]
struct UgoiraEnvelope {
    ugoira_metadata: UgoiraPayload,
}

#[derive(Deserialize)]
struct UgoiraPayload {
    #[serde(default)]
    zip_urls: UgoiraZipUrlsPayload,
    #[serde(default)]
    frames: Vec<UgoiraFramePayload>,
}

#[derive(Default, Deserialize)]
struct UgoiraZipUrlsPayload {
    #[serde(default)]
    medium: Option<String>,
}

#[derive(Deserialize)]
struct UgoiraFramePayload {
    #[serde(default)]
    file: String,
    #[serde(default)]
    delay: u32,
}

#[derive(Deserialize)]
struct UserDetailEnvelope {
    user: UserPayload,
    #[serde(default)]
    profile: UserProfilePayload,
}

#[derive(Deserialize)]
struct CommentsEnvelope {
    #[serde(default)]
    total_comments: u64,
    #[serde(default)]
    comments: Vec<CommentPayload>,
    #[serde(default)]
    next_url: Option<String>,
    #[serde(default)]
    comment_access_control: u8,
}

#[derive(Deserialize)]
struct CommentAddEnvelope {
    comment: CommentPayload,
}

#[derive(Deserialize)]
struct CommentPayload {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    comment: String,
    #[serde(default)]
    date: String,
    #[serde(default)]
    user: Option<UserPayload>,
    #[serde(default)]
    has_replies: bool,
    #[serde(default)]
    parent_comment: Option<Box<CommentPayload>>,
    #[serde(default)]
    stamp: Option<CommentStampPayload>,
}

#[derive(Deserialize)]
struct CommentStampPayload {
    #[serde(default)]
    stamp_id: u64,
    #[serde(default)]
    stamp_url: String,
}

#[derive(Deserialize)]
struct NotificationsEnvelope {
    #[serde(default)]
    notifications: Vec<NotificationPayload>,
    #[serde(default)]
    next_url: Option<String>,
}

#[derive(Deserialize)]
struct AccessBlockEnvelope {
    #[serde(default)]
    blocked_users: Vec<UserPayload>,
    #[serde(default)]
    next_url: Option<String>,
}

#[derive(Deserialize)]
struct MuteSettingsEnvelope {
    #[serde(default)]
    muted_tags: Vec<MutedTagPayload>,
    #[serde(default)]
    muted_users: Vec<MutedUserPayload>,
    #[serde(default)]
    mute_limit_count: u32,
    #[serde(default)]
    for_text: MuteTextLimitsPayload,
}

#[derive(Deserialize)]
struct MutedTagPayload {
    tag: TagPayload,
    #[serde(default)]
    is_premium_slot: bool,
}

#[derive(Deserialize)]
struct MutedUserPayload {
    user: UserPayload,
    #[serde(default)]
    is_premium_slot: bool,
}

#[derive(Default, Deserialize)]
struct MuteTextLimitsPayload {
    #[serde(default)]
    mute_limit_count_if_no_premium: u32,
    #[serde(default)]
    mute_limit_count_if_premium: u32,
}

#[derive(Deserialize)]
struct StampListEnvelope {
    #[serde(default)]
    stamps: Vec<CommentStampPayload>,
}

#[derive(Default, Deserialize)]
struct NotificationPayload {
    #[serde(default)]
    id: u64,
    #[serde(default, rename = "type")]
    type_id: u32,
    #[serde(default)]
    is_read: bool,
    #[serde(default)]
    created_datetime: String,
    #[serde(default)]
    target_url: Option<String>,
    #[serde(default)]
    content: NotificationContentPayload,
    #[serde(default)]
    view_more: Option<NotificationViewMorePayload>,
}

#[derive(Default, Deserialize)]
struct NotificationContentPayload {
    #[serde(default)]
    text: String,
    #[serde(default)]
    left_icon: Option<String>,
    #[serde(default)]
    left_image: Option<String>,
    #[serde(default)]
    right_icon: Option<String>,
    #[serde(default)]
    right_image: Option<String>,
}

#[derive(Deserialize)]
struct NotificationViewMorePayload {
    #[serde(default)]
    title: String,
    #[serde(default)]
    unread_exists: bool,
}

#[derive(Deserialize)]
struct TrendingTagsEnvelope {
    #[serde(default)]
    trend_tags: Vec<TrendingTagPayload>,
}

#[derive(Deserialize)]
struct TrendingTagPayload {
    #[serde(default)]
    tag: String,
    #[serde(default)]
    translated_name: Option<String>,
    illust: IllustrationPayload,
}

#[derive(Deserialize)]
struct UserPreviewEnvelope {
    #[serde(default)]
    user_previews: Vec<UserPreviewPayload>,
    #[serde(default)]
    next_url: Option<String>,
}

#[derive(Deserialize)]
struct UserPreviewPayload {
    user: UserPayload,
    #[serde(default)]
    illusts: Vec<IllustrationPayload>,
    #[serde(default)]
    is_muted: bool,
}

#[derive(Deserialize)]
struct IllustrationPayload {
    id: u64,
    #[serde(default)]
    title: String,
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    image_urls: ImageUrlsPayload,
    user: UserPayload,
    #[serde(default = "one")]
    page_count: u32,
    #[serde(default)]
    width: u32,
    #[serde(default)]
    height: u32,
    #[serde(default)]
    is_bookmarked: Option<bool>,
    #[serde(default)]
    x_restrict: u8,
    #[serde(default)]
    sanity_level: u8,
    #[serde(default, rename = "illust_ai_type")]
    ai_type: u8,
    #[serde(default)]
    tags: Vec<TagPayload>,
    #[serde(default)]
    caption: String,
    #[serde(default)]
    create_date: String,
    #[serde(default)]
    meta_single_page: MetaSinglePagePayload,
    #[serde(default)]
    meta_pages: Vec<MetaPagePayload>,
    #[serde(default)]
    total_view: u64,
    #[serde(default)]
    total_bookmarks: u64,
    #[serde(default)]
    total_comments: u64,
    #[serde(default)]
    tools: Vec<String>,
    #[serde(default = "yes")]
    visible: bool,
    #[serde(default)]
    is_muted: bool,
    #[serde(default)]
    series: Option<SeriesPayload>,
}

#[derive(Default, Deserialize)]
struct ImageUrlsPayload {
    #[serde(default)]
    square_medium: Option<String>,
    #[serde(default)]
    medium: Option<String>,
    #[serde(default)]
    large: Option<String>,
    #[serde(default)]
    original: Option<String>,
}

#[derive(Default, Deserialize)]
struct MetaSinglePagePayload {
    #[serde(default)]
    original_image_url: Option<String>,
}

#[derive(Deserialize)]
struct MetaPagePayload {
    #[serde(default)]
    image_urls: ImageUrlsPayload,
}

#[derive(Deserialize)]
struct UserPayload {
    id: u64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    account: String,
    #[serde(default)]
    profile_image_urls: ProfileImageUrlsPayload,
    #[serde(default)]
    is_followed: bool,
    #[serde(default)]
    comment: String,
}

#[derive(Default, Deserialize)]
struct ProfileImageUrlsPayload {
    #[serde(default)]
    medium: Option<String>,
}

#[derive(Deserialize)]
struct TagPayload {
    #[serde(default)]
    name: String,
    #[serde(default)]
    translated_name: Option<String>,
}

#[derive(Deserialize)]
struct SeriesPayload {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    title: String,
}

#[derive(Default, Deserialize)]
struct UserProfilePayload {
    #[serde(default)]
    webpage: Option<String>,
    #[serde(default)]
    gender: String,
    #[serde(default)]
    birth: String,
    #[serde(default)]
    region: String,
    #[serde(default)]
    job: String,
    #[serde(default)]
    total_follow_users: u64,
    #[serde(default)]
    total_mypixiv_users: u64,
    #[serde(default)]
    total_illusts: u64,
    #[serde(default)]
    total_manga: u64,
    #[serde(default)]
    total_novels: u64,
    #[serde(default)]
    total_illust_bookmarks: u64,
    #[serde(default)]
    background_image_url: Option<String>,
    #[serde(default)]
    twitter_account: String,
    #[serde(default)]
    twitter_url: Option<String>,
    #[serde(default)]
    pawoo_url: Option<String>,
    #[serde(default)]
    is_premium: bool,
}

const fn one() -> u32 {
    1
}

const fn yes() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::{
        access_block_page_from_envelope, bookmark_detail_from_envelope,
        bookmark_tag_page_from_envelope, bookmark_update_parts, comment_page_from_envelope,
        comment_stamps_from_envelope, decode_cursor, encode_cursor, extract_embedded_novel_json,
        mute_settings_from_envelope, normalized_comment, normalized_comment_submission,
        notification_page_from_envelope, novel_page_from_envelope, page_from_envelope,
        recommended_url, user_preview_page_from_envelope, validated_media_url, AccessBlockEnvelope,
        ApiError, BookmarkContentKind, BookmarkDetailEnvelope, BookmarkTagsEnvelope,
        BookmarkUpdate, CommentsEnvelope, IllustrationDetail, IllustrationDetailEnvelope,
        IllustrationListEnvelope, IllustrationSeriesEnvelope, IllustrationSeriesPage,
        MuteSettingsEnvelope, NotificationsEnvelope, NovelContent, NovelContentPayload,
        NovelDetail, NovelDetailEnvelope, NovelListEnvelope, NovelSeriesEnvelope, NovelSeriesPage,
        StampListEnvelope, TrendingTag, TrendingTagsEnvelope, UgoiraEnvelope, UgoiraMetadata,
        UserDetail, UserDetailEnvelope, UserPreviewEnvelope, ILLUSTRATION_BOOKMARK_TAGS_PATH,
        ILLUSTRATION_COMMENTS_PATH, ILLUSTRATION_SERIES_PATH, NOVEL_COMMENTS_PATH,
        NOVEL_RECOMMENDED_PATH, NOVEL_SERIES_PATH, RECOMMENDED_PATH, RELATED_ILLUSTRATIONS_PATH,
        SEARCH_ILLUSTRATIONS_PATH, SEARCH_NOVELS_PATH, SEARCH_USERS_PATH, USER_FOLLOWING_PATH,
        USER_ILLUSTRATIONS_PATH,
    };

    const LIST_RESPONSE: &str = r#"{
      "illusts": [{
        "id": 123456789,
        "title": "Morning sky",
        "type": "illust",
        "image_urls": {
          "square_medium": "https://i.pximg.net/c/360x360_70/img-master/example.jpg",
          "medium": "https://i.pximg.net/c/540x540_70/img-master/example.jpg"
        },
        "user": {
          "id": 42,
          "name": "Alice",
          "account": "alice",
          "profile_image_urls": {"medium": "https://i.pximg.net/user-profile/example.jpg"},
          "is_followed": true
        },
        "page_count": 3,
        "width": 1200,
        "height": 1600,
        "is_bookmarked": true,
        "x_restrict": 1,
        "sanity_level": 6,
        "illust_ai_type": 2,
        "tags": [{"name": "原创"}, {"name": "风景"}]
      }],
      "next_url": "https://app-api.pixiv.net/v1/illust/recommended?filter=for_ios&offset=30"
    }"#;

    #[test]
    fn parses_bookmark_detail_and_tag_pages_from_the_official_shape() {
        let envelope: BookmarkDetailEnvelope = serde_json::from_str(
            r#"{
              "bookmark_detail": {
                "restrict": "private",
                "tags": [
                  {"name": "reference", "is_registered": true},
                  {"name": "blue", "is_registered": false}
                ]
              }
            }"#,
        )
        .unwrap();
        let detail = bookmark_detail_from_envelope(envelope).unwrap();
        assert_eq!(detail.restrict, "private");
        assert_eq!(detail.tags.len(), 2);
        assert!(detail.tags[0].is_registered);

        let envelope: BookmarkTagsEnvelope = serde_json::from_str(
            r#"{
              "bookmark_tags": [{"name": "reference", "count": 7}],
              "next_url": "https://app-api.pixiv.net/v1/user/bookmark-tags/illust?user_id=42&restrict=private&offset=30"
            }"#,
        )
        .unwrap();
        let page = bookmark_tag_page_from_envelope(
            envelope,
            ILLUSTRATION_BOOKMARK_TAGS_PATH,
            &[("user_id", "42"), ("restrict", "private")],
        )
        .unwrap();
        assert_eq!(page.tags[0].count, 7);
        assert!(decode_cursor(
            page.next_cursor.as_deref().unwrap(),
            ILLUSTRATION_BOOKMARK_TAGS_PATH,
            &[("user_id", "42"), ("restrict", "private")]
        )
        .is_ok());
    }

    #[test]
    fn bookmark_updates_keep_restrict_and_deduplicated_tags_together() {
        let update = BookmarkUpdate {
            kind: BookmarkContentKind::Illustration,
            resource_id: "42".into(),
            bookmarked: true,
            restrict: "private".into(),
            tags: vec![" Reference ".into(), "reference".into(), "blue".into()],
        };
        let (path, id_parameter, id, restrict, tags) = bookmark_update_parts(&update).unwrap();
        assert_eq!(path, super::BOOKMARK_ADD_PATH);
        assert_eq!(id_parameter, "illust_id");
        assert_eq!(id, "42");
        assert_eq!(restrict, "private");
        assert_eq!(tags, ["Reference", "blue"]);
    }

    #[test]
    fn parses_official_account_control_payloads_and_locks_access_block_cursor() {
        let blocked: AccessBlockEnvelope = serde_json::from_str(
            r#"{
              "blocked_users": [{
                "id": 42,
                "name": "Alice",
                "account": "alice",
                "profile_image_urls": {"medium": "https://i.pximg.net/user-profile/alice.jpg"}
              }],
              "next_url": "https://app-api.pixiv.net/v1/access-block/users?offset=30"
            }"#,
        )
        .expect("valid access-block response");
        let blocked =
            access_block_page_from_envelope(blocked).expect("validated access-block response");
        assert_eq!(blocked.users.len(), 1);
        assert_eq!(blocked.users[0].id, "42");
        assert!(blocked.next_cursor.is_some());

        let muted: MuteSettingsEnvelope = serde_json::from_str(
            r#"{
              "muted_tags": [{"tag": {"name": "spoiler", "translated_name": "剧透"}, "is_premium_slot": false}],
              "muted_users": [{
                "user": {"id": 43, "name": "Bob", "account": "bob"},
                "is_premium_slot": true
              }],
              "mute_limit_count": 10,
              "for_text": {
                "mute_limit_count_if_no_premium": 1,
                "mute_limit_count_if_premium": 500
              }
            }"#,
        )
        .expect("valid mute response");
        let muted = mute_settings_from_envelope(muted).expect("validated mute response");
        assert_eq!(muted.tags[0].name, "spoiler");
        assert_eq!(muted.users[0].user.id, "43");
        assert!(muted.users[0].is_premium_slot);
        assert_eq!(muted.limit_count, 10);
        assert_eq!(muted.text_limits.without_premium, 1);
        assert_eq!(muted.text_limits.with_premium, 500);
    }

    const DETAIL_RESPONSE: &str = r#"{
      "illust": {
        "id": 123456789,
        "title": "Morning sky",
        "type": "illust",
        "caption": "A <strong>bright</strong> morning.",
        "create_date": "2026-08-03T12:00:00+09:00",
        "image_urls": {"large": "https://i.pximg.net/c/600x1200/example-p0.jpg"},
        "user": {"id": 42, "name": "Alice", "account": "alice", "is_followed": true},
        "page_count": 2,
        "width": 1200,
        "height": 1600,
        "is_bookmarked": true,
        "tags": [{"name": "原创", "translated_name": "Original"}],
        "meta_pages": [
          {"image_urls": {"large": "https://i.pximg.net/c/600x1200/example-p0.jpg", "original": "https://i.pximg.net/img-original/example-p0.jpg"}},
          {"image_urls": {"large": "https://i.pximg.net/c/600x1200/example-p1.jpg", "original": "https://i.pximg.net/img-original/example-p1.jpg"}}
        ],
        "total_view": 5000,
        "total_bookmarks": 420,
        "total_comments": 12,
        "tools": ["CLIP STUDIO PAINT"],
        "visible": true,
        "series": {"id": 7, "title": "Sky series"}
      }
    }"#;

    const USER_RESPONSE: &str = r#"{
      "user": {
        "id": 42,
        "name": "Alice",
        "account": "alice",
        "comment": "Illustrator",
        "profile_image_urls": {"medium": "https://i.pximg.net/user-profile/alice.jpg"},
        "is_followed": true
      },
      "profile": {
        "webpage": "https://example.invalid",
        "region": "Tokyo",
        "job": "Creator",
        "total_follow_users": 88,
        "total_mypixiv_users": 3,
        "total_illusts": 120,
        "total_manga": 5,
        "total_novels": 2,
        "total_illust_bookmarks": 900,
        "background_image_url": "https://i.pximg.net/background/alice.jpg",
        "twitter_account": "alice",
        "twitter_url": "https://twitter.example/alice",
        "is_premium": true
      }
    }"#;

    const TRENDING_RESPONSE: &str = r#"{
      "trend_tags": [{
        "tag": "青空",
        "translated_name": "Blue sky",
        "illust": {
          "id": 99,
          "title": "Sky",
          "type": "illust",
          "image_urls": {"square_medium": "https://i.pximg.net/trend/sky.jpg"},
          "user": {"id": 42, "name": "Alice", "account": "alice"}
        }
      }]
    }"#;

    const USER_PREVIEW_RESPONSE: &str = r#"{
      "user_previews": [{
        "user": {
          "id": 42,
          "name": "Alice",
          "account": "alice",
          "profile_image_urls": {"medium": "https://i.pximg.net/user-profile/alice.jpg"}
        },
        "illusts": [{
          "id": 99,
          "title": "Sky",
          "type": "illust",
          "image_urls": {"square_medium": "https://i.pximg.net/user-preview/sky.jpg"},
          "user": {"id": 42, "name": "Alice", "account": "alice"}
        }],
        "is_muted": false
      }],
      "next_url": "https://app-api.pixiv.net/v1/search/user?word=Alice&offset=30"
    }"#;

    const COMMENTS_RESPONSE: &str = r#"{
      "total_comments": 2,
      "comments": [
        {
          "id": 701,
          "comment": "Great work!",
          "date": "2026-08-03T10:00:00+09:00",
          "user": {
            "id": 42,
            "name": "Alice",
            "account": "alice",
            "profile_image_urls": {"medium": "https://i.pximg.net/user-profile/alice.jpg"}
          },
          "has_replies": true,
          "parent_comment": {},
          "stamp": {
            "stamp_id": 501,
            "stamp_url": "https://s.pximg.net/common/images/emoji/501.png"
          }
        },
        {
          "id": 702,
          "comment": "Thank you!",
          "date": "2026-08-03T10:05:00+09:00",
          "user": {"id": 43, "name": "Bob", "account": "bob"},
          "parent_comment": {
            "id": 701,
            "comment": "Great work!",
            "user": {"id": 42, "name": "Alice", "account": "alice"}
          }
        }
      ],
      "next_url": "https://app-api.pixiv.net/v3/illust/comments?illust_id=99&offset=30",
      "comment_access_control": 0
    }"#;

    const NOTIFICATIONS_RESPONSE: &str = r#"{
      "notifications": [{
        "id": 901,
        "type": 12,
        "is_read": false,
        "created_datetime": "2026-08-11T10:00:00+09:00",
        "target_url": "https://www.pixiv.net/artworks/123",
        "content": {
          "text": "Alice bookmarked your work",
          "left_image": "https://i.pximg.net/user-profile/alice.jpg",
          "right_image": "https://i.pximg.net/c/360x360_70/example.jpg"
        },
        "view_more": {"title": "View more", "unread_exists": true}
      }],
      "next_url": "https://app-api.pixiv.net/v1/notification/list?limit=30&offset=30"
    }"#;

    const STAMPS_RESPONSE: &str = r#"{
      "stamps": [
        {"stamp_id": 501, "stamp_url": "https://s.pximg.net/common/images/emoji/501.png"},
        {"stamp_id": 0, "stamp_url": "https://s.pximg.net/common/images/emoji/invalid.png"},
        {"stamp_id": 502, "stamp_url": "https://example.com/not-allowed.png"}
      ]
    }"#;

    const NOVEL_RESPONSE: &str = r#"{
      "novels": [{
        "id": 8181,
        "title": "A quiet morning",
        "caption": "A short story",
        "image_urls": {
          "square_medium": "https://i.pximg.net/c/128x128/novel.jpg",
          "medium": "https://i.pximg.net/c/176x1200/novel.jpg",
          "large": "https://i.pximg.net/c/600x1200/novel.jpg"
        },
        "create_date": "2026-08-03T08:00:00+09:00",
        "tags": [{"name": "原创", "translated_name": "Original"}],
        "page_count": 3,
        "text_length": 4200,
        "user": {
          "id": 42,
          "name": "Alice",
          "account": "alice",
          "profile_image_urls": {"medium": "https://i.pximg.net/user-profile/alice.jpg"}
        },
        "series": {"id": 9, "title": "Morning series"},
        "is_bookmarked": true,
        "total_bookmarks": 50,
        "total_view": 900,
        "total_comments": 4,
        "visible": true,
        "novel_ai_type": 0
      }],
      "next_url": "https://app-api.pixiv.net/v1/novel/recommended?offset=30"
    }"#;

    const UGOIRA_RESPONSE: &str = r#"{
      "ugoira_metadata": {
        "zip_urls": {"medium": "https://i.pximg.net/img-zip-ugoira/example.zip"},
        "frames": [
          {"file": "000000.jpg", "delay": 80},
          {"file": "000001.jpg", "delay": 120}
        ]
      }
    }"#;

    #[test]
    fn maps_remote_json_into_stable_card_model() {
        let envelope: IllustrationListEnvelope = serde_json::from_str(LIST_RESPONSE).unwrap();
        let page = page_from_envelope(envelope, RECOMMENDED_PATH, &[]).unwrap();
        let card = &page.illustrations[0];

        assert_eq!(card.id, "123456789");
        assert_eq!(card.title, "Morning sky");
        assert_eq!(card.author.name, "Alice");
        assert_eq!(card.page_count, 3);
        assert!(card.is_bookmarked);
        assert_eq!(card.x_restrict, 1);
        assert_eq!(card.ai_type, 2);
        assert_eq!(card.tags, ["原创", "风景"]);
        assert!(page.next_cursor.is_some());
    }

    #[test]
    fn maps_detail_pages_stats_tags_and_series() {
        let envelope: IllustrationDetailEnvelope = serde_json::from_str(DETAIL_RESPONSE).unwrap();
        let detail = IllustrationDetail::from_payload(envelope.illust).unwrap();

        assert_eq!(detail.illustration.id, "123456789");
        assert_eq!(detail.pages.len(), 2);
        assert_eq!(detail.pages[1].page_index, 1);
        assert!(detail.pages[0]
            .original_url
            .as_deref()
            .is_some_and(|url| url.ends_with("example-p0.jpg")));
        assert_eq!(detail.total_views, 5000);
        assert_eq!(detail.tags[0].translated_name.as_deref(), Some("Original"));
        assert_eq!(
            detail.series.as_ref().map(|series| series.id.as_str()),
            Some("7")
        );
    }

    #[test]
    fn maps_user_profile_and_rejects_non_pixiv_backgrounds() {
        let envelope: UserDetailEnvelope = serde_json::from_str(USER_RESPONSE).unwrap();
        let detail = UserDetail::from_envelope(envelope).unwrap();

        assert_eq!(detail.user.id, "42");
        assert_eq!(detail.comment, "Illustrator");
        assert_eq!(detail.profile.total_illustrations, 120);
        assert_eq!(detail.profile.total_follow_users, 88);
        assert!(detail.profile.background_image_url.is_some());
        assert!(detail.profile.is_premium);
    }

    #[test]
    fn maps_trending_tags_and_user_search_previews() {
        let envelope: TrendingTagsEnvelope = serde_json::from_str(TRENDING_RESPONSE).unwrap();
        let tags: Vec<_> = envelope
            .trend_tags
            .iter()
            .filter_map(TrendingTag::from_payload)
            .collect();
        assert_eq!(tags[0].name, "青空");
        assert_eq!(tags[0].translated_name.as_deref(), Some("Blue sky"));
        assert_eq!(tags[0].illustration.id, "99");

        let envelope: UserPreviewEnvelope = serde_json::from_str(USER_PREVIEW_RESPONSE).unwrap();
        let page =
            user_preview_page_from_envelope(envelope, SEARCH_USERS_PATH, &[("word", "Alice")])
                .unwrap();
        assert_eq!(page.users[0].user.id, "42");
        assert_eq!(page.users[0].illustrations[0].id, "99");
        assert!(page.next_cursor.is_some());
    }

    #[test]
    fn followed_user_page_locks_cursor_to_owner_visibility_and_endpoint() {
        let mut envelope: UserPreviewEnvelope =
            serde_json::from_str(USER_PREVIEW_RESPONSE).unwrap();
        envelope.next_url = Some(
            "https://app-api.pixiv.net/v1/user/following?user_id=42&restrict=public&offset=30"
                .to_owned(),
        );
        let page = user_preview_page_from_envelope(
            envelope,
            USER_FOLLOWING_PATH,
            &[("user_id", "42"), ("restrict", "public")],
        )
        .unwrap();
        let cursor = page.next_cursor.unwrap();
        assert!(decode_cursor(
            &cursor,
            USER_FOLLOWING_PATH,
            &[("user_id", "42"), ("restrict", "public")]
        )
        .is_ok());
        assert_eq!(
            decode_cursor(
                &cursor,
                USER_FOLLOWING_PATH,
                &[("user_id", "43"), ("restrict", "public")]
            ),
            Err(ApiError::InvalidCursor)
        );
        assert_eq!(
            decode_cursor(
                &cursor,
                USER_FOLLOWING_PATH,
                &[("user_id", "42"), ("restrict", "private")]
            ),
            Err(ApiError::InvalidCursor)
        );
        assert_eq!(
            decode_cursor(
                &cursor,
                SEARCH_USERS_PATH,
                &[("user_id", "42"), ("restrict", "public")]
            ),
            Err(ApiError::InvalidCursor)
        );
    }

    #[test]
    fn maps_comments_replies_and_locks_comment_cursor_to_the_illustration() {
        let envelope: CommentsEnvelope = serde_json::from_str(COMMENTS_RESPONSE).unwrap();
        let page = comment_page_from_envelope(
            envelope,
            ILLUSTRATION_COMMENTS_PATH,
            &[("illust_id", "99")],
        )
        .unwrap();

        assert_eq!(page.total_comments, 2);
        assert_eq!(page.comments[0].user.as_ref().unwrap().name, "Alice");
        assert!(page.comments[0].has_replies);
        assert_eq!(page.comments[0].stamp.as_ref().unwrap().id, "501");
        assert_eq!(
            page.comments[0].stamp.as_ref().unwrap().url,
            "https://s.pximg.net/common/images/emoji/501.png"
        );
        assert_eq!(page.comments[1].parent.as_ref().unwrap().id, "701");
        assert_eq!(page.comments[1].parent.as_ref().unwrap().user_name, "Alice");
        let cursor = page.next_cursor.unwrap();
        assert!(decode_cursor(&cursor, ILLUSTRATION_COMMENTS_PATH, &[("illust_id", "99")]).is_ok());
        assert_eq!(
            decode_cursor(&cursor, ILLUSTRATION_COMMENTS_PATH, &[("illust_id", "100")]),
            Err(ApiError::InvalidCursor)
        );
    }

    #[test]
    fn comment_input_is_trimmed_bounded_and_rejects_control_characters() {
        assert_eq!(normalized_comment("  hello  ").unwrap(), "hello");
        assert!(normalized_comment(&"好".repeat(140)).is_ok());
        assert_eq!(
            normalized_comment(&"好".repeat(141)),
            Err(ApiError::InvalidInput)
        );
        assert_eq!(
            normalized_comment("bad\u{0}input"),
            Err(ApiError::InvalidInput)
        );
    }

    #[test]
    fn maps_novel_lists_details_and_locks_pagination() {
        let envelope: NovelListEnvelope = serde_json::from_str(NOVEL_RESPONSE).unwrap();
        let page = novel_page_from_envelope(envelope, NOVEL_RECOMMENDED_PATH, &[]).unwrap();
        let novel = &page.novels[0];
        assert_eq!(novel.id, "8181");
        assert_eq!(novel.author.name, "Alice");
        assert_eq!(novel.text_length, 4200);
        assert_eq!(novel.series.as_ref().unwrap().id, "9");
        assert!(novel.is_bookmarked);
        assert!(page.next_cursor.is_some());

        let envelope: NovelDetailEnvelope = serde_json::from_str(
            &NOVEL_RESPONSE.replace("\"novels\": [", "\"novel\": ").replace(
                "}],\n      \"next_url\": \"https://app-api.pixiv.net/v1/novel/recommended?offset=30\"",
                "}",
            ),
        )
        .unwrap();
        let detail = NovelDetail::from_payload(envelope.novel).unwrap();
        assert!(detail.visible);
        assert_eq!(detail.novel.total_views, 900);

        let search_response = NOVEL_RESPONSE.replace(
            "https://app-api.pixiv.net/v1/novel/recommended?offset=30",
            "https://app-api.pixiv.net/v1/search/novel?word=sky&search_target=partial_match_for_tags&offset=30",
        );
        let envelope: NovelListEnvelope = serde_json::from_str(&search_response).unwrap();
        let page = novel_page_from_envelope(
            envelope,
            SEARCH_NOVELS_PATH,
            &[("word", "sky"), ("search_target", "partial_match_for_tags")],
        )
        .unwrap();
        let cursor = page.next_cursor.unwrap();
        assert_eq!(
            decode_cursor(
                &cursor,
                SEARCH_NOVELS_PATH,
                &[
                    ("word", "different"),
                    ("search_target", "partial_match_for_tags")
                ],
            ),
            Err(ApiError::InvalidCursor)
        );
    }

    #[test]
    fn maps_illustration_series_and_locks_its_cursor() {
        let work = serde_json::json!({
            "id": 501,
            "title": "Chapter one",
            "type": "manga",
            "image_urls": {"square_medium": "https://i.pximg.net/series/work.jpg"},
            "user": {"id": 42, "name": "Alice", "account": "alice"}
        });
        let envelope: IllustrationSeriesEnvelope = serde_json::from_value(serde_json::json!({
            "illust_series_detail": {
                "id": 77,
                "title": "Sky chapters",
                "caption": "A connected story",
                "cover_image_urls": {"medium": "https://i.pximg.net/series/cover.jpg"},
                "series_work_count": 3,
                "create_date": "2026-08-03T00:00:00+09:00",
                "width": 1200,
                "height": 1600,
                "user": {"id": 42, "name": "Alice", "account": "alice"},
                "watchlist_added": true
            },
            "illust_series_first_illust": work.clone(),
            "illusts": [work],
            "next_url": "https://app-api.pixiv.net/v1/illust/series?illust_series_id=77&offset=30"
        }))
        .unwrap();
        let page = IllustrationSeriesPage::from_envelope(envelope, "77").unwrap();

        assert_eq!(page.series.id, "77");
        assert_eq!(page.series.work_count, 3);
        assert_eq!(page.first_illustration.id, "501");
        assert_eq!(page.illustrations.len(), 1);
        let cursor = page.next_cursor.unwrap();
        assert!(decode_cursor(
            &cursor,
            ILLUSTRATION_SERIES_PATH,
            &[("illust_series_id", "77")]
        )
        .is_ok());
        assert_eq!(
            decode_cursor(
                &cursor,
                ILLUSTRATION_SERIES_PATH,
                &[("illust_series_id", "78")]
            ),
            Err(ApiError::InvalidCursor)
        );
    }

    #[test]
    fn maps_novel_series_and_locks_its_cursor() {
        let novel = serde_json::json!({
            "id": 801,
            "title": "Part one",
            "user": {"id": 42, "name": "Alice", "account": "alice"}
        });
        let envelope: NovelSeriesEnvelope = serde_json::from_value(serde_json::json!({
            "novel_series_detail": {
                "id": 88,
                "title": "Morning stories",
                "caption": "A serial novel",
                "is_original": true,
                "is_concluded": false,
                "content_count": 4,
                "total_character_count": 12000,
                "user": {"id": 42, "name": "Alice", "account": "alice"},
                "display_text": "4 works",
                "novel_ai_type": 0,
                "watchlist_added": false
            },
            "novel_series_first_novel": novel.clone(),
            "novel_series_latest_novel": novel.clone(),
            "novels": [novel],
            "next_url": "https://app-api.pixiv.net/v2/novel/series?series_id=88&last_order=4"
        }))
        .unwrap();
        let page = NovelSeriesPage::from_envelope(envelope, "88").unwrap();

        assert_eq!(page.series.id, "88");
        assert_eq!(page.series.total_character_count, 12000);
        assert_eq!(page.first_novel.id, "801");
        let cursor = page.next_cursor.unwrap();
        assert!(decode_cursor(&cursor, NOVEL_SERIES_PATH, &[("series_id", "88")]).is_ok());
        assert_eq!(
            decode_cursor(&cursor, NOVEL_SERIES_PATH, &[("series_id", "89")]),
            Err(ApiError::InvalidCursor)
        );
    }

    #[test]
    fn novel_comment_cursor_is_locked_to_the_novel() {
        let response = COMMENTS_RESPONSE.replace(
            "https://app-api.pixiv.net/v3/illust/comments?illust_id=99&offset=30",
            "https://app-api.pixiv.net/v3/novel/comments?novel_id=8181&offset=30",
        );
        let envelope: CommentsEnvelope = serde_json::from_str(&response).unwrap();
        let page =
            comment_page_from_envelope(envelope, NOVEL_COMMENTS_PATH, &[("novel_id", "8181")])
                .unwrap();
        let cursor = page.next_cursor.unwrap();
        assert!(decode_cursor(&cursor, NOVEL_COMMENTS_PATH, &[("novel_id", "8181")]).is_ok());
        assert_eq!(
            decode_cursor(&cursor, NOVEL_COMMENTS_PATH, &[("novel_id", "8182")]),
            Err(ApiError::InvalidCursor)
        );
    }

    #[test]
    fn extracts_balanced_novel_json_without_accepting_a_different_id() {
        let html = r#"<script>window.__DATA__ = { novel: {"id":"8181","title":"Brace } in text","text":"hello \"world\" {x}","coverUrl":"https://i.pximg.net/novel.jpg","seriesId":"9","seriesTitle":"Morning series","seriesNavigation":{"prev":{"id":8180,"title":"Previous","coverUrl":"https://i.pximg.net/prev.jpg","contentOrder":"1","viewable":true},"next":{"id":8182,"title":"Members only","coverUrl":"https://attacker.example/next.jpg","contentOrder":"3","viewable":false,"viewableMessage":"不可查看"}},"illusts":["99","bad"],"images":["100"]}, isOwnWork: false };</script>"#;
        let json = extract_embedded_novel_json(html).unwrap();
        let payload: NovelContentPayload = serde_json::from_str(json).unwrap();
        let content = NovelContent::from_payload(payload, "8181").unwrap();
        assert_eq!(content.illustration_ids, ["99"]);
        assert_eq!(content.image_ids, ["100"]);
        assert!(content.text.contains("{x}"));
        assert_eq!(
            content.series_navigation.previous.as_ref().unwrap().id,
            "8180"
        );
        assert!(content
            .series_navigation
            .previous
            .as_ref()
            .unwrap()
            .cover_url
            .is_some());
        assert!(!content.series_navigation.next.as_ref().unwrap().viewable);
        assert!(content
            .series_navigation
            .next
            .as_ref()
            .unwrap()
            .cover_url
            .is_none());

        let payload: NovelContentPayload = serde_json::from_str(json).unwrap();
        assert_eq!(
            NovelContent::from_payload(payload, "8182"),
            Err(ApiError::InvalidResponse)
        );

        let multiple_markers = r#"novel: no object follows<script>novel: {"id":"8181","title":"Valid","text":"body"}</script>"#;
        let json = extract_embedded_novel_json(multiple_markers).unwrap();
        let payload: NovelContentPayload = serde_json::from_str(json).unwrap();
        assert_eq!(
            NovelContent::from_payload(payload, "8181").unwrap().text,
            "body"
        );
    }

    #[test]
    fn validates_ugoira_archive_and_frame_metadata() {
        let envelope: UgoiraEnvelope = serde_json::from_str(UGOIRA_RESPONSE).unwrap();
        let metadata = UgoiraMetadata::from_payload(envelope.ugoira_metadata).unwrap();
        assert_eq!(metadata.frames.len(), 2);
        assert_eq!(metadata.frames[1].delay_ms, 120);
        assert!(metadata.zip_url.ends_with("example.zip"));

        let hostile = UGOIRA_RESPONSE.replace("000000.jpg", "../000000.jpg");
        let envelope: UgoiraEnvelope = serde_json::from_str(&hostile).unwrap();
        assert_eq!(
            UgoiraMetadata::from_payload(envelope.ugoira_metadata),
            Err(ApiError::InvalidResponse)
        );
    }

    #[test]
    fn maps_read_only_notifications_and_locks_pagination_to_the_list_endpoint() {
        let envelope: NotificationsEnvelope = serde_json::from_str(NOTIFICATIONS_RESPONSE).unwrap();
        let page = notification_page_from_envelope(envelope).unwrap();
        assert_eq!(page.notifications.len(), 1);
        assert_eq!(page.notifications[0].id, "901");
        assert_eq!(
            page.notifications[0].content.text,
            "Alice bookmarked your work"
        );
        assert_eq!(
            page.notifications[0].target_url.as_deref(),
            Some("https://www.pixiv.net/artworks/123")
        );
        assert!(
            page.notifications[0]
                .view_more
                .as_ref()
                .unwrap()
                .unread_exists
        );
        assert!(page.next_cursor.is_some());

        let hostile = NOTIFICATIONS_RESPONSE.replace("app-api.pixiv.net", "example.com");
        let envelope: NotificationsEnvelope = serde_json::from_str(&hostile).unwrap();
        assert_eq!(
            notification_page_from_envelope(envelope),
            Err(ApiError::InvalidResponse)
        );
    }

    #[test]
    fn supports_official_comment_stamps_and_stamp_only_submissions() {
        let envelope: StampListEnvelope = serde_json::from_str(STAMPS_RESPONSE).unwrap();
        let stamps = comment_stamps_from_envelope(envelope);
        assert_eq!(stamps.len(), 1);
        assert_eq!(stamps[0].id, "501");

        assert_eq!(
            normalized_comment_submission("", Some("501")).unwrap(),
            (String::new(), Some(String::from("501")))
        );
        assert_eq!(
            normalized_comment_submission("hello", None).unwrap(),
            (String::from("hello"), None)
        );
        assert_eq!(
            normalized_comment_submission("", None),
            Err(ApiError::InvalidInput)
        );
        assert_eq!(
            normalized_comment_submission("hello", Some("bad")),
            Err(ApiError::InvalidIdentifier)
        );
    }

    #[test]
    fn cursor_is_opaque_and_locked_to_endpoint_and_resource() {
        let first = recommended_url(None).unwrap();
        assert_eq!(first.host_str(), Some("app-api.pixiv.net"));
        assert!(first.as_str().contains("filter=for_ios"));

        let related =
            url::Url::parse("https://app-api.pixiv.net/v2/illust/related?illust_id=42&offset=30")
                .unwrap();
        let cursor = encode_cursor(&related);
        assert!(!cursor.contains("app-api.pixiv.net"));
        assert_eq!(
            decode_cursor(&cursor, RELATED_ILLUSTRATIONS_PATH, &[("illust_id", "42")]).unwrap(),
            related
        );
        assert_eq!(
            decode_cursor(&cursor, RELATED_ILLUSTRATIONS_PATH, &[("illust_id", "43")]),
            Err(ApiError::InvalidCursor)
        );

        let search = url::Url::parse(
            "https://app-api.pixiv.net/v1/search/illust?word=sky&search_target=partial_match_for_tags&offset=30",
        )
        .unwrap();
        let search_cursor = encode_cursor(&search);
        assert!(decode_cursor(
            &search_cursor,
            SEARCH_ILLUSTRATIONS_PATH,
            &[("word", "sky"), ("search_target", "partial_match_for_tags")]
        )
        .is_ok());
        assert_eq!(
            decode_cursor(
                &search_cursor,
                SEARCH_ILLUSTRATIONS_PATH,
                &[
                    ("word", "different"),
                    ("search_target", "partial_match_for_tags")
                ]
            ),
            Err(ApiError::InvalidCursor)
        );

        let user_search =
            url::Url::parse("https://app-api.pixiv.net/v1/search/user?word=Alice&offset=30")
                .unwrap();
        assert!(decode_cursor(
            &encode_cursor(&user_search),
            SEARCH_USERS_PATH,
            &[("word", "Alice")]
        )
        .is_ok());

        let hostile =
            url::Url::parse("https://example.com/v1/user/illusts?user_id=42&type=illust&offset=30")
                .unwrap();
        assert_eq!(
            decode_cursor(
                &encode_cursor(&hostile),
                USER_ILLUSTRATIONS_PATH,
                &[("user_id", "42"), ("type", "illust")]
            ),
            Err(ApiError::InvalidCursor)
        );
    }

    #[test]
    fn media_urls_are_restricted_to_pixiv_cdn_hosts() {
        assert!(validated_media_url("https://i.pximg.net/example.jpg").is_ok());
        assert!(validated_media_url("https://s.pximg.net/example.png").is_ok());
        assert_eq!(
            validated_media_url("https://i.pximg.net.attacker.example/example.jpg"),
            Err(ApiError::InvalidMediaUrl)
        );
        assert_eq!(
            validated_media_url("http://i.pximg.net/example.jpg"),
            Err(ApiError::InvalidMediaUrl)
        );
    }

    #[test]
    fn serialized_models_contain_no_authentication_material() {
        let envelope: IllustrationDetailEnvelope = serde_json::from_str(DETAIL_RESPONSE).unwrap();
        let serialized =
            serde_json::to_string(&IllustrationDetail::from_payload(envelope.illust).unwrap())
                .unwrap();

        assert!(!serialized.to_ascii_lowercase().contains("token"));
        assert!(!serialized.contains("Authorization"));
    }

    #[test]
    fn pixiv_oauth_failures_returned_as_http_400_require_a_refresh() {
        assert_eq!(
            super::classify_rejection(
                400,
                r#"{"error":{"message":"Error occurred at the OAuth process. Please check your Access Token to fix this. (invalid_grant)"}}"#,
            ),
            ApiError::AuthenticationRequired
        );
        assert_eq!(
            super::classify_rejection(400, r#"{"error":{"message":"invalid offset"}}"#),
            ApiError::Rejected { http_status: 400 }
        );
    }
}
