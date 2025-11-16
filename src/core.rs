// src/core.rs - HIGHLY OPTIMIZED VERSION

use crate::error::{DistillError, Result};
use ahash::AHasher;
use rustc_hash::{FxHashMap, FxHashSet};
use serde_json::{json, Map, Value};
use std::hash::{Hash, Hasher};

// Optimized: Use Vec instead of SmallVec for recursive types (avoids cycle)
// Pre-allocate with capacity to minimize allocations
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum DeepStructureKey {
    Primitive(&'static str),  // Zero-allocation for common type names
    Dict(Vec<(String, DeepStructureKey)>),  // Sorted vec
    List(Vec<DeepStructureKey>),            // Sorted vec of unique structures
    EmptyList,
}

// Optimized: Use FxHashMap instead of Arc<DashMap> (single-threaded = no sync overhead)
type StructureCache = FxHashMap<u64, DeepStructureKey>;
type MemoCache = FxHashMap<(String, bool), Value>;
type FirstExampleCache = FxHashMap<String, Value>;

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
            // Optimization: Pre-allocate with exact capacity
            let mut pairs: Vec<(String, DeepStructureKey)> = Vec::with_capacity(map.len());
            for (k, v) in map {
                pairs.push((k.clone(), get_deep_structure_key_cached(v, strict_typing, cache)?));
            }
            pairs.sort_unstable_by(|a, b| a.0.cmp(&b.0));
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

#[inline]
fn generate_hash(key: &DeepStructureKey) -> Result<String> {
    let mut hasher = AHasher::default();
    key.hash(&mut hasher);
    let hash = hasher.finish();
    // Optimization: Use truncation instead of format for speed
    Ok(format!("{:08x}", (hash & 0xFFFFFFFF) as u32))
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
        if i + 3 < n {
            // Optimization: Avoid creating temporary vec
            if hash_sequence[i + 2] == hash_sequence[i] &&
               hash_sequence[i + 3] == hash_sequence[i + 1] {
                let pattern_a = &hash_sequence[i];
                let pattern_b = &hash_sequence[i + 1];

                let mut run_len_pairs = 1;
                while i + (run_len_pairs + 1) * 2 + 1 < n &&
                      hash_sequence[i + run_len_pairs * 2 + 2] == *pattern_a &&
                      hash_sequence[i + run_len_pairs * 2 + 3] == *pattern_b {
                    run_len_pairs += 1;
                }

                if run_len_pairs >= 1 {
                    output_sequence.push(json!({
                        "pattern": [pattern_a, pattern_b],
                        "repeat": run_len_pairs
                    }));
                    i += run_len_pairs * 2;
                    continue;
                }
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
    first_examples_cache: &mut FirstExampleCache,
    structure_cache: &mut StructureCache,
) -> Result<Value> {
    match original_container {
        Value::Object(map) => {
            // Optimization: Pre-allocate with exact capacity
            let mut new_map = Map::with_capacity(map.len());
            for (k, v_original) in map {
                new_map.insert(
                    k.clone(),
                    distill_recursive(v_original, strict_typing, _repeat_threshold, memoized_examples, first_examples_cache, structure_cache)?
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
            // Optimization: Pre-allocate vectors with capacity
            let mut hash_sequence: Vec<String> = Vec::with_capacity(original_list.len());
            let mut first_occurrence_indices: FxHashMap<String, usize> = FxHashMap::with_capacity_and_hasher(
                original_list.len() / 10,  // Estimate 10% unique structures
                Default::default()
            );

            // First pass: compute hashes and track first occurrences
            for (i, item) in original_list.iter().enumerate() {
                let deep_key = get_deep_structure_key_cached(item, strict_typing, structure_cache)?;
                let current_hash = generate_hash(&deep_key)?;
                hash_sequence.push(current_hash.clone());

                first_occurrence_indices.entry(current_hash.clone()).or_insert_with(|| {
                    first_examples_cache.entry(current_hash).or_insert_with(|| item.clone());
                    i
                });
            }

            // Second pass: distill first examples
            let mut distilled_first_examples: FxHashMap<String, Value> = FxHashMap::with_capacity_and_hasher(
                first_occurrence_indices.len(),
                Default::default()
            );

            for hash in first_occurrence_indices.keys() {
                let memo_key = (hash.clone(), false);

                if let Some(cached_value) = memoized_examples.get(&memo_key) {
                    distilled_first_examples.insert(hash.clone(), cached_value.clone());
                } else {
                    // Clone the original item to avoid borrow checker issues
                    let original_item = first_examples_cache.get(hash)
                        .ok_or_else(|| DistillError::Internal(format!("Original first example missing for hash {}", hash)))?
                        .clone();

                    let distilled_value = distill_recursive(
                        &original_item,
                        strict_typing,
                        _repeat_threshold,
                        memoized_examples,
                        first_examples_cache,
                        structure_cache
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
                        "summarized_pattern": pattern_string,
                        "item_count": summarized_hashes.len()
                    });
                    output_list.push(summary_obj);
                    summarized_hashes.clear();
                }
            };

            for (i, current_hash) in hash_sequence.iter().enumerate() {
                let is_first = first_occurrence_indices[current_hash] == i;
                if is_first {
                    process_summary_block(&mut summarized_hashes_block, &mut hashes_referenced_in_summaries, &mut new_list);

                    let distilled_item = distilled_first_examples.get(current_hash)
                        .ok_or_else(|| DistillError::Internal(format!("Distilled example missing for hash {}", current_hash)))?
                        .clone();

                    first_item_positions.insert(current_hash.clone(), new_list.len());
                    new_list.push(distilled_item);
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
) -> Result<Value> {
    // Optimization: Use FxHashMap instead of Arc<DashMap> (no sync overhead)
    let mut memoized_examples: MemoCache = FxHashMap::default();
    let mut first_examples_cache: FirstExampleCache = FxHashMap::default();
    let mut structure_cache: StructureCache = FxHashMap::default();

    let distilled_data = distill_recursive(
        &json_data,
        strict_typing,
        repeat_threshold,
        &mut memoized_examples,
        &mut first_examples_cache,
        &mut structure_cache,
    )?;

    let description = format!(
        "Distilled JSON structure. Shows the first encountered example for each unique deep structure within lists. \
        Items between these examples are summarized by a 'summarized_pattern' object, indicating the sequence \
        of structure hashes (e.g., hashA hashB(x3) [hashC hashD](x2)) and the total item count. \
        First examples are labeled with '_structure_hash' only if their hash appears in a subsequent summary pattern. \
        Strict primitive typing for structure detection: {}. Repeat threshold for pattern summarization (internal, affects formatting): >=2.",
        strict_typing
    );

    let mut final_output_map = Map::new();
    final_output_map.insert("description".to_string(), Value::String(description));
    final_output_map.insert("distilled_data".to_string(), distilled_data);

    Ok(Value::Object(final_output_map))
}
