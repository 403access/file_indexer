use inquire::Select;
use std::io;

use crate::modules::commands::{
    commands_progress_bar::{create_progress_bar, set_runnning_message},
    commands_setup::{build_commands, validate_commands},
};

pub fn commands_loop() -> io::Result<()> {
    let commands = build_commands();
    validate_commands(&commands)?;

    loop {
        let options = commands
            .iter()
            .map(|cmd| cmd.name.to_string())
            .collect::<Vec<String>>();

        let ans = Select::new("Choose an action:", options.clone())
            .prompt()
            .unwrap();

        let pb = create_progress_bar();
        set_runnning_message(&pb, &ans);

        match commands.iter().find(|c| c.name == ans) {
            Some(cmd) => match (cmd.action)(&pb) {
                // Exit command returns true
                Ok(true) => break,
                // Other commands return false unless app should exit
                Ok(false) => {}
                // Error handling
                Err(e) => pb.println(format!("[error] Command failed: {}", e)),
            },
            None => pb.println("[error] Command not found.".to_string()),
        }

        pb.finish_with_message("Done ✅");

        println!("\nPress Enter to go back...");
        let _ = std::io::stdin().read_line(&mut String::new());
    }

    Ok(())
}
