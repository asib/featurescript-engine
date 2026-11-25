//! Kept states: what a context's geometry was after each operation, so that a design built again with other values
//! starts from where the builds first differ.
//!
//! A state carries a [`Stamp`]: a hash of every call that has changed it, with what each call was given. Two states
//! with one stamp were made the same way, so they are the same. An operation about to run on a state works out the
//! stamp it will leave, and if a state with that stamp is kept, takes it instead of running.
//!
//! OCCT's algorithms can alter the shapes they are given (a sweep adds curves to its profile's edges, a boolean can
//! raise a vertex's tolerance), and a kept state shares its shapes with the states made from it. So each kept state
//! has a print of its shapes ([`occt::Shape::print`]), taken when it was made, and is only taken up again while its
//! shapes still give that print. One that was altered is dropped, and its operation runs again.
//!
//! Nothing is kept unless a host asks ([`keep`]): a page that rebuilds one design does, the fixtures don't.

use crate::model::GeomState;
use fs_interp::Value;
use fs_interp::value::Kind;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;

/// What made a state: a hash of the calls that changed it and their arguments, 128 bits of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Stamp(u64, u64);

impl Stamp {
    /// A new context's.
    pub const START: Stamp = Stamp(0, 0);
}

/// What a call was given, fed in to make the stamp it leaves.
pub struct Feed {
    a: DefaultHasher,
    b: DefaultHasher,
    /// Whether everything fed could be told apart from anything else: a function or a builtin object can't.
    whole: bool,
}

impl Feed {
    fn after(stamp: Stamp, name: &str) -> Feed {
        let mut feed = Feed { a: DefaultHasher::new(), b: DefaultHasher::new(), whole: true };
        // The two halves differ from here on.
        feed.b.write_u64(0x9e37_79b9_7f4a_7c15);
        feed.u64(stamp.0);
        feed.u64(stamp.1);
        feed.text(name);
        feed
    }

    pub fn u64(&mut self, v: u64) {
        self.a.write_u64(v);
        self.b.write_u64(v);
    }

    pub fn text(&mut self, s: &str) {
        self.u64(s.len() as u64);
        self.a.write(s.as_bytes());
        self.b.write(s.as_bytes());
    }

    pub fn value(&mut self, v: &Value) {
        match v.tag {
            Some(tag) => {
                self.u64(1);
                self.u64(u64::from(tag));
            }
            None => self.u64(0),
        }
        match &v.kind {
            Kind::Undefined => self.u64(0),
            Kind::Bool(b) => {
                self.u64(1);
                self.u64(u64::from(*b));
            }
            Kind::Number(n) => {
                self.u64(2);
                self.u64(n.to_bits());
            }
            Kind::String(s) => {
                self.u64(3);
                self.text(s);
            }
            Kind::Array(items) => {
                self.u64(4);
                self.u64(items.len() as u64);
                for item in items.iter() {
                    self.value(item);
                }
            }
            Kind::Map(map) => {
                self.u64(5);
                self.u64(map.len() as u64);
                for (key, value) in map.iter() {
                    self.value(key);
                    self.value(value);
                }
            }
            Kind::Box(_) | Kind::Builtin(_) | Kind::Function(_) => self.whole = false,
        }
    }

    /// The call reads something that can't be fed, such as a file: what it leaves can't be told from its arguments.
    pub fn untold(&mut self) {
        self.whole = false;
    }

    fn stamp(self) -> Option<Stamp> {
        self.whole.then(|| Stamp(self.a.finish(), self.b.finish()))
    }
}

struct Kept {
    state: GeomState,
    result: Value,
    print: u64,
    /// The last build that made or took it.
    build: u64,
}

/// How many operations a build ran and how many it took from what was kept.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub ran: usize,
    pub kept: usize,
}

#[derive(Default)]
struct Store {
    /// How many builds a state is kept for after the last that used it; 0 keeps nothing.
    builds: u64,
    build: u64,
    states: HashMap<Stamp, Kept>,
    counts: Counts,
}

thread_local! {
    // One engine runs per thread (see fs_interp::types), and making one empties this.
    static STORE: RefCell<Store> = RefCell::new(Store::default());
}

/// Keeps each operation's state for `builds` builds after the last that used it, or with 0 keeps nothing, which is
/// how an engine starts.
pub fn keep(builds: usize) {
    STORE.with(|s| {
        let mut store = s.borrow_mut();
        store.builds = builds as u64;
        if builds == 0 {
            store.states.clear();
        }
    });
}

/// Forgets every kept state: for a new engine, whose type tags may number differently.
pub(crate) fn reset() {
    STORE.with(|s| *s.borrow_mut() = Store::default());
}

/// Starts a build: states no build has used for a while are dropped, and the counts start again.
pub fn begin() {
    STORE.with(|s| {
        let mut store = s.borrow_mut();
        store.build += 1;
        store.counts = Counts::default();
        let (build, builds) = (store.build, store.builds);
        store.states.retain(|_, kept| kept.build + builds >= build);
    });
}

/// What the build since [`begin`] ran and took.
pub fn counts() -> Counts {
    STORE.with(|s| s.borrow().counts)
}

fn keeping() -> bool {
    STORE.with(|s| s.borrow().builds > 0)
}

/// A print of every body's shape, to tell whether anything has altered them since.
fn print(state: &GeomState) -> u64 {
    let mut hasher = DefaultHasher::new();
    for body in &state.model.bodies {
        hasher.write_u64(body.shape.print());
    }
    hasher.finish()
}

/// The stamp a call leaves on `state`: none if nothing is kept, if the state's own is unknown, or if what the call
/// was given can't be told apart from anything else.
fn next(state: &GeomState, name: &str, feed: impl FnOnce(&mut Feed)) -> Option<Stamp> {
    if !keeping() {
        return None;
    }
    let mut fed = Feed::after(state.stamp?, name);
    feed(&mut fed);
    fed.stamp()
}

/// Marks `state` as changed by a call given `args`, when the call changes it in place. A call that never changes
/// the state must not come here: its stamp would then differ from another build's that didn't make the call.
pub(crate) fn changed(state: &mut GeomState, name: &str, args: &[Value], sheet_metal: bool) {
    state.stamp = next(state, name, |feed| {
        feed.u64(u64::from(sheet_metal));
        for arg in args {
            feed.value(arg);
        }
    });
}

/// The stamp of a state a feature's abort went back to: the old state's, and the transient ids the feature used
/// up, which aren't given out again.
pub(crate) fn aborted(old: Option<Stamp>, next_tid: u64) -> Option<Stamp> {
    let mut fed = Feed::after(old?, "abortFeature");
    fed.u64(next_tid);
    fed.stamp()
}

/// Runs an operation on `state`, or takes the state it left the last time it ran on this state with what `feed`
/// feeds: everything it was given that its result depends on.
pub(crate) fn step<E>(
    state: &mut GeomState,
    name: &str,
    feed: impl FnOnce(&mut Feed),
    run: impl FnOnce(&mut GeomState) -> Result<Value, E>,
) -> Result<Value, E> {
    let stamp = next(state, name, feed);
    if let Some(stamp) = stamp
        && let Some((kept, result)) = take(stamp)
    {
        *state = kept;
        return Ok(result);
    }
    let result = run(state);
    // Whether it failed or not: what it did before failing is the same each time.
    state.stamp = stamp;
    STORE.with(|s| s.borrow_mut().counts.ran += 1);
    if let (Some(stamp), Ok(value)) = (stamp, &result) {
        let kept = Kept { state: state.clone(), result: value.clone(), print: print(state), build: 0 };
        STORE.with(|s| {
            let mut store = s.borrow_mut();
            let build = store.build;
            store.states.insert(stamp, Kept { build, ..kept });
        });
    }
    result
}

/// The state kept under `stamp` and its operation's result, if its shapes are still as they were.
fn take(stamp: Stamp) -> Option<(GeomState, Value)> {
    STORE.with(|s| {
        let mut store = s.borrow_mut();
        let build = store.build;
        let kept = store.states.get_mut(&stamp)?;
        if print(&kept.state) != kept.print {
            store.states.remove(&stamp);
            return None;
        }
        kept.build = build;
        let taken = (kept.state.clone(), kept.result.clone());
        store.counts.kept += 1;
        Some(taken)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamp(values: &[Value]) -> Option<Stamp> {
        let mut feed = Feed::after(Stamp::START, "op");
        for value in values {
            feed.value(value);
        }
        feed.stamp()
    }

    #[test]
    fn a_stamp_tells_what_it_was_fed() {
        let n = Value::number;
        let s = |text: &str| Value::string(text.to_string());
        assert_eq!(stamp(&[n(1.0), s("a")]), stamp(&[n(1.0), s("a")]));
        // The last bit of a number, where one string ends and the next starts, what is inside an array and what
        // follows it, a value's type tag, its kind, and whether it's there at all.
        assert_ne!(stamp(&[n(1.0)]), stamp(&[n(1.000_000_000_000_000_2)]));
        assert_ne!(stamp(&[s("ab"), s("c")]), stamp(&[s("a"), s("bc")]));
        assert_ne!(stamp(&[Value::array(vec![n(1.0)]), n(2.0)]), stamp(&[Value::array(vec![n(1.0), n(2.0)])]));
        assert_ne!(stamp(&[n(1.0)]), stamp(&[n(1.0).with_tag(Some(3))]));
        assert_ne!(stamp(&[Value::bool(true)]), stamp(&[n(1.0)]));
        assert_ne!(stamp(&[Value::UNDEFINED]), stamp(&[]));
        // And the call and the state it was made on.
        assert_ne!(Feed::after(Stamp::START, "opExtrude").stamp(), Feed::after(Stamp::START, "opRevolve").stamp());
        assert_ne!(Feed::after(Stamp(1, 0), "op").stamp(), Feed::after(Stamp::START, "op").stamp());
    }

    #[test]
    fn a_call_that_reads_what_cannot_be_fed_leaves_no_stamp() {
        let mut feed = Feed::after(Stamp::START, "opImportForeign");
        feed.value(&Value::number(1.0));
        feed.untold();
        assert_eq!(feed.stamp(), None);
        assert_eq!(aborted(None, 7), None);
        assert_ne!(aborted(Some(Stamp::START), 7), aborted(Some(Stamp::START), 8));
    }
}
