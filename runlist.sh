# Run test with GUI
cargo run --bin uwu-gui -- -r ".\gen_logs.exe --rate 30"
cargo run --bin uwu-gui -- -r "go run gen_logs.go -rate 10"
cargo run --bin uwu-gui -- -r "python gen_logs.py -r 30"
# Run test with TUI
cargo run --bin uwu-tui -- -r ".\gen_logs.exe --rate 30"
