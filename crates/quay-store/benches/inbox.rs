use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use quay_store::dataset::{DatasetShape, seed};
use quay_store::inbox::{InboxFilter, InboxQuery, run};
use quay_store::schema::{self, SchemaVariant};

fn inbox_queries(criterion: &mut Criterion) {
    let directory = match tempfile::tempdir() {
        Ok(directory) => directory,
        Err(error) => panic!("temporary directory: {error}"),
    };
    let database = directory.path().join("quay.db");
    let mut connection = match schema::create(&database, SchemaVariant::Corrected) {
        Ok(connection) => connection,
        Err(error) => panic!("creating the reference database: {error}"),
    };
    if let Err(error) = seed(
        &mut connection,
        &DatasetShape::reference(),
        SchemaVariant::Corrected,
    ) {
        panic!("seeding the reference dataset: {error}");
    }

    let filter = InboxFilter::default();
    let mut group = criterion.benchmark_group("inbox");
    for query in InboxQuery::for_schema(SchemaVariant::Corrected) {
        group.bench_function(query.id(), |bencher| {
            bencher.iter(|| black_box(run(&connection, query, &filter)));
        });
    }
    group.finish();
}

criterion_group!(benches, inbox_queries);
criterion_main!(benches);
