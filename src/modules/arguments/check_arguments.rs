pub fn check_arguments(args: &Vec<String>) -> Result<(), String> {
    println!("Usage: file_indexer [--help]");

    // Debug: Print all provided arguments
    println!("Provided all provided arguments ({} in total):", args.len());
    for arg in args {
        println!("- {}", arg);
    }
    println!();

    if args.len() == 0 {
        println!("⚠️ No arguments provided.");
        println!("\tThat is weird since the first argument is usually the path that was used to call the program.");
        println!();

        return Err("❌ No arguments provided.".to_string());
    }

    if args.len() == 1 {
        println!("⚠️  Not enough arguments provided.");
        println!("\tThe first argument is usually the path that was used to call this program.");
        println!("\tIn case you wanted to provide an actual command to this program, e.g. the 'help' command, you need to pass it as an additional argument.");
        println!();

        return Ok(());
    }

    if args.len() == 2 {
        let arg = &args[1];

        if arg == "help" {
            println!("Usage: file_indexer [help]");
            println!("\thelp: Show this help message.");
            println!();

            return Ok(());
        }

        return Err(format!("❌ Unknown argument provided: {}.", arg));
    }

    return Err(format!("❌ Too many arguments provided."));
}
