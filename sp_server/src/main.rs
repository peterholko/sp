use siege_perilous::setup;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    println!("{:?}", args);

    let mut command = &"new_game".to_string();

    if args.len() > 1 {
        command = &args[1];
    }

    setup(command);
}
