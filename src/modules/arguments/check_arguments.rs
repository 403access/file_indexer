use std::env::current_dir;

pub fn check_arguments(args: &Vec<String>) -> Result<String, String> {
    println!("Usage: file_indexer [--help]");

    // Debug: Print all provided arguments
    println!("Provided all provided arguments ({} in total):", args.len());
    for arg in args {
        println!("- {}", arg);
    }

    if args.len() == 0 {
        println!("⚠️ No arguments provided.");
        println!("\tThat is weird since the first argument is usually the path that was used to call the program.");

        return Err("❌ No arguments provided.".to_string());
    }

    if args.len() == 1 {
        println!("⚠️ Not enough arguments provided.");
        println!("\tThe first argument is the path that was used to call the program. It's called the current working directory; short: CWD.");

        println!("\t-By default, the CWD is used as path.");
        let cwd = current_dir().map_err(|e| format!("Failed to get current directory: {e}"))?;

        let path = cwd.to_string_lossy().to_string();
        println!("\t-CWD: {}", path);

        println!("\t-Pass a specific path as an argument to index that directory.");

        return Ok(path);
    }

    if args.len() == 2 {
        let arg = &args[0];

        if arg == "--help" {
            println!("Usage: file_indexer [--help]");
            println!("\t--help: Show this help message.");
            println!("\tArguments:");
            println!("\t\t[optional] path: The directory path to index. If not provided, the current working directory is used.");

            return Err("Help message displayed.".to_string());
        }

        println!("✅ Using provided path: {}", arg);

        return Ok(arg.to_string());
    }

    return Err("❌ Too many arguments provided.".to_string());
}
