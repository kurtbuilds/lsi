use criterion::{black_box, criterion_group, criterion_main, Criterion};
use lsi::GLOBAL_TABLE;
use lsi::{Istr, InternedData};
use string_interner::StringInterner;
use ustr::ustr;

static DATA_64X10K: &str = include_str!("../data/64x10k.txt");

fn data_64x10k() -> Vec<&'static str> {
    DATA_64X10K.split('\n').collect::<Vec<_>>()
}

fn bench_intern_strings(c: &mut Criterion) {
    let data = data_64x10k();
    c.bench_function("lsi::Istr::new", |b| b.iter(|| {
        for &s in &data {
            Istr::new(s);
        }
    }));
    println!("lsi::Istr::new: {} interned", GLOBAL_TABLE.len());
    // c.bench_function("string_interner::StringInterner::get_or_intern", |b| b.iter(|| {
    //     let mut interner: StringInterner = StringInterner::new();
    //     for &s in &data {
    //         interner.get_or_intern(black_box(s));
    //     }
    // }));
    c.bench_function("ustr::ustr", |b| b.iter(|| {
        for &s in &data {
            ustr(s);
        }
    }));
}

criterion_group!(create_strings, bench_intern_strings);
criterion_main!(create_strings);