//! `FakePeople`: the people of the frontend (picker, names), an adapter over the fake of the locked `person_directory_v1`.
//!
//! Pattern: Adapter (stub). Tasks keep only `PersonRef`s (ADR 0085); who they are and who can be suggested comes from
//! the `PersonDirectory` of fs-people. This is its `FakeDirectory` filled from [`KNOWN`] (the invented people of the
//! sample, `example.org`: same names, kinds and AI mark as the tasks show), handed on unchanged; the plan has no
//! tasks-specific people trait.

use crate::sample_people::KNOWN;
use fs_people_directory::{
    Candidate, ContactAddress, DirectoryChange, Entry, Excerpt, ExcerptField, FakeDirectory,
    PersonDirectory, PersonView, ProposeEntry, Resolved, Suggest,
};
use fs_types::{Capabilities, ObjectId, Page, PageCursor, PersonRef, Reference};

/// A `PersonDirectory` with the fixed invented people of fs-people.
pub struct FakePeople {
    directory: FakeDirectory,
}

impl FakePeople {
    /// The directory of the people of [`KNOWN`]; the asker is person 1.
    #[must_use]
    pub fn new() -> Self {
        let directory = FakeDirectory::new().with_asker(KNOWN.first().map(crate::Known::person));
        for k in KNOWN {
            let mut entry = Entry::new(k.person(), k.display);
            entry.address = ContactAddress::parse(&format!("mailto:{}", k.address)).ok();
            entry.ai = k.ai;
            directory.add(entry);
        }
        Self { directory }
    }

    /// The reference of sample person `n` (`None` when there is none).
    #[must_use]
    pub fn person(n: u8) -> Option<PersonRef> {
        KNOWN
            .iter()
            .find(|k| k.number == u64::from(n))
            .map(crate::Known::person)
    }

    /// The directory inside, for tests that change it.
    #[must_use]
    pub fn directory(&self) -> &FakeDirectory {
        &self.directory
    }
}

impl Default for FakePeople {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl PersonDirectory for FakePeople {
    fn capabilities(&self) -> Capabilities {
        self.directory.capabilities()
    }
    async fn resolve(&self, person: &PersonRef) -> fs_people_directory::Result<Resolved> {
        self.directory.resolve(person).await
    }
    async fn asker(&self) -> fs_people_directory::Result<Option<PersonRef>> {
        self.directory.asker().await
    }
    async fn resolve_many(
        &self,
        people: &[PersonRef],
    ) -> fs_people_directory::Result<Vec<Resolved>> {
        self.directory.resolve_many(people).await
    }
    async fn changes(
        &self,
        since: Option<&PageCursor>,
        size: usize,
    ) -> fs_people_directory::Result<Page<DirectoryChange>> {
        self.directory.changes(since, size).await
    }
    async fn find_by_address(
        &self,
        address: &ContactAddress,
    ) -> fs_people_directory::Result<Vec<PersonView>> {
        self.directory.find_by_address(address).await
    }
    async fn suggest(&self, query: &Suggest) -> fs_people_directory::Result<Vec<Candidate>> {
        self.directory.suggest(query).await
    }
    async fn open_batch(&self, origin: &Reference) -> fs_people_directory::Result<ObjectId> {
        self.directory.open_batch(origin).await
    }
    async fn propose(
        &self,
        entry: &ProposeEntry,
        batch: Option<ObjectId>,
    ) -> fs_people_directory::Result<PersonRef> {
        self.directory.propose(entry, batch).await
    }
    async fn excerpt(
        &self,
        person: &PersonRef,
        fields: &[ExcerptField],
    ) -> fs_people_directory::Result<Excerpt> {
        self.directory.excerpt(person, fields).await
    }
}
