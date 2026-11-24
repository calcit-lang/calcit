//! Selective linking and reachability pruning for generated WASM modules.
//!
//! Computes transitive reachability from exported entry points (`export_name`),
//! tracks reachable `call_indirect` funcref tables, compacts local function
//! indices, remaps call/table references, and elides the embedded Ryū decimal
//! formatter functions, private stack global, and read-only lookup table when
//! unused.

use super::*;
use std::collections::{BTreeSet, HashMap, HashSet};

pub(super) struct LinkedModule {
  pub(super) kept_import_indices: Vec<usize>,
  pub(super) fns: Vec<CompiledFn>,
  pub(super) used_tables: BTreeSet<u32>,
  pub(super) table_elements: Vec<(u32, Vec<u32>)>,
  pub(super) needs_number_format: bool,
}

fn collect_used_tables(fns: &[CompiledFn]) -> BTreeSet<u32> {
  fns
    .iter()
    .flat_map(|f| f.instructions.iter())
    .filter_map(|instruction| match instruction {
      Instruction::CallIndirect { table_index, .. } | Instruction::ReturnCallIndirect { table_index, .. } => Some(*table_index),
      _ => None,
    })
    .collect()
}

fn touches_number_stack(fns: &[CompiledFn], number_stack_global: u32) -> bool {
  fns.iter().flat_map(|f| f.instructions.iter()).any(|instruction| {
    matches!(
      instruction,
      Instruction::GlobalGet(index) | Instruction::GlobalSet(index) if *index == number_stack_global
    )
  })
}

fn build_table_elements(
  used_tables: &BTreeSet<u32>,
  num_imports: u32,
  layout: &ModuleFunctionLayout,
  remap: Option<&HashMap<u32, u32>>,
) -> Vec<(u32, Vec<u32>)> {
  if layout.table_fn_count == 0 || used_tables.is_empty() {
    return Vec::new();
  }
  used_tables
    .iter()
    .map(|&table| {
      let fn_indices = (0..layout.table_fn_count)
        .map(|slot| {
          let old_target = layout
            .rest_adapters
            .get(&(table, slot))
            .copied()
            .unwrap_or(num_imports + layout.runtime_fn_count + slot);
          remap.and_then(|map| map.get(&old_target).copied()).unwrap_or(old_target)
        })
        .collect();
      (table, fn_indices)
    })
    .collect()
}

fn remap_instruction(instruction: &Instruction<'static>, old_to_new: &HashMap<u32, u32>) -> Instruction<'static> {
  match instruction {
    Instruction::Call(index) => Instruction::Call(old_to_new.get(index).copied().unwrap_or(*index)),
    Instruction::ReturnCall(index) => Instruction::ReturnCall(old_to_new.get(index).copied().unwrap_or(*index)),
    Instruction::RefFunc(index) => Instruction::RefFunc(old_to_new.get(index).copied().unwrap_or(*index)),
    other => other.clone(),
  }
}

/// Prune unreachable local functions, unused `call_indirect` funcref tables, and
/// unreferenced Ryū number-formatting artifacts before encoding sections.
pub(super) fn link_module(fns: &[CompiledFn], num_imports: u32, layout: &ModuleFunctionLayout) -> LinkedModule {
  let mut pending: Vec<u32> = fns
    .iter()
    .enumerate()
    .filter_map(|(i, f)| f.export_name.as_ref().map(|_| num_imports + i as u32))
    .collect();

  // Synthetic unit-test modules with no exports keep all functions and tables.
  if pending.is_empty() {
    let used_tables = collect_used_tables(fns);
    let table_elements = build_table_elements(&used_tables, num_imports, layout, None);
    let needs_number_format = layout.number_format_fn_count > 0 || touches_number_stack(fns, layout.global_layout.number_stack);
    return LinkedModule {
      kept_import_indices: (0..num_imports as usize).collect(),
      fns: fns.to_vec(),
      used_tables,
      table_elements,
      needs_number_format,
    };
  }

  let mut reachable = HashSet::new();
  let mut used_tables = BTreeSet::new();
  while let Some(fn_idx) = pending.pop() {
    if !reachable.insert(fn_idx) || fn_idx < num_imports {
      continue;
    }
    let Some(func) = fns.get((fn_idx - num_imports) as usize) else {
      continue;
    };
    for instruction in &func.instructions {
      match instruction {
        Instruction::Call(callee) | Instruction::ReturnCall(callee) | Instruction::RefFunc(callee) => {
          pending.push(*callee);
        }
        Instruction::CallIndirect { table_index, .. } | Instruction::ReturnCallIndirect { table_index, .. }
          if used_tables.insert(*table_index) =>
        {
          for slot in 0..layout.table_fn_count {
            let target = layout
              .rest_adapters
              .get(&(*table_index, slot))
              .copied()
              .unwrap_or(num_imports + layout.runtime_fn_count + slot);
            pending.push(target);
          }
        }
        _ => {}
      }
    }
  }

  let kept_import_indices: Vec<usize> = if layout.prune_unused_imports {
    (0..num_imports as usize).filter(|&i| reachable.contains(&(i as u32))).collect()
  } else {
    (0..num_imports as usize).collect()
  };
  let new_num_imports = kept_import_indices.len() as u32;

  let mut old_to_new: HashMap<u32, u32> = HashMap::with_capacity(kept_import_indices.len() + reachable.len());
  for (new_idx, &old_idx) in kept_import_indices.iter().enumerate() {
    old_to_new.insert(old_idx as u32, new_idx as u32);
  }
  let mut kept_indices = Vec::new();
  for i in 0..fns.len() {
    let old_idx = num_imports + i as u32;
    if reachable.contains(&old_idx) {
      let new_idx = new_num_imports + kept_indices.len() as u32;
      old_to_new.insert(old_idx, new_idx);
      kept_indices.push(i);
    }
  }

  let kept_fns: Vec<CompiledFn> = kept_indices
    .into_iter()
    .map(|i| {
      let src = &fns[i];
      CompiledFn {
        export_name: src.export_name.clone(),
        params: src.params.clone(),
        results: src.results.clone(),
        locals: src.locals.clone(),
        instructions: src.instructions.iter().map(|instr| remap_instruction(instr, &old_to_new)).collect(),
      }
    })
    .collect();

  let table_elements = build_table_elements(&used_tables, num_imports, layout, Some(&old_to_new));
  let ryu_end = num_imports.saturating_add(layout.number_format_fn_count);
  let ryu_fn_reachable = (num_imports..ryu_end).any(|idx| reachable.contains(&idx));
  let needs_number_format = ryu_fn_reachable || touches_number_stack(&kept_fns, layout.global_layout.number_stack);

  LinkedModule {
    kept_import_indices,
    fns: kept_fns,
    used_tables,
    table_elements,
    needs_number_format,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn empty_layout(runtime_fn_count: u32, number_format_fn_count: u32, table_fn_count: u32) -> ModuleFunctionLayout {
    ModuleFunctionLayout {
      runtime_fn_count,
      number_format_fn_count,
      table_fn_count,
      memory_layout: WasmMemoryLayout::compute(16384, 0),
      global_layout: WasmGlobalLayout::compute(0, false),
      rest_adapters: HashMap::new(),
      prune_unused_imports: false,
    }
  }

  fn fn_with(export_name: Option<&str>, instructions: Vec<Instruction<'static>>) -> CompiledFn {
    CompiledFn {
      export_name: export_name.map(str::to_owned),
      params: vec![],
      results: vec![],
      locals: vec![],
      instructions,
    }
  }

  #[test]
  fn pure_arithmetic_prunes_dead_runtime_ryu_and_unused_tables() {
    // Imports: 0..2
    // Local 2 (0): __rt_ryu_format (number_format_fn_count = 2 -> indices 2, 3)
    // Local 3 (1): __rt_ryu_helper
    // Local 4 (2): __rt_dead_hof (calls table 1 indirectly)
    // Local 5 (3): user helper called by main
    // Local 6 (4): exported main (calls 5 and import 1)
    let layout = empty_layout(3, 2, 1);
    let num_stack = layout.global_layout.number_stack;
    let fns = vec![
      fn_with(
        None,
        vec![Instruction::GlobalGet(num_stack), Instruction::Drop, Instruction::Call(3)],
      ),
      fn_with(None, vec![]),
      fn_with(
        None,
        vec![Instruction::CallIndirect {
          type_index: 1,
          table_index: 1,
        }],
      ),
      fn_with(None, vec![]),
      fn_with(Some("add"), vec![Instruction::Call(5), Instruction::Call(1)]),
    ];

    let linked = link_module(&fns, 2, &layout);
    assert!(!linked.needs_number_format);
    assert!(linked.used_tables.is_empty());
    assert!(linked.table_elements.is_empty());
    assert_eq!(linked.fns.len(), 2);
    assert_eq!(linked.fns[0].export_name, None);
    assert_eq!(linked.fns[1].export_name.as_deref(), Some("add"));
    assert!(matches!(linked.fns[1].instructions[0], Instruction::Call(2)));
    assert!(matches!(linked.fns[1].instructions[1], Instruction::Call(1)));
  }

  #[test]
  fn transitive_call_to_ryu_keeps_formatter_and_remaps_indices() {
    let layout = empty_layout(3, 2, 0);
    let num_stack = layout.global_layout.number_stack;
    // No imports: local 0 = ryu_entry -> calls 1; local 1 = ryu_helper; local 2 = dead helper;
    // local 3 = format_wrapper -> calls 0; local 4 = exported print -> calls 3.
    let fns = vec![
      fn_with(None, vec![Instruction::GlobalSet(num_stack), Instruction::Call(1)]),
      fn_with(None, vec![]),
      fn_with(None, vec![Instruction::Unreachable]),
      fn_with(None, vec![Instruction::Call(0)]),
      fn_with(Some("print_num"), vec![Instruction::Call(3)]),
    ];

    let linked = link_module(&fns, 0, &layout);
    assert!(linked.needs_number_format);
    // Kept old indices: {0, 1, 3, 4} -> new indices {0, 1, 2, 3}
    assert_eq!(linked.fns.len(), 4);
    assert!(matches!(linked.fns[0].instructions[1], Instruction::Call(1)));
    assert!(matches!(linked.fns[2].instructions[0], Instruction::Call(0)));
    assert!(matches!(linked.fns[3].instructions[0], Instruction::Call(2)));
    assert_eq!(linked.fns[3].export_name.as_deref(), Some("print_num"));
  }

  #[test]
  fn call_indirect_keeps_table_slots_rest_adapters_and_remaps_elements() {
    // 0: dead runtime helper
    // 1: dead ryu helper
    // 2: table slot 0 (user fn 0, unexported) -> called via rest adapter
    // 3: exported main -> CallIndirect { table_index: 2 }
    // 4: rest adapter for (table 2, slot 0) -> calls 2
    let mut layout = empty_layout(2, 1, 1);
    layout.rest_adapters.insert((2, 0), 4);
    let fns = vec![
      fn_with(None, vec![Instruction::Unreachable]),
      fn_with(None, vec![Instruction::Unreachable]),
      fn_with(None, vec![]),
      fn_with(
        Some("entry"),
        vec![Instruction::CallIndirect {
          type_index: 2,
          table_index: 2,
        }],
      ),
      fn_with(None, vec![Instruction::Call(2)]),
    ];

    let linked = link_module(&fns, 0, &layout);
    assert!(!linked.needs_number_format);
    assert_eq!(linked.used_tables.iter().copied().collect::<Vec<_>>(), vec![2]);
    // Kept old indices: {2, 3, 4} -> new {0, 1, 2}
    assert_eq!(linked.fns.len(), 3);
    assert_eq!(linked.fns[1].export_name.as_deref(), Some("entry"));
    assert!(matches!(linked.fns[2].instructions[0], Instruction::Call(0)));
    assert_eq!(linked.table_elements, vec![(2, vec![2])]);
  }

  #[test]
  fn unexported_module_preserves_functions_without_pruning() {
    let layout = empty_layout(1, 0, 0);
    let fns = vec![fn_with(None, vec![Instruction::Call(1)]), fn_with(None, vec![])];
    let linked = link_module(&fns, 0, &layout);
    assert_eq!(linked.fns.len(), 2);
    assert!(!linked.needs_number_format);
  }

  #[test]
  fn pruning_unused_imports_compacts_import_and_function_indices() {
    // Old imports: 0 (unused), 1 (called by exported entry), 2 (unused), 3 (called by indirect target).
    // Old local 4 (0): dead runtime helper
    // Old local 5 (1): table slot 0 (user fn 0, unexported) -> calls import 3
    // Old local 6 (2): exported entry -> calls import 1 and CallIndirect { table_index: 1 }
    let mut layout = empty_layout(1, 0, 1);
    layout.prune_unused_imports = true;
    let fns = vec![
      fn_with(None, vec![Instruction::Unreachable]),
      fn_with(None, vec![Instruction::Call(3)]),
      fn_with(
        Some("entry"),
        vec![
          Instruction::Call(1),
          Instruction::CallIndirect {
            type_index: 1,
            table_index: 1,
          },
        ],
      ),
    ];

    let linked = link_module(&fns, 4, &layout);
    // Kept imports: [1, 3] -> new indices 0 and 1.
    assert_eq!(linked.kept_import_indices, vec![1, 3]);
    // Kept locals: old 5 -> new 2 (table slot 0), old 6 -> new 3 (entry).
    assert_eq!(linked.fns.len(), 2);
    assert!(matches!(linked.fns[0].instructions[0], Instruction::Call(1)));
    assert_eq!(linked.fns[1].export_name.as_deref(), Some("entry"));
    assert!(matches!(linked.fns[1].instructions[0], Instruction::Call(0)));
    assert_eq!(linked.table_elements, vec![(1, vec![2])]);
  }

  #[test]
  fn preserving_imports_when_pruning_disabled_keeps_all_import_slots() {
    let layout = empty_layout(0, 0, 0);
    let fns = vec![fn_with(Some("entry"), vec![Instruction::Call(2)])];
    let linked = link_module(&fns, 3, &layout);
    assert_eq!(linked.kept_import_indices, vec![0, 1, 2]);
    assert_eq!(linked.fns.len(), 1);
    assert!(matches!(linked.fns[0].instructions[0], Instruction::Call(2)));
  }
}
