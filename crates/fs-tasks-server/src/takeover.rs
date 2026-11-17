//! The rule for a server that does not answer: may it be ended? A pure decision over facts.
//!
//! Pattern: Policy + Guard — `decide` looks only at made-up or real facts of the process behind the socket and says
//! `TakeOver` (it is provably this program: end it, start a new one) or `Refuse` (never; another program is only reported). The
//! facts are read elsewhere (`procfs`); the killing is done elsewhere (`ending`). So every rule is testable with
//! made-up data: a process of another user, another program, a reused process number, a link to this program.

/// A program file: where it is and which file it really is (a link or a hard link to it is the same file).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    /// The device of the file.
    pub dev: u64,
    /// The file number on the device.
    pub ino: u64,
    /// The real path (links resolved; a trailing ` (deleted)` of a replaced file is cut off).
    pub path: String,
}

impl Image {
    /// The same program file: the same file, or the same real path (the file was replaced by an update, the running one
    /// is the old copy of it). The name alone never counts: a `fs-tasks` somewhere else is another program.
    #[must_use]
    pub fn same_as(&self, other: &Image) -> bool {
        (self.dev == other.dev && self.ino == other.ino) || self.path == other.path
    }
}

/// What is known about the process that holds the socket.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Peer {
    /// Its process number.
    pub pid: i32,
    /// Its user (`u32::MAX` if the facts do not agree).
    pub uid: u32,
    /// When it started, in clock ticks since the boot; `None` if it cannot be read (it is gone, or hidden).
    pub start: Option<u64>,
    /// Its program file; `None` if it cannot be read.
    pub exe: Option<Image>,
}

/// This process.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Own {
    /// The process number.
    pub pid: i32,
    /// The user.
    pub uid: u32,
    /// The own program file.
    pub exe: Image,
}

/// Why a process is not ended without asking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// The process number is the system's (1 or lower) or this process itself.
    Itself,
    /// The process belongs to another user.
    ForeignUser,
    /// Its start time or program file cannot be read: nothing can be verified.
    Unreadable,
    /// The process started at another time than the one seen first: the number was reused by another process.
    PidReused,
    /// The process runs a program file other than this one.
    OtherProgram,
}

impl Why {
    /// The code of the reason (stable, for the message and the log).
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Why::Itself => "itself",
            Why::ForeignUser => "foreign-user",
            Why::Unreadable => "unreadable",
            Why::PidReused => "pid-reused",
            Why::OtherProgram => "other-program",
        }
    }
}

/// What to do with the process behind a socket that does not answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// It is this program of this user, the same process as seen first: end it and start a new server.
    TakeOver,
    /// Never: the person is told, nobody ends it from here.
    Refuse(Why),
}

/// Decide about `peer`. `seen` is the start time seen when the process was found first (or shown to the person): a
/// process that started at another time is not the one that was seen (the number was reused).
#[must_use]
pub fn decide(seen: Option<u64>, peer: &Peer, own: &Own) -> Decision {
    if peer.pid <= 1 || peer.pid == own.pid {
        return Decision::Refuse(Why::Itself);
    }
    if peer.uid != own.uid {
        return Decision::Refuse(Why::ForeignUser);
    }
    match (seen, peer.start) {
        (Some(first), Some(now)) if first == now => {}
        (Some(_), Some(_)) => return Decision::Refuse(Why::PidReused),
        _ => return Decision::Refuse(Why::Unreadable),
    }
    match &peer.exe {
        None => Decision::Refuse(Why::Unreadable),
        Some(exe) if exe.same_as(&own.exe) => Decision::TakeOver,
        Some(_) => Decision::Refuse(Why::OtherProgram),
    }
}
