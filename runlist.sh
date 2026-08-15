# Run test with grahphic
cargo run -p uwu-gui -- -r ".\gen_logs.exe --rate 30"
cargo run -p uwu-gui -- -r "go run gen_logs.go -rate 10"
cargo run -p uwu-gui -- -r "python gen_logs.py -r 30"
# Run test with terminal
cargo run -p uwu-tui -- -r ".\gen_logs.exe --rate 30"