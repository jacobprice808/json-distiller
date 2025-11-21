// src/core.rs - HIGHLY OPTIMIZED VERSION

use crate::error::{DistillError, Result};
use ahash::AHasher;
use rustc_hash::{FxHashMap, FxHashSet};
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use std::hash::{Hash, Hasher};
use md5::{Md5, Digest};

// Optimized: Use Vec instead of SmallVec for recursive types (avoids cycle)
// Pre-allocate with capacity to minimize allocations
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum DeepStructureKey {
    Primitive(&'static str),  // Zero-allocation for common type names
    Dict(Vec<(String, DeepStructureKey)>),  // Sorted vec
    List(Vec<DeepStructureKey>),            // Sorted vec of unique structures
    EmptyList,
}

// Implement Ord to match Python's tuple comparison behavior
// Python sorts by repr() when types can't be compared directly
impl Ord for DeepStructureKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.to_python_repr().cmp(&other.to_python_repr())
    }
}

impl PartialOrd for DeepStructureKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl DeepStructureKey {
    /// Convert to Python repr() format for MD5 hashing
    /// This must match Python's repr() exactly for hash compatibility
    fn to_python_repr(&self) -> String {
        match self {
            DeepStructureKey::Primitive(type_name) => {
                format!("('primitive', '{}')", type_name)
            }
            DeepStructureKey::EmptyList => {
                "('list', 'empty')".to_string()
            }
            DeepStructureKey::List(elements) => {
                // Elements are already sorted when DeepStructureKey::List is created
                // Don't sort again here - that would use string comparison instead of tuple comparison
                let element_reprs: Vec<String> = elements
                    .iter()
                    .map(|e| e.to_python_repr())
                    .collect();
                // Python requires trailing comma for single-element tuples: (x,) not (x)
                if element_reprs.len() == 1 {
                    format!("('list', ({},))", element_reprs[0])
                } else {
                    format!("('list', ({}))", element_reprs.join(", "))
                }
            }
            DeepStructureKey::Dict(items) => {
                let items_repr: Vec<String> = items
                    .iter()
                    .map(|(k, v)| format!("('{}', {})", k, v.to_python_repr()))
                    .collect();
                // Python requires trailing comma for single-element tuples: (x,) not (x)
                if items_repr.len() == 1 {
                    format!("('dict', ({},))", items_repr[0])
                } else {
                    format!("('dict', ({}))", items_repr.join(", "))
                }
            }
        }
    }
}

// Use IndexMap for insertion-order preservation (matches Python dict behavior)
type StructureCache = FxHashMap<u64, DeepStructureKey>;  // Order doesn't matter for this cache
// MemoCache key format matches Python: (is_signature, hash, depth, example_index)
type MemoCache = IndexMap<(bool, String, usize, usize), Value>;  // Preserve order

/// Hash a JSON Value directly without serialization (10-50x faster than serde+md5)
#[inline]
fn hash_json_value(value: &Value, strict_typing: bool) -> u64 {
    let mut hasher = AHasher::default();
    strict_typing.hash(&mut hasher);
    hash_value_recursive(value, &mut hasher);
    hasher.finish()
}

/// Recursively hash a JSON Value (inlined for performance)
#[inline]
fn hash_value_recursive(value: &Value, hasher: &mut AHasher) {
    match value {
        Value::Null => hasher.write_u8(0),
        Value::Bool(b) => {
            hasher.write_u8(1);
            b.hash(hasher);
        }
        Value::Number(n) => {
            hasher.write_u8(2);
            n.to_string().hash(hasher);
        }
        Value::String(s) => {
            hasher.write_u8(3);
            s.hash(hasher);
        }
        Value::Array(arr) => {
            hasher.write_u8(4);
            hasher.write_usize(arr.len());
            for item in arr {
                hash_value_recursive(item, hasher);
            }
        }
        Value::Object(obj) => {
            hasher.write_u8(5);
            hasher.write_usize(obj.len());
            // Collect and sort keys for deterministic hashing
            let mut keys: Vec<&String> = obj.keys().collect();
            keys.sort_unstable();
            for key in keys {
                key.hash(hasher);
                hash_value_recursive(&obj[key], hasher);
            }
        }
    }
}

#[inline]
fn get_deep_structure_key_cached(
    item: &Value,
    strict_typing: bool,
    cache: &mut StructureCache,
) -> Result<DeepStructureKey> {
    // Optimization: Skip caching for primitives (faster to recompute than cache lookup)
    if !matches!(item, Value::Object(_) | Value::Array(_)) {
        return get_deep_structure_key_impl(item, strict_typing, cache);
    }

    // Optimization: Hash directly without serialization (10-50x faster)
    let cache_key = hash_json_value(item, strict_typing);

    // Check cache first (FxHashMap is 2x faster than DashMap for single-threaded)
    if let Some(cached) = cache.get(&cache_key) {
        return Ok(cached.clone());
    }

    // Compute and cache
    let result = get_deep_structure_key_impl(item, strict_typing, cache)?;
    cache.insert(cache_key, result.clone());
    Ok(result)
}

#[inline]
fn get_deep_structure_key_impl(
    item: &Value,
    strict_typing: bool,
    cache: &mut StructureCache,
) -> Result<DeepStructureKey> {
    match item {
        Value::Object(map) => {
            // DON'T sort! Python preserves insertion order for dicts (3.7+)
            // Sorting would produce different structure hashes
            let mut pairs: Vec<(String, DeepStructureKey)> = Vec::with_capacity(map.len());
            for (k, v) in map {
                pairs.push((k.clone(), get_deep_structure_key_cached(v, strict_typing, cache)?));
            }
            // Note: serde_json::Map preserves insertion order, so we maintain it
            Ok(DeepStructureKey::Dict(pairs))
        }
        Value::Array(list) => {
            if list.is_empty() {
                Ok(DeepStructureKey::EmptyList)
            } else {
                // Optimization: Use FxHashSet (2x faster than std HashSet)
                let mut element_keys = FxHashSet::with_capacity_and_hasher(
                    list.len().min(16),  // Cap initial capacity
                    Default::default()
                );
                for elem in list {
                    element_keys.insert(get_deep_structure_key_cached(elem, strict_typing, cache)?);
                }
                // Convert to sorted vec
                let mut sorted_keys: Vec<DeepStructureKey> = element_keys.into_iter().collect();
                sorted_keys.sort_unstable();
                Ok(DeepStructureKey::List(sorted_keys))
            }
        }
        Value::Null => {
            // Optimization: Use &'static str (zero allocation)
            if strict_typing {
                Ok(DeepStructureKey::Primitive("NoneType"))
            } else {
                Ok(DeepStructureKey::Primitive("value"))
            }
        }
        _ => {
            if strict_typing {
                // Optimization: Use &'static str for common types
                let type_key = match item {
                    Value::Bool(_) => DeepStructureKey::Primitive("bool"),
                    Value::String(_) => DeepStructureKey::Primitive("str"),
                    Value::Number(n) => {
                        if n.is_f64() {
                            DeepStructureKey::Primitive("float")
                        } else {
                            DeepStructureKey::Primitive("int")
                        }
                    }
                    _ => return Err(DistillError::Internal("Unexpected type in primitive match arm".to_string())),
                };
                Ok(type_key)
            } else {
                Ok(DeepStructureKey::Primitive("value"))
            }
        }
    }
}

/// Pass 1: Collect minimum depth for each structure hash
/// Used when position_dependent=false to show examples only at shallowest occurrence
fn collect_structure_depths(
    container: &Value,
    depth: usize,
    strict_typing: bool,
    cache: &mut StructureCache,
    accumulator: &mut FxHashMap<String, usize>,
) -> Result<()> {
    match container {
        Value::Object(map) => {
            // Recurse on all values
            for v in map.values() {
                collect_structure_depths(v, depth + 1, strict_typing, cache, accumulator)?;
            }
            Ok(())
        }
        Value::Array(list) => {
            if list.is_empty() {
                return Ok(());
            }

            // Skip lists of primitives (same logic as distill_recursive)
            let is_list_of_primitives = list.iter().all(|item| {
                !matches!(item, Value::Object(_) | Value::Array(_))
            });

            if is_list_of_primitives {
                return Ok(());
            }

            // Compute hashes for all items in this list
            for item in list {
                let deep_key = get_deep_structure_key_cached(item, strict_typing, cache)?;
                let current_hash = generate_hash(&deep_key)?;

                // Track minimum depth for this hash
                accumulator
                    .entry(current_hash)
                    .and_modify(|min_depth| *min_depth = (*min_depth).min(depth))
                    .or_insert(depth);

                // Recurse into the item to find nested structures
                collect_structure_depths(item, depth + 1, strict_typing, cache, accumulator)?;
            }

            Ok(())
        }
        _ => Ok(()),
    }
}

#[inline]
fn generate_hash(key: &DeepStructureKey) -> Result<String> {
    // Use MD5 to match Python's hash generation exactly
    // Python: hashlib.md5(repr(key).encode('utf-8')).hexdigest()[:8]
    let repr_string = key.to_python_repr();
    let mut hasher = Md5::new();
    hasher.update(repr_string.as_bytes());
    let result = hasher.finalize();
    // Take first 8 hex characters (4 bytes)
    Ok(format!("{:02x}{:02x}{:02x}{:02x}",
        result[0], result[1], result[2], result[3]))
}

#[inline]
fn find_adjacent_patterns_python_style(hash_sequence: &[String]) -> Vec<Value> {
    if hash_sequence.is_empty() {
        return Vec::new();
    }

    // Optimization: Pre-allocate with estimated capacity
    let mut output_sequence: Vec<Value> = Vec::with_capacity(hash_sequence.len() / 4);
    let mut i = 0;
    let n = hash_sequence.len();

    while i < n {
        let current_hash = &hash_sequence[i];
        let mut run_len = 1;

        // Count consecutive identical hashes
        while i + run_len < n && hash_sequence[i + run_len] == *current_hash {
            run_len += 1;
        }

        if run_len >= 2 {
            output_sequence.push(json!({
                "pattern": [current_hash],
                "repeat": run_len
            }));
            i += run_len;
            continue;
        }

        // Check for alternating pattern (AB AB AB...)
        // Matches Python: requires pattern to appear at i+2:i+4
        if i + 3 < n {
            if hash_sequence[i + 2] == hash_sequence[i] &&
               hash_sequence[i + 3] == hash_sequence[i + 1] {
                let pattern_a = &hash_sequence[i];
                let pattern_b = &hash_sequence[i + 1];

                // Count how many complete pairs we have
                // Start at 1 since we've confirmed pattern appears twice (at i:i+2 and i+2:i+4)
                let mut run_len_pairs = 1;
                while i + (run_len_pairs + 1) * 2 <= n &&
                      hash_sequence.get(i + run_len_pairs * 2) == Some(pattern_a) &&
                      hash_sequence.get(i + run_len_pairs * 2 + 1) == Some(pattern_b) {
                    run_len_pairs += 1;
                }

                output_sequence.push(json!({
                    "pattern": [pattern_a, pattern_b],
                    "repeat": run_len_pairs
                }));
                i += run_len_pairs * 2;
                continue;
            }
        }

        output_sequence.push(Value::String(current_hash.clone()));
        i += 1;
    }

    output_sequence
}

#[inline]
fn format_pattern_to_string_python_style(pattern_output: &[Value]) -> String {
    // Optimization: Pre-allocate string capacity
    let mut parts = Vec::with_capacity(pattern_output.len());

    for element_val in pattern_output {
        if let Some(hash_str) = element_val.as_str() {
            parts.push(hash_str.to_string());
        } else if let Some(summary_obj) = element_val.as_object() {
            if let (Some(Value::Array(pattern_arr)), Some(Value::Number(repeat_num))) =
                (summary_obj.get("pattern"), summary_obj.get("repeat"))
            {
                if let Some(repeat_count) = repeat_num.as_u64() {
                    let pattern_hashes: Vec<&str> = pattern_arr.iter()
                        .filter_map(|v| v.as_str())
                        .collect();
                    let pattern_str = pattern_hashes.join(" ");

                    if pattern_hashes.len() > 1 {
                        parts.push(format!("[{}](x{})", pattern_str, repeat_count));
                    } else {
                        parts.push(format!("{}(x{})", pattern_str, repeat_count));
                    }
                }
            }
        }
    }
    parts.join(" ")
}

fn distill_recursive(
    original_container: &Value,
    strict_typing: bool,
    _repeat_threshold: usize,
    memoized_examples: &mut MemoCache,
    structure_cache: &mut StructureCache,
    depth: usize,
    min_depths: &FxHashMap<String, usize>,
    position_dependent: bool,
    global_examples_shown: &mut FxHashMap<String, usize>, // Matches Python's global_examples_tracker
) -> Result<Value> {
    match original_container {
        Value::Object(map) => {
            // Optimization: Pre-allocate with exact capacity
            let mut new_map = Map::with_capacity(map.len());
            for (k, v_original) in map {
                new_map.insert(
                    k.clone(),
                    distill_recursive(v_original, strict_typing, _repeat_threshold, memoized_examples, structure_cache, depth + 1, min_depths, position_dependent, global_examples_shown)?
                );
            }
            Ok(Value::Object(new_map))
        }
        Value::Array(original_list) => {
            if original_list.is_empty() {
                return Ok(Value::Array(vec![]));
            }

            // CRITICAL FIX: Handle lists of primitives specially (matches Python behavior)
            // For lists of primitives, summarization is problematic due to generic structure keys.
            // Instead, return unique sorted values from this specific list.
            let is_list_of_primitives = original_list.iter().all(|item| {
                !matches!(item, Value::Object(_) | Value::Array(_))
            });

            if is_list_of_primitives {
                // Collect unique values
                let mut unique_values: FxHashSet<Value> = FxHashSet::default();
                for item in original_list {
                    unique_values.insert(item.clone());
                }

                // Sort values (null at end)
                let mut sorted_values: Vec<Value> = unique_values.into_iter()
                    .filter(|v| !v.is_null())
                    .collect();

                // Sort using JSON string representation for consistent ordering
                sorted_values.sort_by(|a, b| {
                    match (a, b) {
                        (Value::Number(n1), Value::Number(n2)) => {
                            n1.to_string().cmp(&n2.to_string())
                        }
                        (Value::String(s1), Value::String(s2)) => s1.cmp(s2),
                        (Value::Bool(b1), Value::Bool(b2)) => b1.cmp(b2),
                        _ => serde_json::to_string(a).unwrap_or_default()
                            .cmp(&serde_json::to_string(b).unwrap_or_default())
                    }
                });

                // Add nulls at end
                let null_count = original_list.iter().filter(|v| v.is_null()).count();
                for _ in 0..null_count {
                    sorted_values.push(Value::Null);
                }

                return Ok(Value::Array(sorted_values));
            }

            // Normal distillation for lists of objects/arrays
            // Use IndexMap to preserve insertion order (matches Python dict behavior)
            let mut hash_sequence: Vec<String> = Vec::with_capacity(original_list.len());
            let mut first_occurrence_indices: IndexMap<String, usize> = IndexMap::with_capacity(original_list.len() / 10);
            // Create LOCAL first_examples for this array (like Python's first_items_to_distill)
            // This ensures each depth level gets its own examples, not global ones
            let mut local_first_examples: IndexMap<String, Value> = IndexMap::new();

            // First pass: compute hashes and track first occurrences
            for (i, item) in original_list.iter().enumerate() {
                let deep_key = get_deep_structure_key_cached(item, strict_typing, structure_cache)?;
                let current_hash = generate_hash(&deep_key)?;
                hash_sequence.push(current_hash.clone());

                first_occurrence_indices.entry(current_hash.clone()).or_insert_with(|| {
                    local_first_examples.entry(current_hash.clone()).or_insert_with(|| item.clone());
                    i
                });
            }

            // Second pass: distill first examples (IndexMap preserves insertion order)
            let mut distilled_first_examples: IndexMap<String, Value> = IndexMap::with_capacity(
                first_occurrence_indices.len()
            );

            for hash in first_occurrence_indices.keys() {
                // Match Python's memo_key format EXACTLY: (is_signature=false, hash, depth, example_index=0)
                // example_index is 0 since we only show one example per hash (MAX_EXAMPLES_PER_STRUCTURE=1)
                let memo_key = (false, hash.clone(), depth, 0);

                if let Some(cached_value) = memoized_examples.get(&memo_key) {
                    distilled_first_examples.insert(hash.clone(), cached_value.clone());
                } else {
                    // Get the original item from LOCAL cache (matches Python's per-depth behavior)
                    let original_item = local_first_examples.get(hash)
                        .ok_or_else(|| DistillError::Internal(format!("Original first example missing for hash {}", hash)))?
                        .clone();

                    let distilled_value = distill_recursive(
                        &original_item,
                        strict_typing,
                        _repeat_threshold,
                        memoized_examples,
                        structure_cache,
                        depth + 1,
                        min_depths,
                        position_dependent,
                        global_examples_shown
                    )?;
                    memoized_examples.insert(memo_key, distilled_value.clone());
                    distilled_first_examples.insert(hash.clone(), distilled_value);
                }
            }

            // Third pass: build output with summaries
            let mut new_list: Vec<Value> = Vec::with_capacity(original_list.len() / 4);
            let mut summarized_hashes_block: Vec<String> = Vec::new();
            let mut hashes_referenced_in_summaries: FxHashSet<String> = FxHashSet::default();
            let mut first_item_positions: FxHashMap<String, usize> = FxHashMap::default();

            let process_summary_block = |
                summarized_hashes: &mut Vec<String>,
                referenced_hashes: &mut FxHashSet<String>,
                output_list: &mut Vec<Value>
            | {
                if !summarized_hashes.is_empty() {
                    let pattern_output = find_adjacent_patterns_python_style(summarized_hashes);
                    let pattern_string = format_pattern_to_string_python_style(&pattern_output);

                    // Track which hashes are referenced in patterns
                    for element_val in &pattern_output {
                        if let Some(hash_str) = element_val.as_str() {
                            referenced_hashes.insert(hash_str.to_string());
                        } else if let Some(summary_obj) = element_val.as_object() {
                            if let Some(Value::Array(pattern_arr)) = summary_obj.get("pattern") {
                                for hash_val in pattern_arr {
                                    if let Some(h) = hash_val.as_str() {
                                        referenced_hashes.insert(h.to_string());
                                    }
                                }
                            }
                        }
                    }

                    let summary_obj = json!({
                        "item_count": summarized_hashes.len(),
                        "summarized_pattern": pattern_string
                    });
                    output_list.push(summary_obj);
                    summarized_hashes.clear();
                }
            };

            for (i, current_hash) in hash_sequence.iter().enumerate() {
                let is_first = first_occurrence_indices[current_hash] == i;

                // Determine whether to show example based on position_dependent mode
                // Matches Python's logic exactly
                let should_show_example = if position_dependent {
                    // Position-dependent: show examples independently at each depth level
                    is_first
                } else {
                    // Position-independent: show ONLY at minimum depth (shallowest occurrence)
                    // AND only if we haven't shown this hash before (global counter check)
                    let hash_min_depth = min_depths.get(current_hash).copied().unwrap_or(usize::MAX);
                    let examples_shown_count = global_examples_shown.get(current_hash).copied().unwrap_or(0);
                    // MAX_EXAMPLES_PER_STRUCTURE = 1 in Python, so check < 1
                    is_first && depth == hash_min_depth && examples_shown_count < 1
                };

                if should_show_example {
                    process_summary_block(&mut summarized_hashes_block, &mut hashes_referenced_in_summaries, &mut new_list);

                    let distilled_item = distilled_first_examples.get(current_hash)
                        .ok_or_else(|| DistillError::Internal(format!("Distilled example missing for hash {}", current_hash)))?
                        .clone();

                    first_item_positions.insert(current_hash.clone(), new_list.len());
                    new_list.push(distilled_item);

                    // Increment global counter (matches Python's global_examples_tracker)
                    *global_examples_shown.entry(current_hash.clone()).or_insert(0) += 1;
                } else {
                    summarized_hashes_block.push(current_hash.clone());
                }
            }
            process_summary_block(&mut summarized_hashes_block, &mut hashes_referenced_in_summaries, &mut new_list);

            // Label first examples that appear in summaries
            for (hash_str, index_in_new_list) in &first_item_positions {
                if hashes_referenced_in_summaries.contains(hash_str) {
                    if let Some(item_to_label) = new_list.get_mut(*index_in_new_list) {
                        if let Value::Object(obj_map) = item_to_label {
                            obj_map.entry("_structure_hash".to_string())
                                .or_insert_with(|| Value::String(hash_str.clone()));
                        }
                    }
                }
            }

            Ok(Value::Array(new_list))
        }
        primitive => Ok(primitive.clone()),
    }
}

pub fn distill_json(
    json_data: Value,
    strict_typing: bool,
    repeat_threshold: usize,
    position_dependent: bool,
) -> Result<Value> {
    // Use IndexMap for insertion-order preservation (matches Python behavior)
    let mut memoized_examples: MemoCache = IndexMap::new();
    let mut structure_cache: StructureCache = FxHashMap::default();
    // Global counter for examples shown (matches Python's global_examples_tracker)
    let mut global_examples_shown: FxHashMap<String, usize> = FxHashMap::default();

    // Pass 1: Collect minimum depths for each hash (when position_dependent=false)
    let mut min_depths: FxHashMap<String, usize> = FxHashMap::default();
    if !position_dependent && matches!(json_data, Value::Object(_) | Value::Array(_)) {
        collect_structure_depths(&json_data, 0, strict_typing, &mut structure_cache, &mut min_depths)?;
    }

    let distilled_data = distill_recursive(
        &json_data,
        strict_typing,
        repeat_threshold,
        &mut memoized_examples,
        &mut structure_cache,
        0,
        &min_depths,
        position_dependent,
        &mut global_examples_shown,
    )?;

    let description = format!(
        "Distilled JSON structure. Shows the first encountered example for each unique deep structure within lists.
POSITION_DEPENDENT mode: {}
  - true: Examples shown independently at each nesting level (predictable, depth-aware).
  - false: Examples shown only at shallowest occurrence (more concise, globally unique).
Items between these examples are summarized by a 'summarized_pattern' object, indicating the sequence
of structure hashes (e.g., hashA hashB(x3) [hashC hashD](x2)) and the total item count.
First examples are labeled with '_structure_hash' only if their hash appears in a subsequent summary pattern.
Strict primitive typing for structure detection: {}. Repeat threshold for pattern summarization (internal, affects formatting): >=2.",
        if position_dependent { "true" } else { "false" },
        if strict_typing { "true" } else { "false" }
    );

    let mut final_output_map = Map::new();
    final_output_map.insert("description".to_string(), Value::String(description));
    final_output_map.insert("distilled_data".to_string(), distilled_data);

    Ok(Value::Object(final_output_map))
}
