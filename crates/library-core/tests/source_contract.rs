//! The `LibrarySource` contract: usable as a trait object, writes refused by default.

use std::collections::HashSet;
use std::sync::Mutex;

use async_trait::async_trait;
use library_core::{
    AddTarget, Capabilities, ChangeToken, Folder, FolderChange, Item, ItemEdit, ItemId, ItemLite,
    LibraryInfo, LibraryKind, LibrarySource, NewFile, Progress, Scope, ScopeHints, SmartFolder,
    SourceError, Tag, TagGroup,
};

/// A read-only source with two items, implementing only the required methods.
struct FakeSource;

#[async_trait]
impl LibrarySource for FakeSource {
    fn capabilities(&self) -> Capabilities {
        Capabilities::READ_ONLY
    }

    async fn info(&self) -> Result<LibraryInfo, SourceError> {
        Ok(LibraryInfo {
            kind: LibraryKind::EagleFolder,
            name: "fake".into(),
            path: None,
            modification_time: None,
        })
    }

    async fn folder_tree(&self) -> Result<Vec<Folder>, SourceError> {
        Ok(vec![])
    }

    async fn load_scope(
        &self,
        _scope: &Scope,
        _hints: &ScopeHints,
        progress: Progress<'_>,
    ) -> Result<Vec<ItemLite>, SourceError> {
        let items = vec![
            ItemLite {
                id: ItemId::from("A"),
                ..ItemLite::default()
            },
            ItemLite {
                id: ItemId::from("B"),
                ..ItemLite::default()
            },
        ];
        progress(items.len(), items.len());
        Ok(items)
    }

    async fn search_ids(&self, _text: &str) -> Result<HashSet<ItemId>, SourceError> {
        Ok(HashSet::new())
    }

    async fn item(&self, _id: &ItemId) -> Result<Option<Item>, SourceError> {
        Ok(None)
    }

    async fn tags(&self) -> Result<Vec<Tag>, SourceError> {
        Ok(vec![])
    }

    async fn tag_groups(&self) -> Result<Vec<TagGroup>, SourceError> {
        Ok(vec![])
    }

    async fn smart_folders(&self) -> Result<Vec<SmartFolder>, SourceError> {
        Ok(vec![])
    }
}

fn boxed() -> Box<dyn LibrarySource> {
    Box::new(FakeSource)
}

#[tokio::test]
async fn works_as_a_trait_object_and_reports_progress() {
    let source = boxed();
    let calls = Mutex::new(Vec::new());
    let items = source
        .load_scope(&Scope::All, &ScopeHints::default(), &|loaded, total| {
            calls.lock().unwrap().push((loaded, total));
        })
        .await
        .unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(*calls.lock().unwrap(), [(2, 2)]);
    assert_eq!(source.info().await.unwrap().kind, LibraryKind::EagleFolder);
}

#[tokio::test]
async fn writes_are_refused_by_default() {
    let source = boxed();
    let id = ItemId::from("A");
    assert_eq!(
        source
            .apply_item_edit(&id, ItemEdit::Trash)
            .await
            .unwrap_err(),
        SourceError::ReadOnly
    );
    assert_eq!(
        source
            .add_files(
                vec![NewFile {
                    path: "a.jpg".into(),
                    name: None
                }],
                AddTarget::default()
            )
            .await
            .unwrap_err(),
        SourceError::ReadOnly
    );
    let change = FolderChange::Create {
        name: "x".into(),
        parent: None,
        description: String::new(),
        icon_color: None,
    };
    assert_eq!(
        source.save_folder(change).await.unwrap_err(),
        SourceError::ReadOnly
    );
    assert_eq!(
        source.rename_tag("a", "b").await.unwrap_err(),
        SourceError::ReadOnly
    );
    assert_eq!(
        source.merge_tags("a", "b").await.unwrap_err(),
        SourceError::ReadOnly
    );
}

#[tokio::test]
async fn optional_reads_have_safe_defaults() {
    let source = boxed();
    assert_eq!(source.thumbnail(&ItemId::from("A")).await.unwrap(), None);
    assert!(matches!(
        source.changes_since(&ChangeToken::default()).await,
        Err(SourceError::NotSupported(_))
    ));
}

#[test]
fn read_only_capabilities_allow_nothing() {
    // Default capabilities are the read-only ones, so a new source starts safe.
    assert_eq!(Capabilities::READ_ONLY, Capabilities::default());
}
