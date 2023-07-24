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
    // Otherwise, keep proceeding down the tag pipeline.
    children: BTreeMap<String, TagTree<L>>,
}
impl<L: Listener> TagTree<L> {
    fn new() -> Self {
        Self {
            interested: Vec::new(),
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
                panic!("filter is missing required key {}", key)
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
        let mut cur = &mut self.root;
        for listener in cur.interested.iter_mut() {
            listener.accept(evt);
        }
        for key in &self.pipeline {
            let Some(v) = evt.tags.get(key) else { break };
            let Some(next) = cur.children.get_mut(v) else { break };
            for listener in next.interested.iter_mut() {
                listener.accept(evt);
            }
            cur = next;
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

    #[test]
    fn tree_scanner_smoke_test() {
        let mut topic = TreeScanner::default();
        let count = Arc::new(AtomicU32::default());
        topic.subscribe(
            Counter(count.clone()),
            Filter {
                tags: BTreeMap::new(),
            },
        );
        topic.subscribe(
            Counter(count.clone()),
            Filter {
                tags: vec![(
                    "hello".to_owned(),
                    vec!["world".to_owned()].into_iter().collect(),
                )]
                .into_iter()
                .collect(),
            },
        );

        let evt = Event {
            tags: vec![("hello".to_owned(), "world".to_owned())]
                .into_iter()
                .collect(),
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
