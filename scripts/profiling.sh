
cargo build --release
sudo flamegraph -o target/my_flamegraph.svg target/release/calcit
echo `pwd`/`ls target/my_flamegraph.svg` | pbcopy

echo
echo "Copiled svg path."