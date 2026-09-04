# Make Linux classification gates self-contained

Review identified that the new workflow steps depended on an earlier `cargo
run` leaving `target/debug/calcit` behind. Each pull-request and publish gate
now explicitly runs `cargo build --bin calcit` before checking the generated
Dynamic classification, so reordering or conditionally skipping unrelated
steps cannot silently break the gate.
