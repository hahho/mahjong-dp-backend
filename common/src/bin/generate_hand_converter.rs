use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() != 2 {
        eprintln!("Usage: {} <output_path>", args[0]);
        eprintln!("Example: {} converter.dat", args[0]);
        std::process::exit(1);
    }
    
    let output_path = &args[1];
    let conv = common::mahjong::HandConverter::new();
    conv.save_as_file(output_path).unwrap();
    println!("HandConverter saved to: {}", output_path);
}
