//! Who may see and do what: the access of the asking person to a list, the claim rule and the AI rules.
//!
//! Pattern: Policy (small questions of the state, each one a rule of `tasks_v1/mod.rs`: EIGENTUM, ZUSTÄNDIGE UND
//! ÜBERNAHME). The groups are asked at the time of the question (`GroupMembers::memberships_of`), never stored.

use crate::mutation::Mutation;
use crate::state::State;
use crate::tasks_v1::{
    Error, ListAccess, ObjectId, PersonArt, PersonRef, Task, TaskList, TaskResult,
};

fn is_group(p: &PersonRef) -> bool {
    matches!(p.art, PersonArt::Group | PersonArt::Organization)
}

fn forbidden(capability: &str) -> Error {
    Error::Forbidden {
        capability: capability.to_owned(),
    }
}

impl State {
    /// Whether the asking person is an AI.
    pub(crate) fn asker_is_ai(&self) -> bool {
        self.groups.is_agent(&self.asker())
    }

    /// The groups the asking person is in now.
    fn my_groups(&self) -> Vec<PersonRef> {
        self.groups.memberships_of(&self.asker())
    }

    /// What the asking person may do with the list (`None` = may not see it). An AI gets at most `Write`.
    pub(crate) fn access_of(&self, list: &TaskList) -> Option<ListAccess> {
        let asker = self.asker();
        let base = if self.is_me(&list.owner) || list.owner.id == asker.id {
            Some(ListAccess::Manage)
        } else if self.acting.is_some() && list.owner == self.account {
            // Somebody acting (a colleague, an AI) works in the lists of the account with `Write`.
            Some(ListAccess::Write)
        } else if is_group(&list.owner) {
            if self.groups.manages(&asker, &list.owner) {
                Some(ListAccess::Manage)
            } else if self.my_groups().iter().any(|g| g.id == list.owner.id) {
                Some(ListAccess::Write)
            } else {
                None
            }
        } else {
            None
        };
        let capped = base.map(|a| {
            if self.asker_is_ai() {
                a.min(ListAccess::Write)
            } else {
                a
            }
        });
        let only = self
            .read_only
            .iter()
            .any(|(who, l)| who.id == asker.id && *l == list.id);
        capped.map(|a| if only { a.min(ListAccess::Read) } else { a })
    }

    pub(crate) fn sees(&self, list: &TaskList) -> bool {
        self.access_of(list).is_some()
    }

    /// Whether the asking person may see the list of the task at row `i`.
    pub(crate) fn sees_task(&self, i: usize) -> bool {
        self.sees_list_id(self.rows[i].task.list) || self.mutated(Mutation::SeesAll)
    }

    /// Whether the asking person may see the list `id` (a list that is gone shows nobody anything to hide).
    pub(crate) fn sees_list_id(&self, id: ObjectId) -> bool {
        self.list_at(id).is_none_or(|l| self.sees(&self.lists[l]))
    }

    /// Whether the feed may show what happened in the list `id`: a list that is gone (purged, expired) is seen by
    /// nobody, so what the feed holds of it stays hidden from everybody, its owner included.
    pub(crate) fn sees_feed_of(&self, id: ObjectId) -> bool {
        self.list_at(id).is_some_and(|l| self.sees(&self.lists[l]))
    }

    /// The access to the list `id` of the asking person; `NotFound` when they may not see it.
    pub(crate) fn access_to(&self, id: ObjectId) -> TaskResult<ListAccess> {
        self.list_at(id)
            .and_then(|l| self.access_of(&self.lists[l]))
            .ok_or_else(|| Error::NotFound(id).into())
    }

    /// `Forbidden { capability }` unless the asking person has at least `Write` on the list.
    pub(crate) fn need_write(&self, list: ObjectId, capability: &str) -> TaskResult<()> {
        if self.access_to(list)? >= ListAccess::Write {
            Ok(())
        } else {
            Err(forbidden(capability).into())
        }
    }

    /// `Forbidden { capability }` unless the asking person has `Manage` on the list (also one in the trash).
    pub(crate) fn need_manage(&self, list: ObjectId, capability: &str) -> TaskResult<()> {
        let l = self.list_at(list).ok_or(Error::NotFound(list))?;
        match self.access_of(&self.lists[l]) {
            None => Err(Error::NotFound(list).into()),
            Some(ListAccess::Manage) => Ok(()),
            Some(_) => Err(forbidden(capability).into()),
        }
    }

    /// Who may own a list (EIGENTUM): the asking person themselves, or a group they are in; a group needs a
    /// member, another person is `Invalid`, an AI or an AI group is `Forbidden`.
    pub(crate) fn check_owner(&self, owner: &PersonRef, capability: &str) -> TaskResult<()> {
        if (self.groups.is_agent(owner) || self.asker_is_ai()) && !self.mutated(Mutation::AiOwns) {
            return Err(forbidden(capability).into());
        }
        if is_group(owner) {
            let member = self.my_groups().iter().any(|g| g.id == owner.id);
            return if member || self.groups.manages(&self.asker(), owner) {
                Ok(())
            } else {
                Err(forbidden(capability).into())
            };
        }
        if owner.id == self.asker().id || self.is_me(owner) {
            Ok(())
        } else {
            Err(Self::bad("owner", "other-person"))
        }
    }

    /// The claim rule: the asking person may take the task when they are assigned, when they are in an assigned
    /// group (asked through `memberships_of`, never `members_of`), or when nobody is assigned and they may write.
    pub(crate) fn may_claim(&self, t: &Task) -> bool {
        if t.assignees.iter().any(|a| self.is_me(&a.who)) {
            return true;
        }
        let mine = self.my_groups();
        if t.assignees.iter().any(|a| {
            is_group(&a.who)
                && (mine.iter().any(|g| g.id == a.who.id) || self.mutated(Mutation::GroupOpen))
        }) {
            return true;
        }
        t.assignees.is_empty() && self.access_to(t.list).is_ok_and(|a| a >= ListAccess::Write)
    }

    /// Whether the person `who` is one of the groups the asking person is in (for the filter "my groups").
    pub(crate) fn in_my_groups(&self, who: &PersonRef) -> bool {
        is_group(who) && self.my_groups().iter().any(|g| g.id == who.id)
    }
}
