# Reconcile concrete data-definition schemas in scaffold plans

- Accept an existing concrete `StructDef` or `EnumDef` schema when a scaffold plan declares the corresponding broad definition-kind marker.
- Decode zero-payload canonical schema wrappers consistently at both snapshot-load and write-validation boundaries, avoiding their accidental interpretation as anonymous enum values.
- Keep the compatibility rule directional and limited to data-definition schemas so unrelated function and value schemas remain strict.
- Cover both struct and enum definition markers with a regression test.
