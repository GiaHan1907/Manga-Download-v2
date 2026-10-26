use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Capability {
    Search,
    ResolveUrl,
    Details,
    /// Adapter can resolve an ordered page manifest and fetch page images.
    /// UI only offers download controls for sources declaring this capability.
    Pages,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceStatus {
    Ready,
    /// Contract value: disabled state is surfaced through `SourceView.enabled`
    /// in Phase 05; reserved for the provider-side availability model.
    #[allow(dead_code)]
    Disabled,
    #[allow(dead_code)]
    Unavailable,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    pub id: String,
    pub display_name: String,
    pub capabilities: Vec<Capability>,
    pub status: SourceStatus,
    pub attribution: Option<String>,
}

/// Remote search result item. `source_id`/`source_manga_id` form the stable
/// identity; titles are display data only and never identity.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteMangaSummary {
    pub source_id: String,
    pub source_manga_id: String,
    pub source_display_name: String,
    pub title: String,
    pub author: Option<String>,
    /// Safe internal cover reference (file key), never a fetchable remote URL.
    pub cover_key: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteChapter {
    pub source_chapter_id: String,
    pub title: String,
    pub number: Option<f64>,
    /// Adapter-defined stable display order (independent of `number`).
    pub order: i64,
    pub published_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteMangaDetail {
    #[serde(flatten)]
    pub summary: RemoteMangaSummary,
    pub description: Option<String>,
    pub genres: Vec<String>,
    pub canonical_url: Option<String>,
    pub chapters: Vec<RemoteChapter>,
    /// Internal catalog id when this manga is already saved.
    pub saved_manga_id: Option<String>,
    /// Truthful in Phase 05: metadata is not downloaded content.
    pub available_offline: bool,
}

#[derive(Debug, Clone)]
pub struct RemoteSearchPage {
    pub items: Vec<RemoteMangaSummary>,
    pub has_more: bool,
}

/// One genre the source itself publishes. `slug` is the token the source uses
/// in its own catalog URL and `label` is the source's wording — neither is
/// invented or translated locally.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GenreTag {
    pub slug: String,
    pub label: String,
}

/// One Home card plus the chapter label the source itself printed next to it.
/// Nothing else is invented: no rating, no synopsis, no view count.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeFeedItem {
    #[serde(flatten)]
    pub summary: RemoteMangaSummary,
    pub latest_chapter: Option<String>,
    /// Relative age the source itself printed for that chapter, e.g.
    /// "37 phút trước". Never computed locally, so it cannot drift off the
    /// source's own wording.
    pub latest_time: Option<String>,
}

/// A source's own front page, split into the three rails the Home screen
/// renders. Adapters that have no front page return
/// `SourceError::UnsupportedOperation`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteHomeFeed {
    /// The source's featured/spotlight rail (drives the hero).
    pub featured: Vec<HomeFeedItem>,
    /// Newest updated stories.
    pub newest: Vec<HomeFeedItem>,
    /// Most-read ranking.
    pub popular: Vec<HomeFeedItem>,
}

/// One ordered page in a chapter manifest. The `url` is source-controlled and
/// internal to Rust only — it is never serialized to the renderer. Image bytes
/// are fetched by the engine through the owning adapter's `fetch_image`.
#[derive(Debug, Clone)]
pub struct PageDescriptor {
    pub index: usize,
    pub url: String,
}

/// Ordered page set for one chapter. Identities are the adapter's stable
/// chapter id plus page index; a re-resolved manifest must reconcile against
/// stored checkpoints rather than trusting new list positions blindly.
#[derive(Debug, Clone)]
pub struct PageManifest {
    pub source_chapter_id: String,
    pub pages: Vec<PageDescriptor>,
    /// `Some("pdf")` when the chapter is ONE downloadable file (a scanned
    /// volume) instead of a page list: `pages` then holds exactly one
    /// descriptor pointing at that file, the worker stores the bytes as-is,
    /// and the artifact keeps that extension. The offline reader cannot render
    /// it — it is opened with an external application.
    pub single_file_format: Option<String>,
}
