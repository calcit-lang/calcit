# Keep malformed macro parameters as hard errors

- Review identified that the staged macro-schema partition was too broad and also included `E_DEF_PARAM_SHAPE`.
- Only schema compatibility mismatches (`E_SCHEMA_REQUIRED_ARGS`, `E_SCHEMA_OPTIONAL_ARGS`, and `E_SCHEMA_REST_ARGS`) are now staged during ecosystem migration.
- Malformed source parameter sequences and schema-kind mismatches remain hard preprocessing errors.
