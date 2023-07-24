use std::collections::{BTreeMap, HashMap};

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tagsub::{Event, Filter, LinearScan, Listener, Topic, TreeScanner};

fn linear_scan_benchmark(c: &mut Criterion) {
    let mut topic = LinearScan::default();
    let filter = Filter {
        tags: vec![(
            "hello".to_owned(),
            vec!["world".to_owned()].into_iter().collect(),
        )]
        .into_iter()
        .collect(),
    };
    for _ in 0..1_000 {
        topic.subscribe(Counter::default(), filter.clone());
    }

    c.bench_function("all-match", |b| {
        let evt = Event {
            tags: vec![("hello".to_owned(), "world".to_owned())]
                .into_iter()
                .collect(),
        };
        b.iter(|| topic.accept(&evt));
    });
    c.bench_function("none-match", |b| {
        let evt = Event {
            tags: vec![("hello".to_owned(), "garbage".to_owned())]
                .into_iter()
                .collect(),
        };
        b.iter(|| topic.accept(&evt));
    });
}

fn tree_scanner_benchmark(c: &mut Criterion) {
    let mut topic = TreeScanner::default();
    let filter = Filter {
        tags: vec![(
            "hello".to_owned(),
            vec!["world".to_owned()].into_iter().collect(),
        )]
        .into_iter()
        .collect(),
    };
    for _ in 0..1_000 {
        topic.subscribe(Counter::default(), filter.clone());
    }

    c.bench_function("all-match", |b| {
        let evt = Event {
            tags: vec![("hello".to_owned(), "world".to_owned())]
                .into_iter()
                .collect(),
        };
        b.iter(|| topic.accept(&evt));
    });
    c.bench_function("none-match", |b| {
        let evt = Event {
            tags: vec![("hello".to_owned(), "garbage".to_owned())]
                .into_iter()
                .collect(),
        };
        b.iter(|| topic.accept(&evt));
    });
}

#[derive(Default)]
struct Counter(u32);
impl Listener for Counter {
    fn accept(&mut self, evt: &tagsub::Event) {
        self.0 += 1;
    }
}

criterion_group!(benches, linear_scan_benchmark, tree_scanner_benchmark);
criterion_main!(benches);
