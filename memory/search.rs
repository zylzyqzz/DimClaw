use crate::core::traits::memory::MemoryItem;

pub fn hybrid_merge(mut vector_hits: Vec<MemoryItem>, mut keyword_hits: Vec<MemoryItem>, limit: usize) -> Vec<MemoryItem> {
    for v in &mut vector_hits {
        v.score *= 0.7;
    }
    for k in &mut keyword_hits {
        k.score *= 0.3;
    }

    let mut merged = Vec::new();
    merged.extend(vector_hits);
    merged.extend(keyword_hits);

    merged.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    let mut dedup = std::collections::HashSet::new();
    let mut out = Vec::new();
    for item in merged {
        if dedup.insert(item.id.clone()) {
            out.push(item);
        }
        if out.len() >= limit {
            break;
        }
    }
    out
}
