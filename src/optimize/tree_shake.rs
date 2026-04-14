#![allow(dead_code)]
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct TreeShakeReport {
    pub kept_symbols: Vec<String>,
    pub removed_symbols: Vec<String>,
}

pub fn tree_shake(all_symbols: &[String], entry_symbols: &[String]) -> TreeShakeReport {
    let entry_set: HashSet<&str> = entry_symbols.iter().map(|s| s.as_str()).collect();

    let mut kept = Vec::new();
    let mut removed = Vec::new();

    for symbol in all_symbols {
        if entry_set.contains(symbol.as_str()) {
            kept.push(symbol.clone());
        } else {
            removed.push(symbol.clone());
        }
    }

    TreeShakeReport {
        kept_symbols: kept,
        removed_symbols: removed,
    }
}
