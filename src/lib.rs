use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, PartialEq, Eq)]
pub struct Event {
    pub tags: BTreeMap<String, String>,
}

pub trait Listener {
    fn accept(&mut self, evt: &Event);
}

pub trait Topic<L: Listener> {
    fn subscribe(&mut self, listener: L, filter: Filter);
}

#[derive(Clone)]
pub struct Filter {
    pub tags: BTreeMap<String, BTreeSet<String>>,
}
impl Filter {
    pub const EMPTY: Filter = Filter {
        tags: BTreeMap::new(),
    };
}

#[derive(Default)]
pub struct LinearScan<L: Listener> {
    pub listeners: Vec<(L, Filter)>,
}
impl<L: Listener> Topic<L> for LinearScan<L> {
    fn subscribe(&mut self, listener: L, filter: Filter) {
        self.listeners.push((listener, filter));
    }
}
impl<T: Listener> Listener for LinearScan<T> {
    fn accept(&mut self, evt: &Event) {
        for (listener, filter) in self.listeners.iter_mut() {
            if filter.tags.iter().all(|(tag, values)| {
                evt.tags
                    .get(tag)
                    .map(|v| values.contains(v))
                    .unwrap_or(false)
            }) {
                listener.accept(evt);
            }
        }
    }
}

#[derive(Default)]
pub struct TreeScanner<L: Listener> {
    // The listeners that want to know about all events at this level.
    pipeline: Vec<String>,
    root: TagTree<L>,
}
#[derive(Default)]
struct TagTree<L: Listener> {
    // Listeners that are interested in any event that makes it this far into the pipeline.
    interested: Vec<L>,
    // Listeners that do not care about this particular tag in the pipeline, but want to be filtered on the subsequent ones.
    passthrough: Option<Box<TagTree<L>>>,
    // Otherwise, keep proceeding down the tag pipeline.
    children: BTreeMap<String, TagTree<L>>,
}
impl<L: Listener> TagTree<L> {
    fn new() -> Self {
        Self {
            interested: Vec::new(),
            passthrough: None,
            children: BTreeMap::new(),
        }
    }
}
impl<L: Listener> Topic<L> for TreeScanner<L> {
    fn subscribe(&mut self, listener: L, filter: Filter)
    where
        L: Listener,
    {
        let missing: Vec<String> = filter
            .tags
            .keys()
            .filter(|k| !self.pipeline.contains(k))
            .cloned()
            .collect();
        self.pipeline.extend(missing);

        let mut cur = &mut self.root;
        for key in self.pipeline.iter().take(filter.tags.len()) {
            let Some(vs) = filter.tags.get(key) else {
                if cur.passthrough.is_none() {
                    cur.passthrough = Some(Box::new(TagTree::new()));
                }
                cur = cur.passthrough.as_deref_mut().unwrap();
                continue;
            };
            if vs.len() != 1 {
                todo!("unsupported tag filter: {} = {:?}", key, vs);
            }
            let v = vs.iter().next().unwrap().clone();
            cur = cur.children.entry(v).or_insert_with(TagTree::new);
        }
        cur.interested.push(listener);
    }
}
impl<T: Listener> Listener for TreeScanner<T> {
    fn accept(&mut self, evt: &Event) {
        let mut cur = vec![&mut self.root];
        for key in &self.pipeline {
            let mut next = Vec::new();
            for c in cur {
                for listener in c.interested.iter_mut() {
                    listener.accept(evt);
                }
                if let Some(passthrough) = c.passthrough.as_deref_mut() {
                    next.push(passthrough);
                }
                if let Some(v) = evt.tags.get(key) {
                    if let Some(child) = c.children.get_mut(v) {
                        next.push(child);
                    }
                }
            }
            cur = next;
        }
        for c in cur {
            for listener in c.interested.iter_mut() {
                listener.accept(evt);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    };

    use super::*;

    macro_rules! mkset {
        ($($x:expr),+ $(,)?) => {{
            let mut s = BTreeSet::new();
            $( s.insert($x.to_owned());)+
            s
        }};
    }
    macro_rules! mkevt {
        ($($tag:expr => $value:expr),* $(,)?) => {{
            let mut tags = BTreeMap::new();
            $( tags.insert($tag.to_owned(), $value.to_owned()); )*
            Event { tags }
        }};
    }
    macro_rules! mkfilter {
        ($($tag:expr => [$($value:expr),+]),+ $(,)?) => {{
            let mut tags = BTreeMap::new();
            $( tags.insert($tag.to_owned(), mkset!($($value)*)); )*
            Filter { tags }
        }};
    }

    #[test]
    fn tree_scanner_smoke_test() {
        let mut topic = TreeScanner::default();
        let count = Arc::new(AtomicU32::default());
        topic.subscribe(Counter(count.clone()), Filter::EMPTY);
        topic.subscribe(Counter(count.clone()), mkfilter! { "hello" => ["world"] });

        let evt = mkevt! {"hello" => "world"};
        topic.accept(&evt);
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn tree_scanner_handles_missing_filter_tags() {
        let mut topic = TreeScanner::default();
        let count = Arc::new(AtomicU32::default());
        topic.subscribe(Counter(count.clone()), mkfilter! { "a" => ["foo"] });
        topic.subscribe(Counter(count.clone()), mkfilter! { "b" => ["foo"] });

        let evt = mkevt! {
            "a" => "foo",
            "b" => "foo"
        };
        topic.accept(&evt);
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[derive(Default)]
    struct Counter(Arc<AtomicU32>);
    impl Listener for Counter {
        fn accept(&mut self, _evt: &Event) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
}
