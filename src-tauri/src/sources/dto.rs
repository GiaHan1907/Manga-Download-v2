use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Capability {
    Search,
    ResolveUrl,
    Details,
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
