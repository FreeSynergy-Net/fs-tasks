//! The two shapes of one call of the provider: a unit of work (write) and a read.
//!
//! Pattern: Template (each trait method is one short body inside one of these; the deadline run comes first).

/// One unit of work: the deadline run, the time of the write, `begin`, the body, `finish`.
macro_rules! write {
    ($s:expr, $w:ident, $c:ident, $now:ident, $body:expr) => {{
        $s.sweep().await?;
        let $w = $s.who();
        let $now = $s.clock.tick();
        let $c = $s.store.begin().await?;
        let r = $body.await;
        $crate::store::finish($c, r).await
    }};
}

/// One read after the deadline run.
macro_rules! read {
    ($s:expr, $w:ident, $c:ident, $body:expr) => {{
        $s.sweep().await?;
        let $w = $s.who();
        let $c = $s.store.read().await;
        $body.await
    }};
}

pub(crate) use read;
pub(crate) use write;
