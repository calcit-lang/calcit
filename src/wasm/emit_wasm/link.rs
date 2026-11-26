//! Selective linking and reachability pruning for generated WASM modules.
//!
//! Computes transitive reachability from exported entry points (`export_name`),
//! compacts local function indices, remaps call references, and elides the
//! embedded Ryū decimal formatter functions, private stack global, and
//! read-only lookup table when unused.

use super::*;
use std::collections::{HashMap, HashSet};

pub(super) struct LinkedModule {
  pub(super) kept_import_indices: Vec<usize>,
  pub(super) fns: Vec<CompiledFn>,
  pub(super) needs_number_format: bool,
}

fn touches_number_stack(fns: &[CompiledFn], number_stack_global: u32) -> bool {
  fns.iter().flat_map(|f| f.instructions.iter()).any(|instruction| {
    matches!(
      instruction,
      Instruction::GlobalGet(index) | Instruction::GlobalSet(index) if *index == number_stack_global
    )
  })
}

fn remap_instruction(instruction: &Instruction<'static>, old_to_new: &HashMap<u32, u32>) -> Instruction<'static> {
  match instruction {
    Instruction::Call(index) => Instruction::Call(old_to_new.get(index).copied().unwrap_or(*index)),
    Instruction::ReturnCall(index) => Instruction::ReturnCall(old_to_new.get(index).copied().unwrap_or(*index)),
    other => other.clone(),
  }
}

/// Prune unreachable local functions and unreferenced Ryū number-formatting
/// artifacts before encoding sections.
pub(super) fn link_module(fns: &[CompiledFn], num_imports: u32, layout: &ModuleFunctionLayout) -> LinkedModule {
  let mut pending: Vec<u32> = fns
    .iter()
    .enumerate()
    .filter_map(|(i, f)| f.export_name.as_ref().map(|_| num_imports + i as u32))
    .collect();

  // Synthetic unit-test modules with no exports keep all functions.
  if pending.is_empty() {
    let needs_number_format = layout.number_format_fn_count > 0 || touches_number_stack(fns, layout.global_layout.number_stack);
    return LinkedModule {
      kept_import_indices: (0..num_imports as usize).collect(),
      fns: fns.to_vec(),
      needs_number_format,
    };
  }

  let mut reachable = HashSet::new();
  while let Some(fn_idx) = pending.pop() {
    if !reachable.insert(fn_idx) || fn_idx < num_imports {
      continue;
    }
    let Some(func) = fns.get((fn_idx - num_imports) as usize) else {
      continue;
    };
    for instruction in &func.instructions {
      if let Instruction::Call(callee) | Instruction::ReturnCall(callee) = instruction {
        pending.push(*callee);
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

  let ryu_end = num_imports.saturating_add(layout.number_format_fn_count);
  let ryu_fn_reachable = (num_imports..ryu_end).any(|idx| reachable.contains(&idx));
  let needs_number_format = ryu_fn_reachable || touches_number_stack(&kept_fns, layout.global_layout.number_stack);

  LinkedModule {
    kept_import_indices,
    fns: kept_fns,
    needs_number_format,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn empty_layout(number_format_fn_count: u32) -> ModuleFunctionLayout {
    ModuleFunctionLayout {
      number_format_fn_count,
      memory_layout: WasmMemoryLayout::compute(16384, 0),
      global_layout: WasmGlobalLayout::compute(0, false),
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
  fn pure_arithmetic_prunes_dead_runtime_and_ryu() {
    // Imports: 0..2
    // Local 2 (0): __rt_ryu_format (number_format_fn_count = 2 -> indices 2, 3)
    // Local 3 (1): __rt_ryu_helper
    // Local 4 (2): __rt_dead_helper
    // Local 5 (3): user helper called by main
    // Local 6 (4): exported main (calls 5 and import 1)
    let layout = empty_layout(2);
    let num_stack = layout.global_layout.number_stack;
    let fns = vec![
      fn_with(
        None,
        vec![Instruction::GlobalGet(num_stack), Instruction::Drop, Instruction::Call(3)],
      ),
      fn_with(None, vec![]),
      fn_with(None, vec![Instruction::Unreachable]),
      fn_with(None, vec![]),
      fn_with(Some("add"), vec![Instruction::Call(5), Instruction::Call(1)]),
    ];

    let linked = link_module(&fns, 2, &layout);
    assert!(!linked.needs_number_format);
    assert_eq!(linked.fns.len(), 2);
    assert_eq!(linked.fns[0].export_name, None);
    assert_eq!(linked.fns[1].export_name.as_deref(), Some("add"));
    assert!(matches!(linked.fns[1].instructions[0], Instruction::Call(2)));
    assert!(matches!(linked.fns[1].instructions[1], Instruction::Call(1)));
  }

  #[test]
  fn transitive_call_to_ryu_keeps_formatter_and_remaps_indices() {
    let layout = empty_layout(2);
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
  fn unexported_module_preserves_functions_without_pruning() {
    let layout = empty_layout(0);
    let fns = vec![fn_with(None, vec![Instruction::Call(1)]), fn_with(None, vec![])];
    let linked = link_module(&fns, 0, &layout);
    assert_eq!(linked.fns.len(), 2);
    assert!(!linked.needs_number_format);
  }

  #[test]
  fn pruning_unused_imports_compacts_import_and_function_indices() {
    // Old imports: 0 (unused), 1 (called by exported entry), 2 (unused), 3 (called by helper).
    // Old local 4 (0): dead runtime helper
    // Old local 5 (1): user helper -> calls import 3
    // Old local 6 (2): exported entry -> calls import 1 and local 5
    let mut layout = empty_layout(0);
    layout.prune_unused_imports = true;
    let fns = vec![
      fn_with(None, vec![Instruction::Unreachable]),
      fn_with(None, vec![Instruction::Call(3)]),
      fn_with(Some("entry"), vec![Instruction::Call(1), Instruction::Call(5)]),
    ];

    let linked = link_module(&fns, 4, &layout);
    // Kept imports: [1, 3] -> new indices 0 and 1.
    assert_eq!(linked.kept_import_indices, vec![1, 3]);
    // Kept locals: old 5 -> new 2, old 6 -> new 3 (entry).
    assert_eq!(linked.fns.len(), 2);
    assert!(matches!(linked.fns[0].instructions[0], Instruction::Call(1)));
    assert_eq!(linked.fns[1].export_name.as_deref(), Some("entry"));
    assert!(matches!(linked.fns[1].instructions[0], Instruction::Call(0)));
    assert!(matches!(linked.fns[1].instructions[1], Instruction::Call(2)));
  }

  #[test]
  fn preserving_imports_when_pruning_disabled_keeps_all_import_slots() {
    let layout = empty_layout(0);
    let fns = vec![fn_with(Some("entry"), vec![Instruction::Call(2)])];
    let linked = link_module(&fns, 3, &layout);
    assert_eq!(linked.kept_import_indices, vec![0, 1, 2]);
    assert_eq!(linked.fns.len(), 1);
    assert!(matches!(linked.fns[0].instructions[0], Instruction::Call(2)));
  }
}
