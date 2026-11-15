//! Whether a result is out of date, without running every analysis again each time the output is shown.
//!
//! A result is out of date when running its operation again on the current data would give different numbers. That
//! takes a full run of the analysis, so the answer is kept with a key made from what the analysis reads: the command
//! and its settings (with Select, Split and Weight Cases), the number of cases, and each variable the settings name, with
//! its values and its Variable View details. While the key stays the same the kept answer stands, because the same
//! analysis on the same values gives the same numbers. When any of those change, the analysis is run again, once.
//!
//! A variable counts as named when a text setting is its name or contains its name as a word (a formula, a model, a
//! Select Cases condition). Settings that name no variable at all depend on the whole dataset.
//!
//! An analysis that took longer than [`SLOW`] seconds isn't run again just to check: when what it reads has changed,
//! it is marked out of date straight away (a change to the values an analysis uses nearly always changes its numbers),
//! so a cell edit never waits for a random forest. Saved projects keep each result's key (`StoredResult::checked`), so
//! opening one needs no analysis run again when the data are as they were saved.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use prism_engine::project::{Operation, StoredResult};
use prism_engine::{Column, Dataset};
use serde_json::Value;

/// Seconds: an analysis that took longer than this is marked out of date without being run again to check.
pub(crate) const SLOW: f64 = 1.0;

/// The kept answers: for a result, the key it was checked with and whether it was out of date; and how long each
/// operation took the last time it ran.
#[derive(Default)]
pub(crate) struct Recheck {
    known: HashMap<u32, (u64, bool)>,
    /// The key each result was last found up to date at (after an Undo, the data can be back at that key).
    fresh: HashMap<u32, u64>,
    seconds: HashMap<u32, f64>,
}

impl Recheck {
    /// The answer for a result at this key: kept here, or saved with the result as up to date.
    pub(crate) fn get(&self, r: &StoredResult, key: u64) -> Option<bool> {
        if self.fresh.get(&r.id) == Some(&key) {
            return Some(false);
        }
        self.known.get(&r.id).filter(|(k, _)| *k == key).map(|(_, stale)| *stale).or_else(|| r.checked.filter(|(k, _)| *k == key).map(|(_, stale)| stale))
    }

    pub(crate) fn put(&mut self, r: &StoredResult, key: u64, stale: bool) {
        self.known.insert(r.id, (key, stale));
        if !stale {
            self.fresh.insert(r.id, key);
        }
    }

    pub(crate) fn took(&mut self, op: u32, seconds: f64) {
        self.seconds.insert(op, seconds);
    }

    pub(crate) fn slow(&self, op: u32) -> bool {
        self.seconds.get(&op).is_some_and(|s| *s > SLOW)
    }

    pub(crate) fn clear(&mut self) {
        self.known.clear();
        self.fresh.clear();
        self.seconds.clear();
    }
}

/// FNV-1a: a hash that stays the same from one run (and build) of the program to the next, as a key saved in a
/// project must.
struct Fnv(u64);

impl Hasher for Fnv {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 ^= u64::from(*b);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

/// A fast hash of a column's values, in order.
fn column_hash(c: &Column) -> u64 {
    // FNV-1a over 64-bit words, with a final mix: quick on millions of cells, and only ever compared with itself.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |w: u64| {
        h ^= w;
        h = h.wrapping_mul(0x0000_0100_0000_01b3).rotate_left(29);
    };
    match c {
        Column::Numeric(v) => {
            eat(1);
            for x in v {
                eat(x.map_or(0x7ff8_dead_beef_0001, f64::to_bits));
            }
        }
        Column::Text(v) => {
            eat(2);
            for x in v {
                match x {
                    None => eat(0x7ff8_dead_beef_0002),
                    Some(s) => {
                        eat(s.len() as u64);
                        for chunk in s.as_bytes().chunks(8) {
                            let mut w = [0u8; 8];
                            w[..chunk.len()].copy_from_slice(chunk);
                            eat(u64::from_le_bytes(w));
                        }
                    }
                }
            }
        }
    }
    h ^ (h >> 31)
}

/// Every text in the settings, and the words in each.
fn texts<'a>(v: &'a Value, out: &mut Vec<&'a str>) {
    match v {
        Value::String(s) => out.push(s),
        Value::Array(a) => a.iter().for_each(|x| texts(x, out)),
        Value::Object(o) => o.values().for_each(|x| texts(x, out)),
        _ => {}
    }
}

/// The variables an operation's settings name, by position in the dataset. None: it names none, so it reads them all.
fn named(ds: &Dataset, args: &Value) -> Option<Vec<usize>> {
    let mut all = Vec::new();
    texts(args, &mut all);
    let mut words: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for t in &all {
        words.insert(t);
        words.extend(t.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|w| !w.is_empty()));
    }
    let found: Vec<usize> = ds.variables.iter().enumerate().filter(|(_, v)| words.contains(v.name.as_str())).map(|(i, _)| i).collect();
    (!found.is_empty()).then_some(found)
}

/// Column hashes, worked out at most once per check of the output.
pub(crate) struct Hashes<'a> {
    ds: &'a Dataset,
    done: Vec<Option<u64>>,
}

impl<'a> Hashes<'a> {
    pub(crate) fn new(ds: &'a Dataset) -> Self {
        Hashes { ds, done: vec![None; ds.columns.len()] }
    }

    fn column(&mut self, i: usize) -> u64 {
        *self.done[i].get_or_insert_with(|| column_hash(&self.ds.columns[i]))
    }

    /// The key for one operation on the current data.
    pub(crate) fn key(&mut self, op: &Operation) -> u64 {
        let mut h = Fnv(0xcbf2_9ce4_8422_2325);
        env!("CARGO_PKG_VERSION").hash(&mut h);
        op.seq.hash(&mut h);
        op.command.hash(&mut h);
        op.args.to_string().hash(&mut h);
        self.ds.n_rows().hash(&mut h);
        let vars = named(self.ds, &op.args).unwrap_or_else(|| (0..self.ds.variables.len()).collect());
        for i in vars {
            serde_json::to_string(&self.ds.variables[i]).unwrap_or_default().hash(&mut h);
            self.column(i).hash(&mut h);
        }
        h.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ds() -> Dataset {
        let mut d = Dataset::new("t");
        d.add_variable("y", Column::Numeric(vec![Some(1.0), Some(2.0), None])).unwrap();
        d.add_variable("g", Column::Numeric(vec![Some(1.0), Some(1.0), Some(2.0)])).unwrap();
        d.add_variable("note", Column::Text(vec![Some("a".into()), None, Some("b".into())])).unwrap();
        d
    }

    fn op(args: Value) -> Operation {
        Operation { seq: 1, command: "descriptives".into(), args }
    }

    #[test]
    fn settings_name_variables_directly_and_inside_formulas() {
        let d = ds();
        assert_eq!(named(&d, &json!({ "variables": ["y"] })), Some(vec![0]));
        assert_eq!(named(&d, &json!({ "model": "y ~ g" })), Some(vec![0, 1]));
        assert_eq!(named(&d, &json!({ "data": { "filter": "g == 2" } })), Some(vec![1]));
        assert_eq!(named(&d, &json!({ "kind": "pca" })), None, "naming none means reading all");
    }

    #[test]
    fn the_key_changes_with_what_the_analysis_reads_and_only_that() {
        let mut d = ds();
        let o = op(json!({ "variables": ["y"] }));
        let k = Hashes::new(&d).key(&o);
        assert_eq!(Hashes::new(&d).key(&o), k);
        // Another variable's values: same key.
        if let Column::Numeric(v) = &mut d.columns[1] {
            v[0] = Some(9.0);
        }
        assert_eq!(Hashes::new(&d).key(&o), k);
        // Its own values: a new key.
        if let Column::Numeric(v) = &mut d.columns[0] {
            v[2] = Some(3.0);
        }
        let k2 = Hashes::new(&d).key(&o);
        assert_ne!(k2, k);
        // Its Variable View details (a missing-value code): a new key.
        d.variables[0].missing_values.push(3.0);
        assert_ne!(Hashes::new(&d).key(&o), k2);
    }
}
